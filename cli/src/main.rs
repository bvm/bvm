#[macro_use(err_obj)]
#[macro_use(err)]
extern crate dprint_cli_core;
mod types;
#[macro_use]
mod environment;
#[macro_use]
extern crate lazy_static;

#[cfg(test)]
mod test_builders;

mod arg_parser;
mod configuration;
mod plugins;
mod registry;
mod utils;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use arg_parser::*;
use dprint_cli_core::checksums::get_sha256_checksum;
use dprint_cli_core::types::ErrBox;
use environment::Environment;
use environment::SYS_PATH_DELIMITER;
use plugins::helpers as plugin_helpers;
use plugins::PluginsManifest;
use plugins::PluginsMut;
use plugins::UrlInstallAction;
use types::BinaryName;
use types::CommandName;
use types::PathOrVersionSelector;
use types::VersionSelector;
use utils::ChecksumUrl;

use crate::utils::get_url_from_directory;

fn main() {
    match inner_main() {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err.to_string());
            std::process::exit(1);
        }
    }

    fn inner_main() -> Result<(), ErrBox> {
        let environment = environment::RealEnvironment::new(false)?;
        let args = std::env::args().collect();
        run(&environment, args)
    }
}

fn run<TEnvironment: Environment>(environment: &TEnvironment, args: Vec<String>) -> Result<(), ErrBox> {
    let args = parse_args(environment, args)?;

    match args.sub_command {
        SubCommand::Help(text) => environment.log(&text),
        SubCommand::Version => environment.log(&format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))),
        SubCommand::Install(command) => handle_install_command(environment, command)?,
        SubCommand::InstallUrl(command) => handle_install_url_command(environment, command)?,
        SubCommand::Uninstall(command) => handle_uninstall_command(environment, command)?,
        SubCommand::Use => handle_use_command(environment)?,
        SubCommand::UseBinary(command) => handle_use_binary_command(environment, command)?,
        SubCommand::List => handle_list_command(environment)?,
        SubCommand::Init => handle_init_command(environment)?,
        SubCommand::ClearUrlCache => handle_clear_url_cache(environment)?,
        SubCommand::RecreateShims => recreate_shims(environment)?,
        SubCommand::Registry(command) => handle_registry_command(environment, command)?,
        SubCommand::Add(command) => handle_add_command(environment, command)?,
        SubCommand::Hidden(command) => handle_hidden_command(environment, command)?,
    }

    Ok(())
}

fn handle_install_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: InstallCommand,
) -> Result<(), ErrBox> {
    let (_, config_file) = get_config_file_or_error(environment)?;
    let mut plugins = PluginsMut::load(environment);

    if let Some(pre_install) = &config_file.on_pre_install {
        environment.run_shell_command(&environment.cwd(), pre_install)?;
    }

    for binary in config_file.binaries.iter() {
        match install_binary(&mut plugins, &binary.url, binary.version.as_ref(), command.force) {
            Err(err) => return err!("Error installing {}: {}", binary.url.url, err.to_string()),
            _ => {}
        }
    }

    if command.use_command {
        for entry in config_file.binaries.iter() {
            if let Some(binary) = plugins.get_installed_binary_for_config_binary(&entry)? {
                let identifier = binary.get_identifier();
                for command_name in binary.get_command_names() {
                    plugins
                        .use_global_version(&command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
                }
            }
        }
        plugins.save()?;
    }

    if let Some(post_install) = &config_file.on_post_install {
        environment.run_shell_command(&environment.cwd(), post_install)?;
    }

    Ok(())
}

fn install_binary<TEnvironment: Environment>(
    plugins: &mut PluginsMut<TEnvironment>,
    checksum_url: &ChecksumUrl,
    version_selector: Option<&VersionSelector>,
    force: bool,
) -> Result<(), ErrBox> {
    let install_action = plugins.get_url_install_action(checksum_url, version_selector, force)?;
    if let UrlInstallAction::Install(plugin_file) = install_action {
        // setup the plugin
        let binary_item = plugins.setup_plugin(&plugin_file)?;
        let identifier = binary_item.get_identifier();
        // check if there is a global binary location set and if not, set it
        for command_name in binary_item.get_command_names() {
            plugins.set_global_binary_if_not_set(&identifier, &command_name)?;
        }
        plugins.save()?; // write for every setup plugin in case a further one fails
    }
    Ok(())
}

fn handle_install_url_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: InstallUrlCommand,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment);
    let url = resolve_url_or_name(environment, &command.url_or_name)?;

    let result = install_url(environment, &mut plugins, &url, &command);
    match result {
        Ok(()) => {}
        Err(err) => return err!("Error installing {}. {}", url.url, err.to_string()),
    }

    if command.use_command {
        let identifier = plugins
            .manifest
            .get_identifier_from_url(&url)
            .map(|identifier| identifier.clone())
            .unwrap();
        let command_names = plugins.manifest.get_binary(&identifier).unwrap().get_command_names();

        for command_name in command_names.iter() {
            plugins.use_global_version(&command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
        }

        display_commands_in_config_file_warning_if_necessary(environment, &plugins.manifest, &command_names);
    }

    plugins.save()?;

    return Ok(());

    fn install_url<TEnvironment: Environment>(
        environment: &TEnvironment,
        plugins: &mut PluginsMut<TEnvironment>,
        url: &ChecksumUrl,
        command: &InstallUrlCommand,
    ) -> Result<(), ErrBox> {
        let install_action = plugins.get_url_install_action(url, None, command.force)?;

        match install_action {
            UrlInstallAction::None => {
                environment.log_error("Already installed. Provide the `--force` flag to reinstall.")
            }
            UrlInstallAction::Install(plugin_file) => {
                let identifier = plugin_file.get_identifier();
                // remove the existing binary from the cache (the setup_plugin function will delete it from the disk)
                let previous_global_command_names = plugins.manifest.get_global_command_names(&identifier);
                plugins.remove_binary(&identifier)?;
                plugins.save()?;

                let binary_item = plugins.setup_plugin(&plugin_file)?;
                let identifier = binary_item.get_identifier();
                let binary_name = binary_item.name.clone();
                let version = binary_item.version.clone();
                let command_names = binary_item.get_command_names();

                // set this back as being the global version if setup is successful
                for command_name in previous_global_command_names {
                    if command_names.contains(&command_name) {
                        plugins.use_global_version(
                            &command_name,
                            plugins::GlobalBinaryLocation::Bvm(identifier.clone()),
                        )?;
                    }
                }

                if !command.use_command {
                    let mut not_set_command_name = false;
                    for command_name in command_names.iter() {
                        if !plugins.set_global_binary_if_not_set(&identifier, &command_name)? {
                            not_set_command_name = true;
                        }
                    }
                    if not_set_command_name {
                        environment.log_error(&format!(
                            "Installed. Run `bvm use {} {}` to use it on the path as {}.",
                            binary_name
                                .display_toggled_owner(!plugins.manifest.binary_name_has_same_owner(&binary_name)),
                            version,
                            utils::sentence_join(
                                &command_names
                                    .into_iter()
                                    .map(|c| format!("'{}'", c))
                                    .collect::<Vec<_>>()
                            ),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

fn resolve_url_or_name<TEnvironment: Environment>(
    environment: &TEnvironment,
    url_or_name: &UrlOrName,
) -> Result<ChecksumUrl, ErrBox> {
    return match url_or_name {
        UrlOrName::Url(url) => Ok(url.to_owned()),
        UrlOrName::Name(name) => {
            let registry = registry::Registry::load(environment);
            let url_results = registry.get_urls(&name.name_selector);

            if url_results.is_empty() {
                return err!("There were no registries found for the provided binary. Did you mean to add one using `bvm registry add <url>`?");
            }

            // display an error if there are multiple owners
            let mut binary_names = url_results
                .iter()
                .map(|r| &r.owner)
                .collect::<HashSet<_>>()
                .into_iter()
                .map(|o| format!("{}/{}", o, name.name_selector.name))
                .collect::<Vec<String>>();
            if binary_names.len() > 1 {
                binary_names.sort();
                return err!(
                    "There were multiple binaries with the name '{}'. Please include the owner in the name:\n  {}",
                    name.name_selector.name,
                    binary_names.join("\n  ")
                );
            }
            let binary_name = BinaryName::new(url_results[0].owner.clone(), name.name_selector.name.clone());

            // now get the url
            let urls = url_results.into_iter().map(|r| r.url).collect();
            let selected_url = if let Some(version) = &name.version_selector {
                find_url(environment, &urls, &binary_name, |item| version.matches(&item.version))?
            } else {
                find_latest_url(environment, &urls, &binary_name)?
            };
            if let Some(selected_url) = selected_url {
                Ok(selected_url)
            } else {
                if let Some(version) = &name.version_selector {
                    err!(
                        "Could not find binary '{}' matching '{}' in any registry.",
                        name.name_selector,
                        version
                    )
                } else {
                    return err!("Could not find binary '{}' in any registry.", name.name_selector);
                }
            }
        }
    };

    fn find_url<TEnvironment: Environment>(
        environment: &TEnvironment,
        urls: &Vec<String>,
        name: &BinaryName,
        is_match: impl Fn(&registry::RegistryVersionInfo) -> bool,
    ) -> Result<Option<ChecksumUrl>, ErrBox> {
        let mut best_match: Option<registry::RegistryVersionInfo> = None;
        for url in urls.iter() {
            let registry_file = registry::download_registry_file(environment, &url)?;
            if let Some(registry_binary) = registry_file.take_binary_with_name(&name) {
                for version_info in registry_binary.versions {
                    if is_match(&version_info) {
                        if let Some(best_match_val) = &best_match {
                            if best_match_val.version.cmp(&version_info.version) == Ordering::Less {
                                best_match = Some(version_info);
                            }
                        } else {
                            best_match = Some(version_info);
                        }
                    }
                }
            }
        }

        Ok(match best_match {
            Some(version_info) => Some(version_info.get_url()?),
            None => None,
        })
    }

    fn find_latest_url<TEnvironment: Environment>(
        environment: &TEnvironment,
        urls: &Vec<String>,
        name: &BinaryName,
    ) -> Result<Option<ChecksumUrl>, ErrBox> {
        let mut latest_pre_release: Option<registry::RegistryVersionInfo> = None;
        let mut latest_release: Option<registry::RegistryVersionInfo> = None;
        for url in urls.iter() {
            let registry_file = registry::download_registry_file(environment, &url)?;
            if let Some(registry_binary) = registry_file.take_binary_with_name(&name) {
                for item in registry_binary.versions {
                    let latest = if item.version.is_prerelease() {
                        &mut latest_pre_release
                    } else {
                        &mut latest_release
                    };
                    if let Some(latest) = latest.as_mut() {
                        if item.version.gt(&latest.version) {
                            *latest = item;
                        }
                    } else {
                        *latest = Some(item);
                    }
                }
            }
        }

        Ok(match latest_release.or(latest_pre_release) {
            Some(item) => Some(item.get_url()?),
            None => None,
        })
    }
}

fn handle_uninstall_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    uninstall_command: UninstallCommand,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment);
    let binary = plugin_helpers::get_binary_with_name_and_version(
        &plugins.manifest,
        &uninstall_command.name_selector,
        &uninstall_command.version.to_selector(),
    )?;
    let plugin_dir = plugins::get_plugin_dir(environment, &binary.name, &binary.version);
    let binary_identifier = binary.get_identifier();

    // remove the plugin from the manifest first
    plugins.remove_binary(&binary_identifier)?;
    plugins.save()?;

    // now attempt to delete the directory
    environment.remove_dir_all(&plugin_dir)?;

    // delete the parent directories if empty
    let binary_name_dir = plugin_dir.parent().unwrap();
    if environment.is_dir_empty(&binary_name_dir)? {
        environment.remove_dir_all(&binary_name_dir)?;
        // now delete the owner name if empty
        let owner_name_dir = binary_name_dir.parent().unwrap();
        if environment.is_dir_empty(&owner_name_dir)? {
            environment.remove_dir_all(&owner_name_dir)?;
        }
    }

    Ok(())
}

fn handle_use_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    // use all the binaries in the current configuration file
    let mut plugins = PluginsMut::load(environment);
    let (_, config_file) = get_config_file_or_error(environment)?;
    let mut found_not_installed = false;

    for entry in config_file.binaries.iter() {
        if let Some(binary) = plugins.get_installed_binary_for_config_binary(&entry)? {
            let identifier = binary.get_identifier();
            for command_name in binary.get_command_names() {
                plugins.use_global_version(&command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
            }
        } else {
            found_not_installed = true;
        }
    }

    if !found_not_installed {
        return err!("Ensure binaries are installed before using. Run `bvm install` first then `bvm use`.");
    }

    plugins.save()?;
    Ok(())
}

fn handle_use_binary_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    use_command: UseBinaryCommand,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment);
    let command_names = plugin_helpers::get_command_names_for_name_and_path_or_version_selector(
        &plugins.manifest,
        &use_command.name_selector,
        &use_command.version_selector,
    )?;

    let location = plugin_helpers::get_global_binary_location_for_name_and_path_or_version_selector(
        &plugins.manifest,
        &use_command.name_selector,
        &use_command.version_selector,
    )?;

    for command_name in command_names.iter() {
        match &use_command.version_selector {
            PathOrVersionSelector::Path => {
                if utils::get_path_executable_path(environment, &command_name).is_none() {
                    // todo: maybe this should be a warning instead?
                    return err!(
                        "Could not find any installed binaries on the path that matched '{}'.",
                        command_name
                    );
                }
            }
            _ => {}
        }

        plugins.use_global_version(command_name, location.clone())?;
    }

    display_commands_in_config_file_warning_if_necessary(environment, &plugins.manifest, &command_names);

    plugins.save()?;

    Ok(())
}

fn get_is_command_in_config_file(
    environment: &impl Environment,
    plugin_manifest: &PluginsManifest,
    command_name: &CommandName,
) -> bool {
    let result = get_executable_path_from_config_file(environment, &plugin_manifest, &command_name);
    match result {
        Ok(result) => result.map(|info| info.binary_info).flatten().is_some(),
        Err(_) => false,
    }
}

fn display_commands_in_config_file_warning_if_necessary(
    environment: &impl Environment,
    plugin_manifest: &PluginsManifest,
    command_names: &Vec<CommandName>,
) {
    let command_names = command_names
        .iter()
        .filter(|n| get_is_command_in_config_file(environment, &plugin_manifest, &n))
        .map(|n| n.clone())
        .collect::<Vec<_>>();

    if command_names.is_empty() {
        return;
    }

    let message = format!(
        concat!(
            "Updated globally used {0} of {2}, but local {0} {1} using version specified ",
            "in the current working directory's config file. If you wish to change the local {0}, ",
            "then update your configuration file (check the cwd and ancestor directories for a ",
            "bvm configuration file)."
        ),
        if command_names.len() == 1 {
            "version"
        } else {
            "versions"
        },
        if command_names.len() == 1 { "remains" } else { "remain" },
        utils::sentence_join(&command_names.iter().map(|n| format!("'{}'", n)).collect::<Vec<_>>()),
    );
    environment.log_error(&message);
}

fn handle_list_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment);
    let mut binaries = plugin_manifest.binaries().collect::<Vec<_>>();
    if !binaries.is_empty() {
        binaries.sort();
        let lines = binaries
            .into_iter()
            .map(|b| format!("{} {}", b.name, b.version))
            .collect::<Vec<_>>();

        environment.log(&lines.join("\n"));
    }
    Ok(())
}

fn handle_init_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let config_path = PathBuf::from(configuration::CONFIG_FILE_NAME);
    if environment.path_exists(&config_path) {
        err!(
            "A {} file already exists in the current directory.",
            configuration::CONFIG_FILE_NAME
        )
    } else if environment.path_exists(&PathBuf::from(configuration::HIDDEN_CONFIG_FILE_NAME)) {
        err!(
            "A {} file already exists in the current directory.",
            configuration::HIDDEN_CONFIG_FILE_NAME
        )
    } else {
        environment.write_file_text(&config_path, "{\n  \"binaries\": [\n  ]\n}\n")?;
        environment.log(&format!("Created {}", configuration::CONFIG_FILE_NAME));
        Ok(())
    }
}

fn handle_clear_url_cache<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment);
    plugins.clear_cached_urls();
    plugins.save()?;
    Ok(())
}

fn handle_registry_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    sub_command: RegistrySubCommand,
) -> Result<(), ErrBox> {
    match sub_command {
        RegistrySubCommand::Add(command) => handle_registry_add_command(environment, command),
        RegistrySubCommand::Remove(command) => handle_registry_remove_command(environment, command),
        RegistrySubCommand::List => handle_registry_list_command(environment),
    }
}

fn handle_add_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: AddCommand,
) -> Result<(), ErrBox> {
    let url = resolve_url_or_name(environment, &command.url_or_name)?;
    let (config_file_path, config_file) = get_config_file_or_error(environment)?;
    let mut plugins = PluginsMut::load(environment);

    // install the binary
    install_binary(&mut plugins, &url, None, false)?;
    let binary_identifier = plugins.manifest.get_identifier_from_url(&url).unwrap().clone();
    let binary_name = binary_identifier.get_binary_name();

    // get the replace index if this binary name is already in the config file
    let mut replace_index = None;
    for (i, config_binary) in config_file.binaries.iter().enumerate() {
        // ignore errors when associating
        if let Err(err) = plugins.ensure_url_associated(&config_binary.url) {
            environment.log_error(&format!(
                "Error associating {}. {}",
                &config_binary.url.unresolved_path,
                err.to_string()
            ));
        } else {
            let config_binary_name = plugins
                .manifest
                .get_identifier_from_url(&config_binary.url)
                .unwrap()
                .get_binary_name();
            if binary_name == config_binary_name {
                replace_index = Some(i);
                break;
            }
        }
    }

    // now add it to the configuration file
    let binary = plugins.manifest.get_binary(&binary_identifier).unwrap();
    let checksum = match &url.checksum {
        Some(checksum) => checksum.to_string(),
        None => {
            let url_file_bytes = environment.fetch_url(&url.url)?;
            get_sha256_checksum(&url_file_bytes)
        }
    };

    configuration::add_binary_to_config_file(
        environment,
        &config_file_path,
        &configuration::ConfigFileBinary {
            url: url.with_checksum(checksum),
            version: Some(
                match command.url_or_name {
                    UrlOrName::Url(_) => None,
                    UrlOrName::Name(name) => name.version_selector,
                }
                .unwrap_or(VersionSelector::parse(binary.version.as_str()).unwrap()),
            ),
        },
        replace_index,
    )?;

    plugins.save()?;

    Ok(())
}

fn handle_registry_add_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: RegistryAddCommand,
) -> Result<(), ErrBox> {
    let mut registry = registry::Registry::load(environment);
    let registry_file = registry::download_registry_file(environment, &command.url)?;

    // clear any previous associations if they exist
    registry.remove_url(&command.url);

    // add the current ones
    if registry_file.binaries.is_empty() {
        environment.log_error("For some reason the registry was empty. Did not associate any binaries with this url.");
    } else {
        environment.log("Associated binaries:");
        let mut previous_matches: Vec<(BinaryName, Vec<String>)> = Vec::new();

        // todo: display description of binary here
        for registry_binary in registry_file.binaries {
            let binary_name = registry_binary.get_binary_name();
            let current_urls = registry.get_urls(&binary_name.to_selector());

            if current_urls.len() > 0 {
                previous_matches.push((binary_name.clone(), current_urls.into_iter().map(|u| u.url).collect()));
            }

            environment.log(&format!("* {} - {}", binary_name, registry_binary.description));
            registry.add_url(binary_name, command.url.clone());
        }

        for (binary_name, previous_urls) in previous_matches {
            environment.log(&format!(
                "\nWARNING! This may be ok, but the '{}' binary was already associated to the following url(s): {} -- They are now all associated and binary selection will go through each registry to find a matching version.",
                binary_name,
                previous_urls.join(", ")
            ));
        }
    }

    registry.save(environment)?;
    Ok(())
}

fn handle_registry_remove_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: RegistryRemoveCommand,
) -> Result<(), ErrBox> {
    let mut registry = registry::Registry::load(environment);
    registry.remove_url(&command.url);
    registry.save(environment)?;
    Ok(())
}

fn handle_registry_list_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let registry = registry::Registry::load(environment);
    let mut items = registry.items();

    items.sort_by(|a, b| a.compare(b));

    let lines = items.into_iter().map(|item| item.display()).collect::<Vec<_>>();

    if !lines.is_empty() {
        environment.log(&lines.join("\n"));
    }
    Ok(())
}

fn handle_hidden_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: HiddenSubCommand,
) -> Result<(), ErrBox> {
    match command {
        HiddenSubCommand::ResolveCommand(command) => handle_hidden_resolve_command_command(environment, command),
        HiddenSubCommand::GetPendingEnvChanges => handle_hidden_get_pending_env_changes(environment),
        HiddenSubCommand::ClearPendingEnvChanges => handle_hidden_clear_pending_env_changes_command(environment),
        HiddenSubCommand::GetPaths => handle_hidden_get_paths_command(environment),
        HiddenSubCommand::GetEnvVars => handle_hidden_get_env_vars_command(environment),
        HiddenSubCommand::GetExecEnvChanges(command) => {
            handle_hidden_get_exec_env_changes_command(environment, command)
        }
        HiddenSubCommand::GetExecCommandPath(command) => {
            handle_hidden_get_exec_command_path_command(environment, command)
        }
        HiddenSubCommand::HasCommand(command) => handle_hidden_has_command(environment, command),
        #[cfg(not(target_os = "windows"))]
        HiddenSubCommand::UnixInstall => handle_hidden_unix_install_command(environment),
        #[cfg(target_os = "windows")]
        HiddenSubCommand::WindowsInstall => handle_hidden_windows_install_command(environment),
        #[cfg(target_os = "windows")]
        HiddenSubCommand::WindowsUninstall => handle_hidden_windows_uninstall_command(environment),
        #[cfg(target_os = "windows")]
        HiddenSubCommand::SliceArgs(command) => handle_hidden_slice_args_command(environment, command),
    }
}

fn handle_hidden_resolve_command_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: HiddenResolveCommand,
) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment);
    let command_name = command.command_name;
    let info = get_executable_path_from_config_file(environment, &plugin_manifest, &command_name)?;
    let config_file_binary_info = if let Some(info) = info {
        if let Some(binary_info) = info.binary_info {
            Some(binary_info)
        } else {
            if info.had_uninstalled_binary {
                environment.log_error(&format!(
                    "[bvm warning]: There were some not installed binaries in the current directory (run `bvm install`). Resolving global '{}'.",
                    command_name
                ));
            }
            None
        }
    } else {
        None
    };

    if let Some(binary_info) = config_file_binary_info {
        let identifier = binary_info.binary.get_identifier();
        let executable_path = binary_info.executable_path;
        let command_names = &binary_info.binary.get_command_names();

        let mut plugins = PluginsMut::from_manifest_disallow_write(environment, plugin_manifest);
        for command_name in command_names {
            plugins.use_global_version(&command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
        }

        output_pending_env_changes(environment, &plugins.manifest);
        environment.log("EXEC");
        environment.log(&executable_path.to_string_lossy());
    } else {
        let global_exe_path =
            plugin_helpers::get_global_binary_file_path(environment, &plugin_manifest, &command_name)?;
        environment.log("EXEC");
        environment.log(&global_exe_path.to_string_lossy());
    }

    Ok(())
}

fn handle_hidden_get_pending_env_changes<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment);
    output_pending_env_changes(environment, &plugin_manifest);

    Ok(())
}

fn output_pending_env_changes<TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &PluginsManifest,
) {
    let added_env_vars = plugin_manifest.get_pending_added_env_variables(environment);
    let removed_env_vars = plugin_manifest.get_pending_removed_env_variables(environment);
    let old_path = environment.get_env_path();
    let new_path = plugin_helpers::get_env_path_from_pending_env_changes(environment, &plugin_manifest);

    output_env_changes(environment, &added_env_vars, &removed_env_vars, &old_path, &new_path);
}

fn output_env_changes<TEnvironment: Environment>(
    environment: &TEnvironment,
    added_env_vars: &HashMap<String, String>,
    removed_env_vars: &HashMap<String, String>,
    old_path: &str,
    new_path: &str,
) {
    // removed
    let removed_keys = removed_env_vars.iter().map(|(key, _)| key);
    // sort to create some determinism for testing
    #[cfg(test)]
    let mut removed_keys = removed_keys.collect::<Vec<_>>();
    #[cfg(test)]
    removed_keys.sort();

    for key in removed_keys {
        if !added_env_vars.contains_key(key) {
            output_unset_env_var(environment, key);
        }
    }

    // added
    output_set_env_vars(environment, added_env_vars.iter());

    // path
    if new_path.trim_matches(';') != old_path.trim_matches(';') {
        output_set_env_var(environment, "PATH", &new_path);
    }
}

fn handle_hidden_clear_pending_env_changes_command<TEnvironment: Environment>(
    environment: &TEnvironment,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment);
    plugins.clear_pending_env_changes();
    plugins.save()?;

    Ok(())
}

fn handle_hidden_get_paths_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment);
    let path_text = plugin_manifest.get_env_paths(environment).join(SYS_PATH_DELIMITER);

    environment.log(&path_text);

    Ok(())
}

fn handle_hidden_get_env_vars_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment);
    output_set_env_vars(environment, plugin_manifest.get_env_vars(environment).iter());

    Ok(())
}

fn output_set_env_vars<'a, TEnvironment: Environment>(
    environment: &TEnvironment,
    vars: impl Iterator<Item = (&'a String, &'a String)>,
) {
    // for determinism when testing
    #[cfg(test)]
    let vars = {
        let mut vars = vars.collect::<Vec<_>>();
        vars.sort();
        vars
    };

    for (key, value) in vars {
        output_set_env_var(environment, key, value);
    }
}

fn output_set_env_var<TEnvironment: Environment>(environment: &TEnvironment, key: &str, value: &str) {
    if cfg!(target_os = "windows") {
        environment.log(&format!("SET {}={}", key, value))
    } else {
        environment.log("ADD");
        environment.log(&key);
        environment.log(&value);
    }
}

fn output_unset_env_var<TEnvironment: Environment>(environment: &TEnvironment, key: &str) {
    if cfg!(target_os = "windows") {
        environment.log(&format!("SET {}=", key));
    } else {
        environment.log("REMOVE");
        environment.log(&key);
    }
}

fn handle_hidden_get_exec_env_changes_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: HiddenExecEnvChangesCommand,
) -> Result<(), ErrBox> {
    let plugin_manifest = get_manifest_for_exec_env_changes(environment, &command)?;

    // output the pending environment changes
    output_pending_env_changes(environment, &plugin_manifest);

    Ok(())
}

fn get_manifest_for_exec_env_changes<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: &HiddenExecEnvChangesCommand,
) -> Result<PluginsManifest, ErrBox> {
    // load ensuring the changes here won't affect the system state
    let mut plugins = PluginsMut::load_disallow_write(environment);

    // go through the process of doing a "use" command
    let command_names = plugin_helpers::get_command_names_for_name_and_path_or_version_selector(
        &plugins.manifest,
        &command.name_selector,
        &command.version_selector,
    )?;
    let location = plugin_helpers::get_global_binary_location_for_name_and_path_or_version_selector(
        &plugins.manifest,
        &command.name_selector,
        &command.version_selector,
    )?;

    for command_name in command_names {
        plugins.use_global_version(&command_name, location.clone())?;
    }

    // do not save the plugins, we just want to return a manifest that has the changes above
    Ok(plugins.manifest)
}

fn handle_hidden_get_exec_command_path_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: HiddenGetExecCommandPathCommand,
) -> Result<(), ErrBox> {
    let exec_path = match &command.version_selector {
        PathOrVersionSelector::Path => {
            if let Some(exe_path) = utils::get_path_executable_path(environment, &command.command_name) {
                exe_path
            } else {
                return err!("Could not find binary on the path for the given binary name, version, and command.");
            }
        }
        PathOrVersionSelector::Version(version_selector) => {
            let plugin_manifest = PluginsManifest::load(environment);
            let binary = plugin_helpers::get_binary_with_name_and_version(
                &plugin_manifest,
                &command.name_selector,
                &version_selector,
            )?;
            match plugin_helpers::get_exec_binary_command_exe_path(environment, &binary, &command.command_name) {
                Some(exe_path) => exe_path,
                None => {
                    return err!(
                        "Could not find a matching command. Expected one of the following: {}",
                        binary
                            .get_command_names()
                            .into_iter()
                            .map(|c| c.into_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
        }
    };
    environment.log(&exec_path.to_string_lossy());

    Ok(())
}

fn handle_hidden_has_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: HiddenHasCommandCommand,
) -> Result<(), ErrBox> {
    let command_name = match command.command_name {
        Some(command_name) => command_name,
        None => {
            environment.log("false");
            return Ok(());
        }
    };
    let plugin_manifest = PluginsManifest::load(environment);
    let has_command = plugin_helpers::has_command_name_for_exec(
        environment,
        &plugin_manifest,
        &command.name_selector,
        &command.version_selector,
        &command_name,
    )?;

    environment.log(&format!("{}", has_command));
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn handle_hidden_unix_install_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    recreate_shims(environment)?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn handle_hidden_windows_install_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let data_dir = environment.get_user_data_dir();
    let bin_dir = environment.get_user_home_dir().join("bin");

    environment.ensure_system_path_pre(&PathBuf::from(&bin_dir).to_string_lossy())?;
    environment.ensure_system_path_pre(&PathBuf::from(data_dir).join("shims").to_string_lossy())?;
    recreate_shims(environment)?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn handle_hidden_windows_uninstall_command<TEnvironment: Environment>(
    environment: &TEnvironment,
) -> Result<(), ErrBox> {
    let data_dir = environment.get_user_data_dir();
    let bin_dir = environment.get_user_home_dir().join("bin");
    environment.remove_system_path(&PathBuf::from(&bin_dir).to_string_lossy())?;
    environment.remove_system_path(&PathBuf::from(data_dir).join("shims").to_string_lossy())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn handle_hidden_slice_args_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: SliceArgsCommand,
) -> Result<(), ErrBox> {
    if command.args.len() != 1 {
        return err!("Expected only 1 argument. Args: {:?}", command.args);
    }

    // This is the absolute worst, but basically the batch script calls
    // into here providing a single string and then this strips out the
    // specified number of arguments
    let arg = command.args[0].clone();

    let mut count = 0;
    let mut index = 0;
    let mut in_quotes = false;
    let mut found_var = false;
    for (i, c) in arg.char_indices() {
        index = i + c.len_utf8();

        if !found_var && c.is_whitespace() {
            continue;
        } else {
            found_var = true;
        }

        if !in_quotes {
            if c.is_whitespace() {
                found_var = false;
                count += 1;
                if count == command.count {
                    break;
                }
            } else if c == '\"' {
                in_quotes = true;
                continue;
            }
        } else {
            if c == '\"' {
                in_quotes = false;
                found_var = false;
                count += 1;
                if count == command.count {
                    break;
                }
            }
        }
    }

    let text = &arg[index..].trim();
    let text = if command.delayed_expansion {
        // escape ! for SETLOCAL EnableDelayedExpansion in batch scripts
        text.replace("!", "^^!")
    } else {
        text.to_string()
    }
    .replace("\"", "\"\"");
    environment.log(&format!("\"{}\"", text));
    Ok(())
}

fn recreate_shims(environment: &impl Environment) -> Result<(), ErrBox> {
    let shim_dir = utils::get_shim_dir(environment);
    environment.remove_dir_all(&shim_dir)?;
    environment.create_dir_all(&shim_dir)?;
    let plugin_manifest = PluginsManifest::load(environment);
    for command_name in plugin_manifest.get_all_command_names() {
        plugin_helpers::recreate_shim(environment, &plugin_manifest, &command_name)?;
    }
    Ok(())
}

struct ConfigFileExecutableInfo<'a> {
    binary_info: Option<ConfigFileBinaryInfo<'a>>,
    had_uninstalled_binary: bool,
}

struct ConfigFileBinaryInfo<'a> {
    executable_path: PathBuf,
    binary: &'a plugins::BinaryManifestItem,
}

fn get_executable_path_from_config_file<'a, TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &'a PluginsManifest,
    command_name: &CommandName,
) -> Result<Option<ConfigFileExecutableInfo<'a>>, ErrBox> {
    Ok(if let Some((_, config_file)) = get_config_file(environment)? {
        let mut had_uninstalled_binary = false;
        let mut binary_info = None;

        for config_binary in config_file.binaries.iter() {
            let binary =
                plugin_helpers::get_installed_binary_if_associated_config_file_binary(plugin_manifest, &config_binary);
            if let Some(binary) = binary {
                for command in binary.commands.iter() {
                    if &command.name == command_name {
                        let plugin_cache_dir = plugins::get_plugin_dir(environment, &binary.name, &binary.version);
                        let executable_path = plugin_cache_dir.join(&command.path);

                        binary_info = Some(ConfigFileBinaryInfo {
                            binary,
                            executable_path,
                        });

                        break;
                    }
                }
            } else {
                had_uninstalled_binary = true;
            }
        }

        Some(ConfigFileExecutableInfo {
            binary_info,
            had_uninstalled_binary,
        })
    } else {
        None
    })
}

fn get_config_file_or_error(environment: &impl Environment) -> Result<(PathBuf, configuration::ConfigFile), ErrBox> {
    match get_config_file(environment)? {
        Some(config_file) => Ok(config_file),
        None => {
            err!("Could not find a bvm configuration file in the current directory or its ancestors. Perhaps create one with `bvm init`?")
        }
    }
}

fn get_config_file(environment: &impl Environment) -> Result<Option<(PathBuf, configuration::ConfigFile)>, ErrBox> {
    if let Some(config_file_path) = configuration::find_config_file(environment)? {
        let config_file_text = environment.read_file_text(&config_file_path)?;
        let base = get_url_from_directory(config_file_path.parent().unwrap());
        match configuration::read_config_file(&config_file_text, &base) {
            Ok(file) => Ok(Some((config_file_path, file))),
            Err(err) => err!("Error reading {}: {}", config_file_path.display(), err.to_string()),
        }
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    use super::registry;
    use super::run;
    use crate::environment::{Environment, TestEnvironment, SYS_PATH_DELIMITER};
    use crate::test_builders::{EnvironmentBuilder, PluginDownloadType};
    use dprint_cli_core::types::ErrBox;

    pub const PATH_SEPARATOR: &'static str = if cfg!(target_os = "windows") { "\\" } else { "/" };

    macro_rules! assert_logs {
        ($environment:expr, []) => {
            let logged_messages = $environment.take_logged_messages();
            assert_eq!(logged_messages, Vec::<String>::new());
        };
        ($environment:expr, $messages:expr) => {
            let logged_messages = $environment.take_logged_messages();
            assert_eq!(logged_messages, $messages);
        };
    }

    macro_rules! assert_logs_errors {
        ($environment:expr, []) => {
            let errors = $environment.take_logged_errors();
            assert_eq!(errors, Vec::<String>::new());
        };
        ($environment:expr, $errors:expr) => {
            let errors = $environment.take_logged_errors();
            assert_eq!(errors, $errors);
        };
    }

    macro_rules! assert_has_path {
        ($environment:expr, $path:expr) => {
            assert_eq!($environment.path_exists(&PathBuf::from($path)), true);
        };
    }

    macro_rules! assert_not_has_path {
        ($environment:expr, $path:expr) => {
            assert_eq!($environment.path_exists(&PathBuf::from($path)), false);
        };
    }

    macro_rules! assert_resolves_name {
        ($environment:expr, $name:expr, $binary_path:expr) => {
            run_cli(vec!["hidden", "resolve-command", $name], &$environment).unwrap();
            assert_logs!($environment, ["EXEC".to_string(), $binary_path.clone()]);
        };
    }

    macro_rules! assert_resolves {
        ($environment:expr, $binary_path:expr) => {
            assert_resolves_name!($environment, "name", $binary_path)
        };
    }

    macro_rules! assert_get_paths {
        ($environment:expr, []) => {
            assert_get_paths!($environment, Vec::<String>::new());
        };
        ($environment:expr, $paths:expr) => {
            run_cli(vec!["hidden", "get-paths"], &$environment).unwrap();
            let paths_text = $paths.join(SYS_PATH_DELIMITER);
            assert_logs!($environment, [paths_text]);
        };
    }

    macro_rules! assert_get_env_vars {
        ($environment:expr, [$(($key:expr, $value:expr)),*]) => {
            run_cli(vec!["hidden", "get-env-vars"], &$environment)
                .unwrap();

            #[allow(unused_mut)]
            let mut expected_logs = Vec::<String>::new();
            if cfg!(target_os="windows") {
                $(
                    expected_logs.push(format!("SET {}={}", $key, $value));
                )*
            } else {
                $(
                    expected_logs.push("ADD".to_string());
                    expected_logs.push($key.to_string());
                    expected_logs.push($value.to_string());
                )*
            }
            assert_logs!($environment, expected_logs);
        };
    }

    macro_rules! assert_get_pending_env_changes {
        ($environment:expr, [$(($key:expr, $value:expr)),*], [$($remove_key:expr),*], $new_path:expr) => {
            run_cli(vec!["hidden", "get-pending-env-changes"], &$environment)
                .unwrap();
            assert_logged_env_changes!($environment, [$(($key, $value)),*], [$($remove_key),*], $new_path);
        };
    }

    macro_rules! assert_logged_env_changes {
        ($environment:expr, [$(($key:expr, $value:expr)),*], [$($remove_key:expr),*], $new_path:expr) => {
            let mut expected_logs = Vec::<String>::new();
            $(
                if cfg!(target_os="windows") {
                    expected_logs.push(format!("SET {}=", $remove_key));
                } else {
                    expected_logs.push("REMOVE".to_string());
                    expected_logs.push($remove_key.to_string());
                }
            )*

            $(
                if cfg!(target_os="windows") {
                    expected_logs.push(format!("SET {}={}", $key, $value));
                } else {
                    expected_logs.push("ADD".to_string());
                    expected_logs.push($key.to_string());
                    expected_logs.push($value.to_string());
                }
            )*

            if !$new_path.is_empty() {
                if cfg!(target_os="windows") {
                    expected_logs.push(format!("SET PATH={}", $new_path));
                } else {
                    expected_logs.push("ADD".to_string());
                    expected_logs.push("PATH".to_string());
                    expected_logs.push($new_path.to_string());
                }
            }

            assert_logs!($environment, expected_logs);
        };
    }

    macro_rules! assert_exec_command_path {
        ($environment:expr, $name:expr, $version:expr, $command:expr, $binary_path:expr) => {
            run_cli(
                vec!["hidden", "get-exec-command-path", $name, $version, $command],
                &$environment,
            )
            .unwrap();
            assert_logs!($environment, vec![$binary_path.clone()]);
        };
    }

    macro_rules! assert_has_command {
        ($environment:expr, $name:expr, $version:expr, $command:expr, $result:expr) => {
            run_cli(
                vec!["hidden", "has-command", $name, $version, $command],
                &$environment,
            )
            .unwrap();
            assert_logs!($environment, [$result.to_string()]);
        };
    }

    macro_rules! install_url {
        ($environment:expr, $url:expr) => {
            run_cli(vec!["install", $url], &$environment).unwrap();
        };
    }

    #[test]
    fn should_output_version() {
        let environment = TestEnvironment::new();
        run_cli(vec!["--version"], &environment).unwrap();
        assert_logs!(environment, [format!("bvm {}", env!("CARGO_PKG_VERSION"))]);
    }

    #[test]
    fn should_init() {
        let environment = TestEnvironment::new();
        run_cli(vec!["init"], &environment).unwrap();
        assert_logs!(environment, ["Created bvm.json"]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("bvm.json")).unwrap(),
            "{\n  \"binaries\": [\n  ]\n}\n"
        );
    }

    #[test]
    fn should_error_if_init_has_file() {
        let environment = TestEnvironment::new();
        environment.write_file_text(&PathBuf::from("bvm.json"), "").unwrap();
        let error_text = run_cli(vec!["init"], &environment).err().unwrap();
        assert_eq!(
            error_text.to_string(),
            "A bvm.json file already exists in the current directory."
        );
    }

    #[test]
    fn should_error_if_init_has_hidden_file() {
        let environment = TestEnvironment::new();
        environment.write_file_text(&PathBuf::from(".bvm.json"), "").unwrap();
        let error_text = run_cli(vec!["init"], &environment).err().unwrap();
        assert_eq!(
            error_text.to_string(),
            "A .bvm.json file already exists in the current directory."
        );
    }

    #[test]
    fn install_url_command_no_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let environment = builder.build();

        // install the package
        install_url!(environment, "http://localhost/package.json");
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);

        // check setup was correct
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_has_path!(environment, &binary_path);
        assert_has_path!(environment, &get_shim_path("name"));

        // try to resolve the command globally
        assert_resolves!(environment, binary_path);

        // try to use the path version, it should fail
        let error_message = run_cli(vec!["use", "name", "path"], &environment).err().unwrap();
        assert_eq!(
            error_message.to_string(),
            "Could not find any installed binaries on the path that matched 'name'."
        );
    }

    #[test]
    fn install_url_command_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let path_exe_path = builder.add_binary_to_path("name");
        let environment = builder.build();

        // install the package
        install_url!(environment, "http://localhost/package.json");
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 1.0.0...",
                "Installed. Run `bvm use name 1.0.0` to use it on the path as 'name'."
            ]
        );

        // try to resolve globally, it should use command on path
        assert_resolves!(environment, path_exe_path);

        // now use the installed version
        run_cli(vec!["use", "name", "1.0.0"], &environment).unwrap();
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // switch back to the path
        run_cli(vec!["use", "name", "path"], &environment).unwrap();
        assert_resolves!(&environment, path_exe_path);
    }

    #[test]
    fn install_url_command_previous_install() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        let third_binary_path = get_binary_path("owner", "name", "3.0.0");
        let fourth_binary_path = get_binary_path("owner", "name", "4.0.0");
        let fourth_binary_path_second = get_binary_path_second("owner", "name", "4.0.0");

        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "3.0.0");
        builder.create_remote_zip_multiple_commands_package("http://localhost/package4.json", "owner", "name", "4.0.0");
        let environment = builder.build();

        // install the first package
        install_url!(environment, "http://localhost/package.json");
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0...",]);

        // now install the second
        install_url!(environment, "http://localhost/package2.json");
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 2.0.0...",
                "Installed. Run `bvm use name 2.0.0` to use it on the path as 'name'."
            ]
        );
        assert_resolves!(&environment, first_binary_path);

        // use the second package
        run_cli(vec!["use", "name", "2.0.0"], &environment).unwrap();
        assert_resolves!(&environment, second_binary_path);

        // install the third package with --use
        run_cli(vec!["install", "--use", "http://localhost/package3.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 3.0.0...",]);
        assert_resolves!(&environment, third_binary_path);

        // install the fourth package
        install_url!(environment, "http://localhost/package4.json");
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 4.0.0...",
                "Installed. Run `bvm use name 4.0.0` to use it on the path as 'name' and 'name-second'."
            ]
        );
        assert_resolves!(&environment, third_binary_path);

        // now install the fourth package again, but with --use
        run_cli(vec!["install", "--use", "http://localhost/package4.json"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            ["Already installed. Provide the `--force` flag to reinstall."]
        );
        assert_resolves!(&environment, fourth_binary_path);
        assert_resolves_name!(&environment, "name-second", fourth_binary_path_second);

        // now install with --force
        run_cli(
            vec!["install", "--force", "http://localhost/package4.json"],
            &environment,
        )
        .unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 4.0.0...",]);
        assert_resolves!(&environment, fourth_binary_path);
        assert_resolves_name!(&environment, "name-second", fourth_binary_path_second);
    }

    #[test]
    fn install_url_command_tar_gz() {
        let builder = EnvironmentBuilder::new();
        let binary_path = get_binary_path("owner", "name", "1.0.0");

        builder.create_remote_tar_gz_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let environment = builder.build();

        // install and check setup
        install_url!(environment, "http://localhost/package.json");
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0...",]);
        assert_has_path!(environment, &binary_path);
        assert_has_path!(environment, &get_shim_path("name"));

        // yeah, this isn't realistic, but it's just some dummy data to ensure the file was extracted correctly
        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.read_file_text(&PathBuf::from(binary_path)).unwrap(),
                "test-name-https://github.com/dsherret/bvm/releases/download/1.0.0/name-windows.tar.gz"
            );
        }
    }

    #[test]
    fn install_url_command_use_with_config_file_same_command() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner2", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // install the package
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();
        environment.clear_logs();

        // install and use the other package
        run_cli(vec!["install", "--use", "http://localhost/package2.json"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner2/name 2.0.0...",
                concat!(
                    "Updated globally used version of 'name', but local version remains using version specified ",
                    "in the current working directory's config file. If you wish to change the local version, ",
                    "then update your configuration file (check the cwd and ancestor directories for a bvm ",
                    "configuration file)."
                )
            ]
        );

        // should still resolve to the cwd's binary
        assert_resolves!(&environment, first_binary_path);
    }

    #[test]
    fn install_url_pre_and_post_install() {
        let builder = EnvironmentBuilder::new();
        let first_bin_dir = get_binary_dir("owner", "name", "1.0.0");
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package.json", "owner", "name", "1.0.0");
        plugin_builder.on_pre_install("command1");
        plugin_builder.on_post_install("command2");
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let environment = builder.build();

        install_url!(environment, "http://localhost/package.json");
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment.take_run_shell_commands(),
            [
                (first_bin_dir.clone(), "command1".to_string()),
                (first_bin_dir.clone(), "command2".to_string()),
            ]
        );
    }

    #[test]
    fn install_url_output_dir() {
        let builder = EnvironmentBuilder::new();
        let first_bin_dir = get_binary_dir("owner", "name", "1.0.0");
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package1.json", "owner", "name", "1.0.0");
        plugin_builder.output_dir("bin");
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package2.json", "owner", "name", "2.0.0");
        plugin_builder.output_dir("../bin"); // should error
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package3.json", "owner", "name", "3.0.0");
        #[cfg(target_os = "windows")]
        plugin_builder.output_dir("C:\\bin"); // should error
        #[cfg(not(target_os = "windows"))]
        plugin_builder.output_dir("/bin"); // should error
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let environment = builder.build();

        install_url!(environment, "http://localhost/package1.json");
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        let output_dir = PathBuf::from(first_bin_dir).join("bin");
        let bin_name = if cfg!(target_os = "windows") {
            "name.exe"
        } else {
            "name"
        };
        assert_eq!(environment.path_exists(&output_dir.join(bin_name)), true);

        // test going down a dir
        let err_message = run_cli(vec!["install", "http://localhost/package2.json"], &environment)
            .err()
            .unwrap();
        assert_eq!(
            err_message.to_string(),
            "Error installing http://localhost/package2.json. Invalid path '../bin'. A path cannot go down directories."
        );

        // test absolute path
        let err_message = run_cli(vec!["install", "http://localhost/package3.json"], &environment)
            .err()
            .unwrap();
        let expected_error = format!(
            "Error installing http://localhost/package3.json. Invalid path '{}'. A path cannot be absolute.",
            if cfg!(target_os = "windows") { "C:\\bin" } else { "/bin" }
        );
        assert_eq!(err_message.to_string(), expected_error);
    }

    #[test]
    fn install_command_no_existing_binary() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // attempt to install in directory that doesn't have the config file
        let error_text = run_cli(vec!["install"], &environment).err().unwrap().to_string();
        assert_eq!(
            error_text,
            "Could not find a bvm configuration file in the current directory or its ancestors. Perhaps create one with `bvm init`?"
        );

        // move to the correct dir, then try again
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);

        // now try to resolve the binary
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // go up a directory and it should resolve
        environment.set_cwd("/");
        assert_resolves!(environment, binary_path);
    }

    #[test]
    fn install_command_previous_install_binary() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package2.json"]);
        let environment = builder.build();

        // install a package globally
        run_cli(vec!["install", "http://localhost/package.json"], &environment).unwrap();
        environment.clear_logs();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 2.0.0..."]);

        // now try to resolve the binary
        assert_resolves!(environment, second_binary_path);

        // try reinstalling, it should not output anything
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, []);

        // try reinstalling, but provide --force
        run_cli(vec!["install", "--force"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 2.0.0..."]);

        // go up a directory and it should resolve to the previously set global
        environment.set_cwd("/");
        assert_resolves!(environment, first_binary_path);

        // go back and provide --use
        environment.set_cwd("/project");
        run_cli(vec!["install", "--use"], &environment).unwrap();
        assert_logs_errors!(environment, []);

        // go up a directory and it should use the path from the config globally now
        environment.set_cwd("/");
        assert_resolves!(environment, second_binary_path);
    }

    #[test]
    fn install_command_binary_on_path() {
        let builder = EnvironmentBuilder::new();
        let path_exe_path = builder.add_binary_to_path("name");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);

        // now try to resolve the binary
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // go up a directory and it should resolve to binary on the path still
        environment.set_cwd("/");
        assert_resolves!(environment, path_exe_path);
    }

    #[test]
    fn install_command_pre_post_install() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .on_pre_install("echo \"Test\"")
            .on_post_install("echo \"Hello world!\"")
            .add_binary_path("http://localhost/package.json")
            .build();
        let environment = builder.build();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        let logged_shell_commands = environment.take_run_shell_commands();
        assert_eq!(
            logged_shell_commands,
            [
                ("/project".to_string(), "echo \"Test\"".to_string()),
                ("/project".to_string(), "echo \"Hello world!\"".to_string())
            ]
        );
    }

    #[test]
    fn install_command_binary_object() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object("http://localhost/package.json", None, None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
    }

    #[test]
    fn install_command_binary_object_checksum() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object("http://localhost/package.json", Some(checksum.as_str()), None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
    }

    #[test]
    fn install_command_binary_object_checksum_incorrect() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object("http://localhost/package.json", Some("incorrect-checksum"), None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        let error = run_cli(vec!["install"], &environment).err().unwrap();
        assert_eq!(error.to_string(), format!("Error installing http://localhost/package.json: The checksum {} did not match the expected checksum of incorrect-checksum.", checksum));
    }

    #[test]
    fn install_command_binary_object_url_checksum_incorrect() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json@incorrect-checksum"), None, None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        let error = run_cli(vec!["install"], &environment).err().unwrap();
        assert_eq!(error.to_string(), format!("Error installing http://localhost/package.json: The checksum {} did not match the expected checksum of incorrect-checksum.", checksum));
    }

    #[test]
    fn install_command_binary_object_existing_matching_version() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.1.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json"), None, Some("^1.0"))
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, []);
        assert_resolves!(environment, get_binary_path("owner", "name", "1.1.0"));
    }

    #[test]
    fn install_command_binary_object_existing_matching_version_major_minor() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.1.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.3.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json"), None, Some("1.1"))
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();

        // should not install because 1.1 is the same as ^1.1 in a config file
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, []);
        assert_resolves!(environment, get_binary_path("owner", "name", "1.3.0"));
    }

    #[test]
    fn install_command_binary_object_non_existing_matching_version() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.1.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json"), None, Some("~1.0"))
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        assert_resolves!(environment, get_binary_path("owner", "name", "1.0.0"));
    }

    #[test]
    fn install_command_binary_object_version_not_match_path_errors() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json"), None, Some("1.1"))
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        let err = run_cli(vec!["install"], &environment).err().unwrap();
        assert_eq!(err.to_string(), "Error installing http://localhost/package.json: The specified version '1.1' did not match '1.0.0' in the path file. Please specify a different path or update the version.");

        // should still resolve when installed without error
        install_url!(environment, "http://localhost/package.json");
        environment.clear_logs();
        assert_resolves!(environment, get_binary_path("owner", "name", "1.0.0"));
    }

    #[test]
    fn install_unknown_config_key() {
        let environment = TestEnvironment::new();
        environment
            .write_file_text(
                &PathBuf::from("/bvm.json"),
                r#"{"test": "", "binaries": ["http://localhost/package.json"]}"#,
            )
            .unwrap();

        let error_message = run_cli(vec!["install"], &environment).err().unwrap();
        assert_eq!(error_message.to_string(), "Error reading /bvm.json: Unknown key 'test'");
    }

    #[test]
    fn uninstall_command_binary_on_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let path_exe_path = builder.add_binary_to_path("name");
        let environment = builder.build();

        // install and use the package
        run_cli(vec!["install", "--use", "http://localhost/package.json"], &environment).unwrap();
        environment.clear_logs();
        assert_has_path!(environment, &get_shim_path("name"));
        run_cli(vec!["uninstall", "name", "1.0.0"], &environment).unwrap();

        // ensure it resolves the previous binary on the path
        assert_resolves!(environment, path_exe_path);
        assert_not_has_path!(environment, &get_shim_path("name"));
        assert_not_has_path!(environment, &get_binary_path("owner", "name", "1.0.0"));
    }

    #[test]
    fn uninstall_command_multiple_binaries() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_multiple_commands_package("http://localhost/package3.json", "owner", "name", "3.0.0");
        let environment = builder.build();

        // install and the first package
        install_url!(environment, "http://localhost/package.json");
        environment.clear_logs();

        // install and use the second package
        run_cli(vec!["install", "--use", "http://localhost/package2.json"], &environment).unwrap();
        environment.clear_logs();
        assert_has_path!(environment, &get_shim_path("name"));

        // now install the second package
        run_cli(vec!["uninstall", "name", "2.0.0"], &environment).unwrap();

        // ensure it resolves the first binary on the path now
        let name_dir = PathBuf::from(&first_binary_path)
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        assert_resolves!(environment, &first_binary_path);
        assert_has_path!(environment, &get_shim_path("name"));
        assert_has_path!(environment, &first_binary_path);
        assert_not_has_path!(environment, &second_binary_path);
        assert_eq!(environment.is_dir_deleted(&name_dir), false);

        // install and use the third package
        run_cli(vec!["install", "--use", "http://localhost/package3.json"], &environment).unwrap();
        environment.clear_logs();
        assert_has_path!(environment, &get_shim_path("name"));
        assert_has_path!(environment, &get_shim_path("name-second"));
        run_cli(vec!["uninstall", "name", "3.0.0"], &environment).unwrap();
        assert_has_path!(environment, &get_shim_path("name"));
        assert_not_has_path!(environment, &get_shim_path("name-second"));

        // uninstall the first package and it should no longer have the shim
        run_cli(vec!["uninstall", "name", "1.0.0"], &environment).unwrap();
        assert_not_has_path!(environment, &get_shim_path("name"));
        assert_not_has_path!(environment, &first_binary_path);
        assert_eq!(environment.is_dir_deleted(&name_dir), true);
    }

    #[test]
    fn list_command_with_no_installs() {
        let environment = TestEnvironment::new();
        run_cli(vec!["list"], &environment).unwrap();
        assert_logs!(environment, []);
    }

    #[test]
    fn list_command_with_installs() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "b", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package4.json", "owner", "name", "2.0.0"); // same version as above
        builder.create_remote_zip_package("http://localhost/package5.json", "david", "c", "2.1.1");
        let environment = builder.build();

        // install the packages
        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        install_url!(environment, "http://localhost/package4.json");
        install_url!(environment, "http://localhost/package5.json");
        environment.clear_logs();

        // check list
        run_cli(vec!["list"], &environment).unwrap();
        assert_logs!(
            environment,
            ["david/c 2.1.1\nowner/b 2.0.0\nowner/name 1.0.0\nowner/name 2.0.0"]
        );
    }

    #[test]
    fn use_command_multiple_command_binaries() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let first_binary_path_second = get_binary_path_second("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        let third_binary_path = get_binary_path("owner", "name", "2.1.0");
        let third_binary_path_second = get_binary_path_second("owner", "name", "2.1.0");
        let fourth_binary_path = get_binary_path("owner", "name", "2.1.1");

        builder.create_remote_zip_multiple_commands_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_multiple_commands_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_multiple_commands_package("http://localhost/package3.json", "owner", "name", "2.1.0");
        builder.create_remote_zip_multiple_commands_package("http://localhost/package4.json", "owner", "name", "2.1.1");
        builder.create_remote_zip_multiple_commands_package(
            "http://localhost/package5.json",
            "owner",
            "name",
            "3.1.1-alpha",
        );
        let environment = builder.build();

        // install the packages
        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        install_url!(environment, "http://localhost/package4.json");
        install_url!(environment, "http://localhost/package5.json");
        environment.clear_logs();

        assert_resolves!(&environment, first_binary_path);
        assert_resolves_name!(&environment, "name-second", first_binary_path_second);

        // specify full version
        run_cli(vec!["use", "name", "2.1.0"], &environment).unwrap();
        assert_resolves!(&environment, third_binary_path);
        assert_resolves_name!(&environment, "name-second", third_binary_path_second);

        // specify only major
        run_cli(vec!["use", "name", "2"], &environment).unwrap();
        assert_resolves!(&environment, fourth_binary_path);

        // specify minor
        run_cli(vec!["use", "name", "2.0"], &environment).unwrap();
        assert_resolves!(&environment, second_binary_path);
        run_cli(vec!["use", "name", "2.1"], &environment).unwrap();
        assert_resolves!(&environment, fourth_binary_path);

        // specify caret
        run_cli(vec!["use", "name", "^2.0"], &environment).unwrap();
        assert_resolves!(&environment, fourth_binary_path);

        // specify tilde
        run_cli(vec!["use", "name", "~2.0"], &environment).unwrap();
        assert_resolves!(&environment, second_binary_path);

        // specify none (use latest, but not pre releases)
        run_cli(vec!["use", "name"], &environment).unwrap();
        assert_resolves!(&environment, fourth_binary_path);
    }

    #[test]
    fn use_command_config_file_same_command() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner2", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // install the package
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();

        // install the other package
        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();

        // now try to use it
        run_cli(vec!["use", "name", "2.0.0"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [concat!(
                "Updated globally used version of 'name', but local version remains using version specified ",
                "in the current working directory's config file. If you wish to change the local version, ",
                "then update your configuration file (check the cwd and ancestor directories for a bvm configuration file)."
            )]
        );

        // should still resolve to the cwd's binary
        assert_resolves!(&environment, first_binary_path);
    }

    #[test]
    fn use_command_different_owners_path() {
        let builder = EnvironmentBuilder::new();

        let path_binary_path = builder.add_binary_to_path("name");
        let path_second_binary_path = builder.add_binary_to_path("name-second");

        let second_binary_path = get_binary_path("owner2", "name", "1.0.0");
        let second_binary_path_second = get_binary_path_second("owner2", "name", "1.0.0");

        builder.create_remote_zip_multiple_commands_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_multiple_commands_package(
            "http://localhost/package2.json",
            "owner2",
            "name",
            "1.0.0",
        );
        let environment = builder.build();

        // install the packages
        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();

        assert_resolves!(&environment, path_binary_path);
        assert_resolves_name!(&environment, "name-second", path_second_binary_path);

        run_cli(vec!["use", "owner2/name", "1.0.0"], &environment).unwrap();

        assert_resolves!(&environment, second_binary_path);
        assert_resolves_name!(&environment, "name-second", second_binary_path_second);

        // error when not specifying the owner and there are multiple on the path
        let err_message = run_cli(vec!["use", "name", "path"], &environment).err().unwrap();
        assert_eq!(err_message.to_string(), "There were multiple binaries with the name 'name'. Please include the owner in the name:\n  owner/name\n  owner2/name");

        // should be ok when specifying other one
        run_cli(vec!["use", "owner/name", "path"], &environment).unwrap();
        assert_resolves!(&environment, path_binary_path);
        assert_resolves_name!(&environment, "name-second", path_second_binary_path);
    }

    #[test]
    fn clear_url_cache_command_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();
        environment.set_cwd("/project");

        // install
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);

        // clear the url cache
        run_cli(vec!["clear-url-cache"], &environment).unwrap();

        // ensure it still resolves, but it will error
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);
        assert_logs_errors!(environment, ["[bvm warning]: There were some not installed binaries in the current directory (run `bvm install`). Resolving global 'name'."]);

        // install again, but it shouldn't install because already installed
        run_cli(vec!["install"], &environment).unwrap();
        assert_logs_errors!(environment, []);

        // should resolve without error now
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);
    }

    #[test]
    fn registry_add_remove_list_command_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum: "".to_string(),
                path: "https://localhost/test.json".to_string(),
            }],
        );
        builder.create_remote_registry_file(
            "http://localhost/registry2.json",
            "owner",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "2.0.0".into(),
                checksum: "".to_string(),
                path: "https://localhost/test.json".to_string(),
            }],
        );
        builder.create_remote_registry_file(
            "http://localhost/registry3.json",
            "owner2",
            "name2",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum: "".to_string(),
                path: "https://localhost/test.json".to_string(),
            }],
        );
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();
        assert_logs!(
            environment,
            ["Associated binaries:", "* owner/name - Some description."]
        );
        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap(); // add twice
        assert_logs!(
            environment,
            ["Associated binaries:", "* owner/name - Some description."]
        );
        run_cli(vec!["registry", "add", "http://localhost/registry2.json"], &environment).unwrap();
        assert_logs!(
            environment,
            [
                "Associated binaries:",
                "* owner/name - Some description.",
                "\nWARNING! This may be ok, but the 'owner/name' binary was already associated to the following url(s): http://localhost/registry.json -- They are now all associated and binary selection will go through each registry to find a matching version."
            ]
        );
        run_cli(vec!["registry", "add", "http://localhost/registry3.json"], &environment).unwrap();
        assert_logs!(
            environment,
            ["Associated binaries:", "* owner2/name2 - Some description."]
        );
        run_cli(vec!["registry", "list"], &environment).unwrap();
        assert_logs!(environment, ["owner/name - http://localhost/registry.json\nowner/name - http://localhost/registry2.json\nowner2/name2 - http://localhost/registry3.json"]);
        run_cli(
            vec!["registry", "remove", "http://localhost/registry.json"],
            &environment,
        )
        .unwrap();
        run_cli(
            vec!["registry", "remove", "http://localhost/registry.json"],
            &environment,
        )
        .unwrap(); // remove twice should silently ignore
        run_cli(vec!["registry", "list"], &environment).unwrap();
        assert_logs!(
            environment,
            ["owner/name - http://localhost/registry2.json\nowner2/name2 - http://localhost/registry3.json"]
        );
        run_cli(
            vec!["registry", "remove", "http://localhost/registry2.json"],
            &environment,
        )
        .unwrap();
        run_cli(vec!["registry", "list"], &environment).unwrap();
        assert_logs!(environment, ["owner2/name2 - http://localhost/registry3.json"]);
        run_cli(
            vec!["registry", "remove", "http://localhost/registry3.json"],
            &environment,
        )
        .unwrap();
        run_cli(vec!["registry", "list"], &environment).unwrap();
        assert_logs!(environment, []);
    }

    #[test]
    fn registry_install_command() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let checksum2 = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.0.1");
        let checksum3 = builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "1.1.0");
        let checksum4 = builder.create_remote_zip_package("http://localhost/binary.json", "other", "name", "1.0.0");
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![
                registry::RegistryVersionInfo {
                    version: "1.0.0".into(),
                    checksum,
                    path: "http://localhost/package.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "1.0.1".into(),
                    checksum: checksum2,
                    path: "http://localhost/package2.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "1.1.0".into(),
                    checksum: checksum3,
                    path: "http://localhost/package3.json".to_string(),
                },
            ],
        );
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();

        assert_logs!(
            environment,
            vec!["Associated binaries:", "* owner/name - Some description."]
        );

        run_cli(vec!["install", "name", "1.0.0"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);

        // install latest when only specifying major
        run_cli(vec!["install", "name", "1"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 1.1.0...",
                "Installed. Run `bvm use name 1.1.0` to use it on the path as 'name'.",
            ]
        );

        // install latest patch when specifying minor
        run_cli(vec!["install", "name", "1.0"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 1.0.1...",
                "Installed. Run `bvm use name 1.0.1` to use it on the path as 'name'.",
            ]
        );

        run_cli(vec!["uninstall", "name", "1.0.1"], &environment).unwrap();
        run_cli(vec!["uninstall", "name", "1.1.0"], &environment).unwrap();
        environment.clear_logs();

        // install when specifying caret
        run_cli(vec!["install", "name", "^1.0.0"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 1.1.0...",
                "Installed. Run `bvm use name 1.1.0` to use it on the path as 'name'.",
            ]
        );

        // install when specifying tilde
        run_cli(vec!["install", "name", "~1.0.0"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for owner/name 1.0.1...",
                "Installed. Run `bvm use name 1.0.1` to use it on the path as 'name'.",
            ]
        );

        // clear up the state
        run_cli(vec!["uninstall", "name", "1.1.0"], &environment).unwrap();
        run_cli(vec!["uninstall", "name", "1.0.1"], &environment).unwrap();
        environment.clear_logs();

        // now update the registry file to have a different binary
        let builder = EnvironmentBuilder::new();
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "other",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum: checksum4,
                path: "http://localhost/binary.json".to_string(),
            }],
        );
        let new_file_bytes = builder
            .build()
            .download_file(&"http://localhost/registry.json")
            .unwrap();
        environment.add_remote_file(&"http://localhost/registry.json", new_file_bytes);

        // attempt to install the previous binary by name only and it should not exist (since we haven't reassociated the registry with the new name)
        let err_message = run_cli(vec!["install", "name", "1.0.0"], &environment).err().unwrap();
        assert_eq!(
            err_message.to_string(),
            "Could not find binary 'name' matching '1.0.0' in any registry."
        );

        // now reassociate
        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();

        assert_logs!(
            environment,
            ["Associated binaries:", "* other/name - Some description."]
        );

        // and install
        run_cli(vec!["install", "name", "1"], &environment).unwrap();
        assert_logs_errors!(
            environment,
            [
                "Extracting archive for other/name 1.0.0...",
                "Installed. Run `bvm use other/name 1.0.0` to use it on the path as 'name'.",
            ]
        );
    }

    #[test]
    fn registry_install_command_latest() {
        let builder = EnvironmentBuilder::new();
        let checksum1 = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let checksum2 = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        let checksum3 = builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "2.0.1");
        let checksum4 =
            builder.create_remote_zip_package("http://localhost/package4.json", "owner", "name", "3.0.0-alpha");
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![
                registry::RegistryVersionInfo {
                    version: "1.0.0".into(),
                    checksum: checksum1,
                    path: "http://localhost/package.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "2.0.1".into(),
                    checksum: checksum3,
                    path: "http://localhost/package3.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "2.0.0".into(),
                    checksum: checksum2,
                    path: "http://localhost/package2.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "3.0.0-alpha".into(),
                    checksum: checksum4,
                    path: "http://localhost/package4.json".to_string(),
                },
            ],
        );
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();
        environment.clear_logs();

        run_cli(vec!["install", "name"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 2.0.1..."]);
    }

    #[test]
    fn registry_install_command_latest_all_pre_releases() {
        let builder = EnvironmentBuilder::new();
        let checksum1 =
            builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0-alpha");
        let checksum2 =
            builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.0.0-beta");
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![
                registry::RegistryVersionInfo {
                    version: "1.0.0-beta".into(),
                    checksum: checksum2,
                    path: "http://localhost/package2.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "1.0.0-alpha".into(),
                    checksum: checksum1,
                    path: "http://localhost/package.json".to_string(),
                },
            ],
        );
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();
        environment.clear_logs();

        run_cli(vec!["install", "name"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0-beta..."]);
    }

    #[test]
    fn registry_install_command_incorrect_checksum() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum: "wrong-checksum".to_string(),
                path: "http://localhost/package.json".to_string(),
            }],
        );
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();
        environment.clear_logs();

        let err = run_cli(vec!["install", "name", "1.0.0"], &environment).err().unwrap();
        assert_eq!(
            err.to_string(),
            format!(
                "Error installing http://localhost/package.json. The checksum {} did not match the expected checksum of wrong-checksum.",
                checksum
            )
        );
    }

    #[test]
    fn registry_install_command_no_registry() {
        let environment = TestEnvironment::new();
        let err = run_cli(vec!["install", "name", "1.0.0"], &environment).err().unwrap();
        assert_eq!(
            err.to_string(),
            "There were no registries found for the provided binary. Did you mean to add one using `bvm registry add <url>`?",
        );
    }

    #[test]
    fn registry_install_command_multiple_owners() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum,
                path: "http://localhost/package.json".to_string(),
            }],
        );

        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner2", "name", "1.0.0");
        builder.create_remote_registry_file(
            "http://localhost/registry2.json",
            "owner2",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum,
                path: "http://localhost/package2.json".to_string(),
            }],
        );
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();
        run_cli(vec!["registry", "add", "http://localhost/registry2.json"], &environment).unwrap();
        environment.clear_logs();

        let error = run_cli(vec!["install", "name", "1.0.0"], &environment).err().unwrap();
        assert_eq!(error.to_string(), "There were multiple binaries with the name 'name'. Please include the owner in the name:\n  owner/name\n  owner2/name");
    }

    #[test]
    fn binary_has_environment_path_and_variable() {
        let builder = EnvironmentBuilder::new();
        builder.add_binary_to_path("name");
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package.json", "owner", "name", "1.0.0");
        plugin_builder.add_env_path("dir");
        plugin_builder.add_env_var("test", "1");
        #[cfg(target_os = "windows")]
        plugin_builder.add_env_var("other", "%BVM_CURRENT_BINARY_DIR%\\dir");
        #[cfg(not(target_os = "windows"))]
        plugin_builder.add_env_var("other", "$BVM_CURRENT_BINARY_DIR/dir");
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package2.json", "owner", "name", "2.0.0");
        plugin_builder.add_env_path("dir2");
        plugin_builder.add_env_path(&format!("other{}path", PATH_SEPARATOR));
        plugin_builder.add_env_var("test", "2");
        plugin_builder.download_type(PluginDownloadType::TarGz);
        plugin_builder.build();
        builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "3.0.0");
        let environment = builder.build();
        let original_path = environment.get_env_path();

        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        environment.clear_logs();

        run_cli(vec!["use", "name", "1.0.0"], &environment).unwrap();

        let first_path_str = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\1.0.0\\dir"
        } else {
            "/local-data/binaries/owner/name/1.0.0/dir"
        };
        let second_path_str1 = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\2.0.0\\dir2"
        } else {
            "/local-data/binaries/owner/name/2.0.0/dir2"
        };
        let second_path_str2 = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\2.0.0\\other\\path"
        } else {
            "/local-data/binaries/owner/name/2.0.0/other/path"
        };

        // check pending environment state
        assert_get_pending_env_changes!(
            environment,
            [("other", first_path_str), ("test", "1")],
            [],
            format!("{}{}{}", original_path, SYS_PATH_DELIMITER, first_path_str)
        );

        // only windows will have updated the environment path and variables
        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/bin"),
                    PathBuf::from("/path-dir"),
                    PathBuf::from(&first_path_str)
                ]
            );
            assert_eq!(
                environment.get_sys_env_variables(),
                [
                    ("other".to_string(), first_path_str.to_string()),
                    ("test".to_string(), "1".to_string())
                ]
            );
        } else {
            assert_eq!(environment.get_sys_env_variables(), []);
        }

        // should output correctly when the path ends with delimiter
        environment.set_env_path(&format!("{}{}", original_path, SYS_PATH_DELIMITER));
        assert_get_pending_env_changes!(
            environment,
            [("other", first_path_str), ("test", "1")],
            [],
            format!("{}{}{}", original_path, SYS_PATH_DELIMITER, first_path_str)
        );

        // update with the current environment settings
        update_with_pending_env_changes(&environment);

        // now this should output nothing
        assert_get_pending_env_changes!(environment, [], [], "");

        // ensure this exists in get-paths and get-env-vars
        assert_get_paths!(environment, [first_path_str]);
        assert_get_env_vars!(environment, [("other", first_path_str), ("test", "1")]);

        // now switch
        run_cli(vec!["use", "name", "2.0.0"], &environment).unwrap();

        // check pending environment state
        assert_get_pending_env_changes!(
            environment,
            [("test", "2")],
            ["other"],
            format!(
                "{0}{1}{2}{1}{3}",
                original_path, SYS_PATH_DELIMITER, second_path_str1, second_path_str2
            )
        );

        // update with the current environment settings
        update_with_pending_env_changes(&environment);

        // ensure the paths exist in get-paths now and env-vars in get-env-vars
        assert_get_paths!(environment, [second_path_str1, second_path_str2]);
        assert_get_env_vars!(environment, [("test", "2")]);

        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/bin"),
                    PathBuf::from("/path-dir"),
                    PathBuf::from(&second_path_str1),
                    PathBuf::from(&second_path_str2)
                ]
            );
            assert_eq!(
                environment.get_sys_env_variables(),
                [("test".to_string(), "2".to_string())]
            );
        }

        // now switch
        run_cli(vec!["use", "name", "3.0.0"], &environment).unwrap();

        // check pending environment state
        assert_get_pending_env_changes!(environment, [], ["test"], original_path);

        // update with the current environment settings
        update_with_pending_env_changes(&environment);

        // ensure all pending environment changes are empty
        assert_get_paths!(environment, []);
        assert_get_pending_env_changes!(environment, [], [], "");

        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/bin"),
                    PathBuf::from("/path-dir")
                ]
            );
            assert_eq!(environment.get_sys_env_variables(), []);
        }

        // use the path version then go back to the first
        run_cli(vec!["use", "name", "path"], &environment).unwrap();
        run_cli(vec!["use", "name", "1.0.0"], &environment).unwrap();

        assert_get_pending_env_changes!(
            environment,
            [("other", first_path_str), ("test", "1")],
            [],
            format!("{}{}{}", original_path, SYS_PATH_DELIMITER, first_path_str)
        );
    }

    #[test]
    fn binary_path_var_absolute_and_relative() {
        let builder = EnvironmentBuilder::new();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package.json", "owner", "name", "4.0.0");
        #[cfg(target_os = "windows")]
        plugin_builder.add_env_path("%BVM_CURRENT_BINARY_DIR%\\dir");
        #[cfg(not(target_os = "windows"))]
        plugin_builder.add_env_path("$BVM_CURRENT_BINARY_DIR/dir");
        plugin_builder.add_env_path("/absolute"); // absolute should stay absolute
        plugin_builder.add_env_path("relative");
        plugin_builder.download_type(PluginDownloadType::TarGz);
        plugin_builder.build();
        let environment = builder.build();
        let original_path = environment.get_env_path();

        install_url!(environment, "http://localhost/package.json");
        environment.clear_logs();

        let bin_dir = PathBuf::from(get_binary_dir("owner", "name", "4.0.0"));
        let first_path = bin_dir.join("dir").to_string_lossy().to_string();
        let second_path = "/absolute";
        let third_path = bin_dir.join("relative").to_string_lossy().to_string();

        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/bin"),
                    PathBuf::from(&first_path),
                    PathBuf::from(&second_path),
                    PathBuf::from(&third_path),
                ]
            );
        }

        assert_get_pending_env_changes!(
            environment,
            [],
            [],
            format!(
                "{1}{0}{2}{0}{3}{0}{4}",
                SYS_PATH_DELIMITER, original_path, first_path, second_path, third_path
            )
        );
    }

    #[test]
    fn add_command_url_no_binaries() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_bvmrc_builder().build();
        let environment = builder.build();

        // run the add command
        environment.set_cwd("/project");
        run_cli(vec!["add", "http://localhost/package.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/project/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/package.json",
      "checksum": "{}",
      "version": "1.0.0"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn add_command_url_other_binary() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "2.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/other.json"), None, Some("~1.1"))
            .build();
        let environment = builder.build();

        // run the add command
        environment.set_cwd("/project");
        run_cli(vec!["add", "http://localhost/package.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/project/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/other.json",
      "version": "~1.1"
    }},
    {{
      "path": "http://localhost/package.json",
      "checksum": "{}",
      "version": "1.0.0"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn add_command_registry() {
        let builder = EnvironmentBuilder::new();
        let checksum1 = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let checksum2 = builder.create_remote_zip_package("http://localhost/other1.json", "owner", "other", "1.0.0");
        let checksum3 = builder.create_remote_zip_package("http://localhost/other2.json", "owner", "other", "2.0.0");
        builder.create_remote_registry_file(
            "http://localhost/registry1.json",
            "owner",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum: checksum1.clone(),
                path: "http://localhost/package.json".to_string(),
            }],
        );
        builder.create_remote_registry_file(
            "http://localhost/registry2.json",
            "owner",
            "other",
            vec![
                registry::RegistryVersionInfo {
                    version: "1.0.0".into(),
                    checksum: checksum2.clone(),
                    path: "http://localhost/other1.json".to_string(),
                },
                registry::RegistryVersionInfo {
                    version: "2.0.0".into(),
                    checksum: checksum3.clone(),
                    path: "http://localhost/other2.json".to_string(),
                },
            ],
        );
        builder.create_bvmrc_builder().path("/bvm.json").build();
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry1.json"], &environment).unwrap();
        run_cli(vec!["registry", "add", "http://localhost/registry2.json"], &environment).unwrap();
        environment.clear_logs();

        run_cli(vec!["add", "owner/name", "1.0.0"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);

        run_cli(vec!["add", "other"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/other 2.0.0..."]);

        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/package.json",
      "checksum": "{}",
      "version": "1.0.0"
    }},
    {{
      "path": "http://localhost/other2.json",
      "checksum": "{}",
      "version": "2.0.0"
    }}
  ]
}}
"#,
                checksum1, checksum3
            )
        );

        // now say to use ~1.0 and it should replace that in the file
        run_cli(vec!["add", "other", "~1.0"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/other 1.0.0..."]);

        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/package.json",
      "checksum": "{}",
      "version": "1.0.0"
    }},
    {{
      "path": "http://localhost/other1.json",
      "checksum": "{}",
      "version": "~1.0"
    }}
  ]
}}
"#,
                checksum1, checksum2
            )
        );

        // specify a version that doesn't exist and it should error
        let err = run_cli(vec!["add", "other", "~1.1"], &environment).err().unwrap();
        assert_eq!(
            err.to_string(),
            "Could not find binary 'other' matching '~1.1' in any registry."
        );
    }

    #[test]
    fn add_command_existing_url() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_registry_file(
            "http://localhost/registry.json",
            "owner",
            "name",
            vec![registry::RegistryVersionInfo {
                version: "1.0.0".into(),
                checksum: checksum.clone(),
                path: "http://localhost/package.json".to_string(),
            }],
        );
        builder
            .create_bvmrc_builder()
            .path("/bvm.json")
            .add_binary_path("http://localhost/package.json")
            .build();
        let environment = builder.build();
        run_cli(vec!["install"], &environment).unwrap();
        environment.clear_logs();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment).unwrap();
        environment.clear_logs();
        run_cli(vec!["add", "name", "1"], &environment).unwrap();

        // should replace it
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/package.json",
      "checksum": "{}",
      "version": "1"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn add_command_existing_package_different_url_replaces() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/bvm.json")
            .add_binary_path("http://localhost/package.json")
            .build();
        let environment = builder.build();

        // should also associate the existing url if not associated
        run_cli(vec!["add", "http://localhost/package2.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/package2.json",
      "checksum": "{}",
      "version": "1.0.0"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn add_command_existing_package_different_url_replaces_start() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "1.0.0");
        builder.create_remote_zip_package("http://localhost/final.json", "owner", "final", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/bvm.json")
            .add_binary_object("http://localhost/package1.json", None, None)
            .add_binary_object("http://localhost/other.json", None, Some("~1.0.0"))
            .add_binary_object("http://localhost/final.json", None, Some("1"))
            .build();
        let environment = builder.build();

        run_cli(vec!["add", "http://localhost/package2.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 2.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/package2.json",
      "checksum": "{}",
      "version": "2.0.0"
    }},
    {{
      "path": "http://localhost/other.json",
      "version": "~1.0.0"
    }},
    {{
      "path": "http://localhost/final.json",
      "version": "1"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn add_command_existing_package_different_url_replaces_middle() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "1.0.0");
        builder.create_remote_zip_package("http://localhost/final.json", "owner", "final", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/bvm.json")
            .add_binary_object("http://localhost/other.json", None, Some("~1.0.0"))
            .add_binary_object("http://localhost/package1.json", None, None)
            .add_binary_object("http://localhost/final.json", None, Some("1"))
            .build();
        let environment = builder.build();

        run_cli(vec!["add", "http://localhost/package2.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 2.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/other.json",
      "version": "~1.0.0"
    }},
    {{
      "path": "http://localhost/package2.json",
      "checksum": "{}",
      "version": "2.0.0"
    }},
    {{
      "path": "http://localhost/final.json",
      "version": "1"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn add_command_existing_package_different_url_replaces_end() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "1.0.0");
        builder.create_remote_zip_package("http://localhost/final.json", "owner", "final", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/bvm.json")
            .add_binary_object("http://localhost/other.json", None, Some("~1.0.0"))
            .add_binary_object("http://localhost/final.json", None, Some("1"))
            .add_binary_object("http://localhost/package1.json", None, None)
            .build();
        let environment = builder.build();

        run_cli(vec!["add", "http://localhost/package2.json"], &environment).unwrap();
        assert_logs_errors!(environment, ["Extracting archive for owner/name 2.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/bvm.json")).unwrap(),
            format!(
                r#"{{
  "binaries": [
    {{
      "path": "http://localhost/other.json",
      "version": "~1.0.0"
    }},
    {{
      "path": "http://localhost/final.json",
      "version": "1"
    }},
    {{
      "path": "http://localhost/package2.json",
      "checksum": "{}",
      "version": "2.0.0"
    }}
  ]
}}
"#,
                checksum
            )
        );
    }

    #[test]
    fn get_exec_env_path_gets() {
        let builder = EnvironmentBuilder::new();
        builder.add_binary_to_path("name");
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package.json", "owner", "name", "1.0.0");
        plugin_builder.add_env_path("dir");
        plugin_builder.add_env_var("test", "1");
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package2.json", "owner", "name", "2.0.0");
        plugin_builder.add_env_path("dir2");
        plugin_builder.add_env_path(&format!("other{}path", PATH_SEPARATOR));
        plugin_builder.add_env_var("test", "2");
        plugin_builder.download_type(PluginDownloadType::TarGz);
        plugin_builder.build();
        let environment = builder.build();
        let original_path = environment.get_env_path();

        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();

        let first_path_str = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\1.0.0\\dir"
        } else {
            "/local-data/binaries/owner/name/1.0.0/dir"
        };
        let second_path_str1 = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\2.0.0\\dir2"
        } else {
            "/local-data/binaries/owner/name/2.0.0/dir2"
        };
        let second_path_str2 = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\2.0.0\\other\\path"
        } else {
            "/local-data/binaries/owner/name/2.0.0/other/path"
        };

        // should get the environment changes for the provided version
        run_cli(vec!["hidden", "get-exec-env-changes", "name", "1"], &environment).unwrap();
        let first_bin_env_path = format!("{}{}{}", original_path, SYS_PATH_DELIMITER, first_path_str);
        assert_logged_env_changes!(environment, [("test", "1")], [], first_bin_env_path);

        // the path should remain the same
        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/bin"),
                    PathBuf::from("/path-dir"),
                ]
            );
            assert_eq!(environment.get_sys_env_variables().is_empty(), true);
        }

        // test executing the currently used binary
        run_cli(vec!["use", "name", "1.0.0"], &environment).unwrap();
        update_with_pending_env_changes(&environment);

        run_cli(vec!["hidden", "get-exec-env-changes", "name", "1.0.0"], &environment).unwrap();
        assert_logged_env_changes!(environment, [("test", 1)], [], "");

        // test executing binaries with different paths
        run_cli(vec!["hidden", "get-exec-env-changes", "name", "^2"], &environment).unwrap();
        assert_logged_env_changes!(
            environment,
            [("test", "2")],
            [],
            format!(
                "{1}{0}{2}{0}{3}",
                SYS_PATH_DELIMITER, original_path, second_path_str1, second_path_str2
            )
        );

        // test getting the one on the path
        run_cli(vec!["hidden", "get-exec-env-changes", "name", "path"], &environment).unwrap();
        assert_logged_env_changes!(environment, [], ["test"], original_path);
    }

    #[test]
    fn get_exec_command_path_and_has_command() {
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        let third_binary_dir_sub_path_dir = PathBuf::from(get_binary_dir("owner", "name", "2.1.0")).join("path-dir");
        let third_binary_path = get_binary_path("owner", "name", "2.1.0");
        let fourth_binary_path = get_binary_path("owner", "other", "1.0.0");
        let fourth_binary_path_second = get_binary_path_second("owner", "other", "1.0.0");
        let path_binary_path = get_path_binary_path("name");

        let builder = EnvironmentBuilder::new();
        builder.add_binary_to_path("name");
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder
            .create_plugin_builder("http://localhost/package3.json", "owner", "name", "2.1.0")
            .download_type(PluginDownloadType::Zip)
            .add_env_path("path-dir")
            .build();
        builder.create_remote_zip_multiple_commands_package("http://localhost/other.json", "owner", "other", "1.0.0");
        let environment = builder.build();

        install_url!(environment, "http://localhost/package1.json");
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        install_url!(environment, "http://localhost/other.json");
        environment.clear_logs();

        // create an executable inside the third binary's env path
        let third_binary_path_command_path =
            third_binary_dir_sub_path_dir.join(get_executable_file_name("path-command"));
        environment
            .write_file_text(&third_binary_path_command_path, "")
            .unwrap();
        let third_binary_path_command_path = third_binary_path_command_path.to_string_lossy().to_string();

        // test the exec command
        assert_exec_command_path!(environment, "name", "path", "name", path_binary_path);
        assert_exec_command_path!(environment, "name", "1", "name", first_binary_path);
        assert_exec_command_path!(environment, "name", "2", "name", third_binary_path);
        assert_exec_command_path!(environment, "name", "2.0", "name", second_binary_path);
        assert_exec_command_path!(environment, "name", "^2.0", "name", third_binary_path);
        assert_exec_command_path!(
            environment,
            "name",
            "2.1.0",
            "path-command",
            third_binary_path_command_path
        );
        assert_exec_command_path!(environment, "other", "*", "other", fourth_binary_path);
        assert_exec_command_path!(environment, "other", "*", "other-second", fourth_binary_path_second);
        let err_message = run_cli(
            vec!["hidden", "get-exec-command-path", "other", "*", "something"],
            &environment,
        )
        .err()
        .unwrap();
        assert_eq!(
            err_message.to_string(),
            "Could not find a matching command. Expected one of the following: other, other-second"
        );

        // test the has-command command
        assert_has_command!(environment, "name", "1", "name", true);
        assert_has_command!(environment, "name", "1", "other", false);
        assert_has_command!(environment, "name", "1", "-v", false);
        assert_has_command!(environment, "name", "2.1.0", "path-command", true);
        assert_has_command!(environment, "other", "*", "other", true);
        assert_has_command!(environment, "other", "*", "other-second", true);
        assert_has_command!(environment, "other", "*", "other-second2", false);

        // check when missing last argument
        run_cli(vec!["hidden", "has-command", "name", "1"], &environment).unwrap();
        assert_logs!(environment, ["false"]);
    }

    #[test]
    fn hidden_resolve_command() {
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let builder = EnvironmentBuilder::new();
        let first_path_str = if cfg!(target_os = "windows") {
            "/local-data\\binaries\\owner\\name\\1.0.0\\dir"
        } else {
            "/local-data/binaries/owner/name/1.0.0/dir"
        };
        builder
            .create_plugin_builder("http://localhost/package.json", "owner", "name", "1.0.0")
            .download_type(PluginDownloadType::Zip)
            .add_env_var("test", "1")
            .add_env_path("dir")
            .build();
        builder
            .create_plugin_builder("http://localhost/package2.json", "owner", "name", "2.0.0")
            .download_type(PluginDownloadType::Zip)
            .add_env_var("other", "1")
            .build();
        builder
            .create_bvmrc_builder()
            .path("/bvm.json")
            .add_binary_path("http://localhost/package.json")
            .build();
        let environment = builder.build();
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package.json");
        environment.clear_logs();

        run_cli(vec!["hidden", "resolve-command", "name"], &environment).unwrap();

        let mut expected_logs = get_env_change_logs(
            &[("test", "1")],
            &["other"],
            &format!("/data/shims{0}/bin{0}{1}", SYS_PATH_DELIMITER, &first_path_str),
        );
        expected_logs.push("EXEC".to_string());
        expected_logs.push(first_binary_path);
        assert_eq!(environment.take_logged_messages(), expected_logs);
    }

    #[test]
    fn support_hidden_config_file() {
        let builder = EnvironmentBuilder::new();
        builder
            .create_bvmrc_builder()
            .path("/project/.bvm.json")
            .add_binary_path("http://localhost/package.json")
            .build();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");

        let environment = builder.build();

        // install the package
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).unwrap();
        environment.clear_logs();

        // should still resolve to the cwd's binary
        assert_resolves!(&environment, first_binary_path);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn unix_install_command() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let environment = builder.build();

        install_url!(environment, "http://localhost/package1.json");
        environment.clear_logs();

        let shim_path = PathBuf::from(get_shim_path("name"));
        assert_eq!(environment.path_exists(&shim_path), true);

        environment.remove_file(&shim_path).unwrap();
        run_cli(vec!["hidden", "unix-install"], &environment).unwrap();

        // should have recreated the shim
        assert_eq!(environment.path_exists(&shim_path), true);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_install_command_installs() {
        let environment = TestEnvironment::new();
        environment.remove_system_path("/bin").unwrap();
        environment.remove_system_path("/data/shims").unwrap();
        run_cli(vec!["hidden", "windows-install"], &environment).unwrap();
        assert_eq!(
            environment.get_system_path_dirs(),
            [PathBuf::from("/data\\shims"), PathBuf::from("/.bvm\\bin"),]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_install_command_existing_paths_installs() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let environment = builder.build();
        environment.remove_system_path("/bin").unwrap();

        install_url!(environment, "http://localhost/package1.json");
        environment.clear_logs();

        let shim_path = PathBuf::from(get_shim_path("name"));
        assert_eq!(environment.path_exists(&shim_path), true);

        environment.remove_file(&shim_path).unwrap();
        environment.remove_system_path("/data/shims").unwrap();
        environment.ensure_system_path_pre("/data\\shims").unwrap();
        environment.ensure_system_path_pre("/other-dir").unwrap();
        run_cli(vec!["hidden", "windows-install"], &environment).unwrap();
        assert_eq!(
            environment.get_system_path_dirs(),
            [
                PathBuf::from("/data\\shims"),
                PathBuf::from("/.bvm\\bin"),
                PathBuf::from("/other-dir")
            ]
        );
        // should have recreated the shim
        assert_eq!(environment.path_exists(&shim_path), true);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_uninstall_command_uninstalls() {
        let environment = TestEnvironment::new();
        environment.remove_system_path("/data/shims").unwrap();
        environment.ensure_system_path_pre("/other-dir").unwrap();
        environment.ensure_system_path_pre("/data\\shims").unwrap();
        environment.ensure_system_path_pre("/.bvm\\bin").unwrap();
        run_cli(vec!["hidden", "windows-uninstall"], &environment).unwrap();
        assert_eq!(
            environment.get_system_path_dirs(),
            [PathBuf::from("/other-dir"), PathBuf::from("/bin")]
        );
    }

    fn get_env_change_logs(added: &[(&str, &str)], removed: &[&str], new_path: &str) -> Vec<String> {
        let mut expected_logs = Vec::new();
        for remove_key in removed {
            if cfg!(target_os = "windows") {
                expected_logs.push(format!("SET {}=", remove_key));
            } else {
                expected_logs.push("REMOVE".to_string());
                expected_logs.push(remove_key.to_string());
            }
        }

        for (key, value) in added {
            if cfg!(target_os = "windows") {
                expected_logs.push(format!("SET {}={}", key, value));
            } else {
                expected_logs.push("ADD".to_string());
                expected_logs.push(key.to_string());
                expected_logs.push(value.to_string());
            }
        }

        if !new_path.is_empty() {
            if cfg!(target_os = "windows") {
                expected_logs.push(format!("SET PATH={}", new_path));
            } else {
                expected_logs.push("ADD".to_string());
                expected_logs.push("PATH".to_string());
                expected_logs.push(new_path.to_string());
            }
        }

        expected_logs
    }

    fn get_shim_path(name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/data/shims/{}.bat", name)
        } else {
            format!("/shims/{}", name)
        }
    }

    fn get_binary_dir(owner: &str, name: &str, version: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/local-data\\binaries\\{}\\{}\\{}", owner, name, version)
        } else {
            format!("/local-data/binaries/{}/{}/{}", owner, name, version)
        }
    }

    fn get_binary_path(owner: &str, name: &str, version: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/local-data\\binaries\\{}\\{}\\{}\\{1}.exe", owner, name, version)
        } else {
            format!("/local-data/binaries/{}/{}/{}/{1}", owner, name, version)
        }
    }

    fn get_path_binary_path(name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/path-dir\\{}.bat", name)
        } else {
            format!("/path-dir/{}", name)
        }
    }

    fn get_binary_path_second(owner: &str, name: &str, version: &str) -> String {
        if cfg!(target_os = "windows") {
            format!(
                "/local-data\\binaries\\{}\\{}\\{}\\{1}-second.exe",
                owner, name, version
            )
        } else {
            format!("/local-data/binaries/{}/{}/{}/{1}-second", owner, name, version)
        }
    }

    fn get_executable_file_name(command_name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("{}.exe", command_name)
        } else {
            command_name.to_string()
        }
    }

    fn update_with_pending_env_changes(environment: &TestEnvironment) {
        use super::PluginsManifest;
        let plugin_manifest = PluginsManifest::load(environment);

        for (key, _) in plugin_manifest.get_pending_removed_env_variables(environment) {
            environment.remove_env_var(&key);
        }

        for (key, value) in plugin_manifest.get_pending_added_env_variables(environment) {
            environment.set_env_var(&key, &value);
        }
        let new_path = super::plugin_helpers::get_env_path_from_pending_env_changes(environment, &plugin_manifest);
        environment.set_env_path(&new_path);

        run_cli(vec!["hidden", "clear-pending-env-changes"], &environment).unwrap();
    }

    fn run_cli(args: Vec<&str>, environment: &TestEnvironment) -> Result<(), ErrBox> {
        let mut args: Vec<String> = args.into_iter().map(String::from).collect();
        args.insert(0, String::from(""));
        run(environment, args)
    }
}
