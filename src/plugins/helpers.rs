use dprint_cli_core::types::ErrBox;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;

use super::{BinaryManifestItem, GlobalBinaryLocation, PluginsManifest, get_plugin_dir};
use crate::configuration::ConfigFileBinary;
use crate::environment::{Environment, SYS_PATH_DELIMITER};
use crate::types::{CommandName, NameSelector, PathOrVersionSelector, VersionSelector};
use crate::utils;

pub fn get_installed_binary_if_associated_config_file_binary<'a>(
    manifest: &'a PluginsManifest,
    config_binary: &ConfigFileBinary,
) -> Option<&'a BinaryManifestItem> {
    // the url needs to be associated to an identifier for this to return anything
    if let Some(identifier) = manifest.get_identifier_from_url(&config_binary.path) {
        // return the url version if installed
        if let Some(binary) = manifest.get_binary(&identifier) {
            return Some(binary);
        }

        // else check for the latest matching version in the manifest
        if let Some(version_selector) = &config_binary.version {
            let name_selector = identifier.get_binary_name().to_selector();
            let binary = get_latest_binary_matching_name_and_version(&manifest, &name_selector, version_selector);
            if let Some(binary) = binary {
                return Some(binary);
            }
        }
    }

    None
}

pub fn get_latest_binary_matching_name_and_version<'a>(
    manifest: &'a PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &VersionSelector,
) -> Option<&'a BinaryManifestItem> {
    let binaries = manifest.get_binaries_matching_name_and_version(&name_selector, version_selector);
    get_latest_binary(&binaries)
}

pub fn get_binary_with_name_and_version<'a>(
    plugin_manifest: &'a PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &VersionSelector,
) -> Result<&'a BinaryManifestItem, ErrBox> {
    let binaries = plugin_manifest.get_binaries_matching_name_and_version(name_selector, version_selector);

    if binaries.len() == 0 {
        let binaries = plugin_manifest.get_binaries_matching_name(name_selector);
        if binaries.is_empty() {
            err!("Could not find any installed binaries named '{}'", name_selector)
        } else {
            err!(
                "Could not find binary '{}' that matched version '{}'\n\nInstalled versions:\n  {}",
                name_selector,
                version_selector,
                display_binaries_versions(binaries).join("\n  "),
            )
        }
    } else if !get_have_same_owner(&binaries) {
        return err!(
            "There were multiple binaries with the specified name '{}' that matched version '{}'. Please include the owner to uninstall.\n\nInstalled versions:\n  {}",
            name_selector,
            version_selector,
            display_binaries_versions(binaries).join("\n  "),
        );
    } else {
        Ok(get_latest_binary(&binaries).unwrap())
    }
}

pub fn display_binaries_versions(binaries: Vec<&BinaryManifestItem>) -> Vec<String> {
    if binaries.is_empty() {
        return Vec::new();
    }

    let mut binaries = binaries;
    binaries.sort();
    let have_same_owner = get_have_same_owner(&binaries);
    let lines = binaries
        .into_iter()
        .map(|b| {
            if have_same_owner {
                b.version.to_string()
            } else {
                format!("{} {}", b.name, b.version)
            }
        })
        .collect::<Vec<_>>();

    return lines;
}

pub fn get_have_same_owner(binaries: &Vec<&BinaryManifestItem>) -> bool {
    if binaries.is_empty() {
        true
    } else {
        let first_owner = &binaries[0].name.owner;
        binaries.iter().all(|b| &b.name.owner == first_owner)
    }
}

pub fn get_latest_binary<'a>(binaries: &Vec<&'a BinaryManifestItem>) -> Option<&'a BinaryManifestItem> {
    let mut latest_binary: Option<&'a BinaryManifestItem> = None;

    for binary in binaries.iter() {
        if let Some(latest_binary_val) = &latest_binary {
            if latest_binary_val.cmp(binary) == Ordering::Less {
                latest_binary = Some(binary);
            }
        } else {
            latest_binary = Some(binary);
        }
    }

    latest_binary
}

pub fn get_global_binary_file_path(
    environment: &impl Environment,
    plugin_manifest: &PluginsManifest,
    command_name: &CommandName,
) -> Result<PathBuf, ErrBox> {
    match plugin_manifest.get_global_binary_location(command_name) {
        Some(location) => match location {
            GlobalBinaryLocation::Path => {
                if let Some(path_executable_path) = utils::get_path_executable_path(environment, command_name) {
                    Ok(path_executable_path)
                } else {
                    err!("Binary '{}' is configured to use the executable on the path, but only the bvm version exists on the path. Run `bvm use {0} <some other version>` to select a version to run.", command_name)
                }
            }
            GlobalBinaryLocation::Bvm(identifier) => {
                if let Some(item) = plugin_manifest.get_binary(&identifier) {
                    let command_exe_path = get_exec_binary_command_exe_path(environment, &item, command_name)
                        .expect("Expected to have a command.");
                    Ok(command_exe_path)
                } else {
                    err!("Should have found executable path for global binary. Report this as a bug and update the version used by running `bvm use {} <some other version>`", command_name)
                }
            }
        },
        None => {
            // use the executable on the path
            if let Some(path_executable_path) = utils::get_path_executable_path(environment, command_name) {
                Ok(path_executable_path)
            } else {
                let binaries = plugin_manifest.get_binaries_with_command(command_name);
                if binaries.is_empty() {
                    err!("Could not find binary on the path for command '{}'", command_name)
                } else {
                    err!(
                        "No binary is set on the path for command '{}'. Run `bvm use {0} <version>` to set a global version.\n\nInstalled versions:\n  {}",
                        command_name,
                        display_binaries_versions(binaries).join("\n  "),
                    )
                }
            }
        }
    }
}

pub fn get_exec_binary_command_exe_path<TEnvironment: Environment>(
    environment: &TEnvironment,
    binary: &BinaryManifestItem,
    command_name: &CommandName,
) -> Option<PathBuf> {
    let command = binary.commands.iter().filter(|c| &c.name == command_name).next();

    if let Some(command) = command {
        Some(get_plugin_dir(environment, &binary.name, &binary.version).join(&command.path))
    } else {
        utils::get_command_executable_path_in_dirs(
            environment,
            &command_name,
            binary.get_resolved_env_paths(environment).into_iter(),
        )
    }
}

pub fn get_command_names_for_name_and_path_or_version_selector(
    plugin_manifest: &PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &PathOrVersionSelector,
) -> Result<Vec<CommandName>, ErrBox> {
    match &version_selector {
        PathOrVersionSelector::Path => {
            // get the current binaries for the selector
            let binaries = plugin_manifest.get_binaries_matching_name(&name_selector);
            let have_same_owner = get_have_same_owner(&binaries);
            if !have_same_owner {
                let mut binary_names = binaries
                    .iter()
                    .map(|b| format!("{}", b.name))
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<String>>();
                binary_names.sort();
                return err!(
                    "There were multiple binaries with the name '{}'. Please include the owner in the name:\n  {}",
                    name_selector.name,
                    binary_names.join("\n  ")
                );
            }

            let latest_binary = binaries.iter().filter(|b| !b.version.is_prerelease()).last();
            if let Some(latest_binary) =
                latest_binary.or_else(|| binaries.iter().filter(|b| b.version.is_prerelease()).last())
            {
                Ok(latest_binary.get_command_names())
            } else {
                return err!("Could not find any installed binaries named '{}'.", name_selector);
            }
        }
        PathOrVersionSelector::Version(version_selector) => {
            let binary = get_binary_with_name_and_version(&plugin_manifest, &name_selector, &version_selector)?;
            Ok(binary.get_command_names())
        }
    }
}

pub fn has_command_name_for_exec<TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &PathOrVersionSelector,
    command_name: &CommandName,
) -> Result<bool, ErrBox> {
    Ok(match &version_selector {
        PathOrVersionSelector::Path => {
            let command_names = get_command_names_for_name_and_path_or_version_selector(
                plugin_manifest,
                name_selector,
                version_selector,
            )?;
            command_names.iter().any(|c| c == command_name)
        }
        PathOrVersionSelector::Version(version_selector) => {
            let binary = get_binary_with_name_and_version(&plugin_manifest, &name_selector, &version_selector)?;
            get_exec_binary_command_exe_path(environment, &binary, &command_name).is_some()
        }
    })
}

pub fn get_global_binary_location_for_name_and_path_or_version_selector(
    plugin_manifest: &PluginsManifest,
    name_selector: &NameSelector,
    version_selector: &PathOrVersionSelector,
) -> Result<GlobalBinaryLocation, ErrBox> {
    Ok(match &version_selector {
        PathOrVersionSelector::Path => GlobalBinaryLocation::Path,
        PathOrVersionSelector::Version(version_selector) => {
            let binary = get_binary_with_name_and_version(&plugin_manifest, &name_selector, &version_selector)?;
            let identifier = binary.get_identifier();
            GlobalBinaryLocation::Bvm(identifier)
        }
    })
}

pub fn get_env_path_from_pending_env_changes<TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &PluginsManifest,
) -> String {
    let mut paths = environment
        .get_env_path()
        .split(&SYS_PATH_DELIMITER)
        .map(String::from)
        .collect::<Vec<_>>();

    for path in plugin_manifest.get_relative_pending_removed_paths(environment) {
        if let Some(pos) = paths.iter().position(|x| x == &path) {
            paths.remove(pos);
        }
    }

    for path in plugin_manifest.get_relative_pending_added_paths(environment) {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }

    paths
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(SYS_PATH_DELIMITER)
}

pub fn recreate_shim(
    environment: &impl Environment,
    plugin_manifest: &PluginsManifest,
    command_name: &CommandName,
) -> Result<(), ErrBox> {
    if let Some(location) = get_command_exe_path(environment, &plugin_manifest, &command_name) {
        crate::plugins::create_shim(environment, &command_name, &location)?;
    } else {
        environment.log_error(&format!("Error creating shim for {}. Could not find binary path.", command_name));
    }

    return Ok(());

    fn get_command_exe_path(
        environment: &impl Environment,
        plugin_manifest: &PluginsManifest,
        command_name: &CommandName,
    ) -> Option<PathBuf> {
        let path = match plugin_manifest.get_global_binary_location(command_name) {
            Some(GlobalBinaryLocation::Bvm(identifier)) => {
                plugin_manifest.get_binary(&identifier)
                    .map(|binary| get_exec_binary_command_exe_path(environment, &binary, command_name))
                    .flatten()
            }
            _ => None
        };

        path.or_else(|| utils::get_path_executable_path(environment, command_name))
    }
}