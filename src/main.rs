#[macro_use]
mod types;
mod arg_parser;
mod configuration;
mod plugins;
mod utils;

use arg_parser::*;
use types::{BinaryName, CommandName, ErrBox};

#[tokio::main]
async fn main() -> Result<(), ErrBox> {
    match run().await {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err.to_string());
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run() -> Result<(), ErrBox> {
    let args = parse_args(std::env::args().collect())?;

    match args.sub_command {
        SubCommand::Help(text) => print!("{}", text),
        SubCommand::Version => println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION")),
        SubCommand::Resolve(resolve_command) => handle_resolve_command(resolve_command)?,
        SubCommand::Install => handle_install_command().await?,
        SubCommand::InstallUrl(url) => handle_install_url_command(url).await?,
        SubCommand::Uninstall(uninstall_command) => handle_uninstall_command(uninstall_command)?,
        SubCommand::Use(use_command) => handle_use_command(use_command)?,
    }

    Ok(())
}

fn handle_resolve_command(resolve_command: ResolveCommand) -> Result<(), ErrBox> {
    let plugin_manifest = plugins::read_manifest()?;
    let command_name = CommandName::from_string(resolve_command.binary_name);
    let executable_path = if let Some(info) = get_executable_path_from_config_file(&plugin_manifest, &command_name)? {
        if let Some(executable_path) = info.executable_path {
            Some(executable_path.clone())
        } else {
            if info.had_uninstalled_binary {
                eprintln!(
                    "[bvm warning]: There were uninstalled binaries (run `bvm install`). Resolving global '{}'.",
                    command_name.display()
                );
            }
            None
        }
    } else {
        None
    };
    let executable_path = match executable_path {
        Some(path) => path,
        None => get_global_binary_file_name(&plugin_manifest, &command_name)?,
    };

    println!("{}", executable_path);

    Ok(())
}

async fn handle_install_command() -> Result<(), ErrBox> {
    let config_file_path = match configuration::find_config_file()? {
        Some(file_path) => file_path,
        None => return err!("Could not find .bvmrc.json in the current directory or its ancestors."),
    };
    let config_file_text = std::fs::read_to_string(&config_file_path)?;
    let config_file = configuration::read_config_file(&config_file_text)?;
    let shim_dir = utils::get_shim_dir()?;
    let mut plugin_manifest = plugins::read_manifest()?;

    for entry in config_file.binaries.iter() {
        let is_installed = plugin_manifest
            .get_identifier_from_url(&entry.url)
            .map(|identifier| plugin_manifest.get_binary(&identifier).is_some())
            .unwrap_or(false);

        if !is_installed {
            // setup the plugin
            let binary_item = plugins::setup_plugin(&mut plugin_manifest, &entry, &shim_dir).await?;
            let command_name = binary_item.get_command_name();
            let identifier = binary_item.get_identifier();
            // check if there is a global binary location set and if not, set it
            if plugin_manifest.get_global_binary_location(&command_name).is_none() {
                if utils::get_path_executable_path(&command_name)?.is_some() {
                    plugin_manifest.use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Path);
                } else {
                    plugin_manifest
                        .use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Bvm(identifier));
                }
            }
            plugins::write_manifest(&plugin_manifest)?; // write for every setup plugin in case a further one fails
        }
    }

    Ok(())
}

async fn handle_install_url_command(url: String) -> Result<(), ErrBox> {
    let checksum_url = utils::parse_checksum_url(&url);
    let shim_dir = utils::get_shim_dir()?;
    let mut plugin_manifest = plugins::read_manifest()?;

    // todo: require `--force` if already installed

    // remove the existing binary from the cache (the setup_plugin function will delete it from the disk)
    let was_global_version = if let Some(identifier) = plugin_manifest
        .get_identifier_from_url(&url)
        .map(|identifier| identifier.clone())
    {
        let is_global_version = plugin_manifest.is_global_version(&identifier);
        plugin_manifest.remove_binary(&identifier);
        plugin_manifest.remove_url(&url);
        plugins::write_manifest(&plugin_manifest)?;
        is_global_version
    } else {
        false
    };

    let binary_item = plugins::setup_plugin(&mut plugin_manifest, &checksum_url, &shim_dir).await?;
    let identifier = binary_item.get_identifier();
    let binary_name = binary_item.get_binary_name();
    let version = binary_item.version.clone();
    // set this back as being the global version if setup is successful
    if was_global_version {
        let command_name = binary_item.get_command_name();
        plugin_manifest.use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()));
    }

    let is_global_version = plugin_manifest.is_global_version(&identifier);
    if !is_global_version {
        let command_name = binary_name.get_command_name();
        eprintln!(
            "Installed. Run `bvm use {} {}` to set it as the global '{}' binary.",
            binary_name.display_toggled_owner(!plugin_manifest.command_has_same_owner(&command_name)),
            version,
            command_name.display(),
        );
    }

    plugins::write_manifest(&plugin_manifest)
}

fn handle_uninstall_command(uninstall_command: UninstallCommand) -> Result<(), ErrBox> {
    let shim_dir = utils::get_shim_dir()?;
    let mut plugin_manifest = plugins::read_manifest()?;
    let binary = get_binary_with_name_and_version(
        &plugin_manifest,
        &uninstall_command.binary_name,
        &uninstall_command.version,
    )?;
    let binary_name = binary.get_binary_name();
    let command_name = binary_name.get_command_name();
    let plugin_dir = plugins::get_plugin_dir(&binary.owner, &binary.name, &binary.version)?;
    let binary_identifier = binary.get_identifier();

    // remove the plugin from the manifest first
    plugin_manifest.remove_binary(&binary_identifier);
    plugins::write_manifest(&plugin_manifest)?;

    // check if this is the last binary with this name. If so, delete the shim
    if !plugin_manifest.has_binary_with_command(&command_name) {
        std::fs::remove_file(plugins::get_path_script_path(&shim_dir, &command_name))?;
    }

    // now attempt to delete the directory
    std::fs::remove_dir_all(&plugin_dir)?;

    // delete the parent directories if empty
    let binary_name_dir = plugin_dir.parent().unwrap();
    if utils::is_dir_empty(&binary_name_dir)? {
        std::fs::remove_dir_all(&binary_name_dir)?;
        // now delete the owner name if empty
        let owner_name_dir = binary_name_dir.parent().unwrap();
        if utils::is_dir_empty(&owner_name_dir)? {
            std::fs::remove_dir_all(&owner_name_dir)?;
        }
    }

    Ok(())
}

fn handle_use_command(use_command: UseCommand) -> Result<(), ErrBox> {
    let mut plugin_manifest = plugins::read_manifest()?;
    let command_name = use_command.binary_name.get_command_name();
    let is_binary_in_config_file = get_executable_path_from_config_file(&plugin_manifest, &command_name)?
        .map(|info| info.executable_path)
        .flatten()
        .is_some();
    if use_command.version.to_lowercase() == "path" {
        if !plugin_manifest.has_binary_with_name(&use_command.binary_name) {
            return err!(
                "Could not find any installed binaries named '{}'",
                use_command.binary_name.display()
            );
        }
        if utils::get_path_executable_path(&command_name)?.is_none() {
            return err!(
                "Could not find any installed binaries on the path that matched '{}'",
                command_name.display()
            );
        }
        plugin_manifest.use_global_version(command_name, plugins::GlobalBinaryLocation::Path);
    } else {
        let binary =
            get_binary_with_name_and_version(&plugin_manifest, &use_command.binary_name, &use_command.version)?;
        let identifier = binary.get_identifier(); // separate line to prevent mutating while borrowing
        plugin_manifest.use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier));
    }
    plugins::write_manifest(&plugin_manifest)?;

    if is_binary_in_config_file {
        eprintln!("Updated globally used version, but local version remains using version specified in the current working directory's config file. If you wish to change the local version, then update your configuration file (check the cwd and ancestor directories for a .bvmrc.json file).");
    }

    return Ok(());
}

struct ConfigFileExecutableInfo {
    executable_path: Option<String>,
    had_uninstalled_binary: bool,
}

fn get_executable_path_from_config_file(
    plugin_manifest: &plugins::PluginsManifest,
    command_name: &CommandName,
) -> Result<Option<ConfigFileExecutableInfo>, ErrBox> {
    let config_file_path = configuration::find_config_file()?;
    Ok(if let Some(config_file_path) = config_file_path {
        // todo: cleanup :)
        let config_file_text = std::fs::read_to_string(&config_file_path)?;
        let config_file = configuration::read_config_file(&config_file_text)?;
        let mut had_uninstalled_binary = false;
        let mut config_file_binary = None;

        for url in config_file.binaries.iter() {
            if let Some(identifier) = plugin_manifest.get_identifier_from_url(&url.url) {
                if let Some(cache_item) = plugin_manifest.get_binary(&identifier) {
                    if cache_item.name == command_name.as_str() {
                        config_file_binary = Some(cache_item);
                        break;
                    }
                } else {
                    had_uninstalled_binary = true;
                }
            } else {
                had_uninstalled_binary = true;
            }
        }

        Some(ConfigFileExecutableInfo {
            executable_path: config_file_binary.map(|b| b.file_name.clone()),
            had_uninstalled_binary,
        })
    } else {
        None
    })
}

fn get_binary_with_name_and_version<'a>(
    plugin_manifest: &'a plugins::PluginsManifest,
    binary_name: &BinaryName,
    version: &str,
) -> Result<&'a plugins::BinaryManifestItem, ErrBox> {
    let binaries = plugin_manifest.get_binaries_by_name_and_version(&binary_name, &version);
    if binaries.len() == 0 {
        let binaries = plugin_manifest.get_binaries_with_name(binary_name);
        if binaries.is_empty() {
            err!(
                "Could not find any installed binaries named '{}'",
                binary_name.display()
            )
        } else {
            err!(
                "Could not find binary '{}' with version '{}'\n\nInstalled versions:\n  {}",
                binary_name.display(),
                version,
                display_binaries_versions(binaries),
            )
        }
    } else if binaries.len() > 1 {
        return err!(
            "There were multiple binaries with the specified name '{}' with version '{}'. Please include the owner to uninstall.\n\nInstalled versions:\n  {}",
            binary_name.display(),
            version,
            display_binaries_versions(binaries),
        );
    } else {
        Ok(binaries[0])
    }
}

fn display_binaries_versions(binaries: Vec<&plugins::BinaryManifestItem>) -> String {
    if binaries.is_empty() {
        return String::new();
    }

    let mut binaries = binaries;
    binaries.sort_by(|a, b| a.compare(b));
    let have_same_owner = get_have_same_owner(&binaries);
    let lines = binaries
        .into_iter()
        .map(|b| {
            if have_same_owner {
                b.version.clone()
            } else {
                format!("{}/{} {}", b.owner, b.name, b.version)
            }
        })
        .collect::<Vec<_>>();

    return lines.join("\n  ");

    fn get_have_same_owner(binaries: &Vec<&plugins::BinaryManifestItem>) -> bool {
        let first_owner = &binaries[0].owner;
        binaries.iter().all(|b| &b.owner == first_owner)
    }
}

fn get_global_binary_file_name(
    plugin_manifest: &plugins::PluginsManifest,
    command_name: &CommandName,
) -> Result<String, ErrBox> {
    match plugin_manifest.get_global_binary_location(command_name) {
        Some(location) => match location {
            plugins::GlobalBinaryLocation::Path => {
                if let Some(path_executable_path) = utils::get_path_executable_path(command_name)? {
                    Ok(path_executable_path.to_string_lossy().to_string())
                } else {
                    err!("Binary '{}' is configured to use the executable on the path, but only the bvm version exists on the path. Run `bvm use {0} <some other version>` to select a version to run.", command_name.display())
                }
            }
            plugins::GlobalBinaryLocation::Bvm(identifier) => {
                if let Some(item) = plugin_manifest.get_binary(&identifier) {
                    Ok(item.file_name.clone())
                } else {
                    err!("Should have found executable path for global binary. Report this as a bug and update the version used by running `bvm use {} <some other version>`", command_name.display())
                }
            }
        },
        None => {
            // use the executable on the path
            if let Some(path_executable_path) = utils::get_path_executable_path(command_name)? {
                Ok(path_executable_path.to_string_lossy().to_string())
            } else {
                err!("Could not find binary '{}'", command_name.display())
            }
        }
    }
}
