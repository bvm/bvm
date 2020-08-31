#[macro_use]
mod types;
#[macro_use]
mod environment;

mod arg_parser;
mod configuration;
mod plugins;
mod utils;

use std::path::PathBuf;

use arg_parser::*;
use environment::Environment;
use types::{BinaryName, CommandName, ErrBox};

#[tokio::main]
async fn main() -> Result<(), ErrBox> {
    let environment = environment::RealEnvironment::new(false);
    let args = std::env::args().collect();
    match run(&environment, args).await {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err.to_string());
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn run<TEnvironment: Environment>(environment: &TEnvironment, args: Vec<String>) -> Result<(), ErrBox> {
    let args = parse_args(args)?;

    match args.sub_command {
        SubCommand::Help(text) => environment.log(&text),
        SubCommand::Version => environment.log(&format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))),
        SubCommand::Resolve(command) => handle_resolve_command(environment, command)?,
        SubCommand::Install(command) => handle_install_command(environment, command).await?,
        SubCommand::InstallUrl(command) => handle_install_url_command(environment, command).await?,
        SubCommand::Uninstall(command) => handle_uninstall_command(environment, command)?,
        SubCommand::Use => handle_use_command(environment)?,
        SubCommand::UseBinary(command) => handle_use_binary_command(environment, command)?,
        SubCommand::List => handle_list_command(environment)?,
        SubCommand::Init => handle_init_command(environment)?,
    }

    Ok(())
}

fn handle_resolve_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    resolve_command: ResolveCommand,
) -> Result<(), ErrBox> {
    let plugin_manifest = plugins::read_manifest(environment)?;
    let command_name = CommandName::from_string(resolve_command.binary_name);
    let info = get_executable_path_from_config_file(environment, &plugin_manifest, &command_name)?;
    let executable_path = if let Some(info) = info {
        if let Some(executable_path) = info.executable_path {
            Some(executable_path.clone())
        } else {
            if info.had_uninstalled_binary {
                environment.log_error(&format!(
                    "[bvm warning]: There were some not installed binaries (run `bvm install`). Resolving global '{}'.",
                    command_name.display()
                ));
            }
            None
        }
    } else {
        None
    };
    let executable_path = match executable_path {
        Some(path) => path,
        None => get_global_binary_file_name(environment, &plugin_manifest, &command_name)?,
    };

    environment.log(&executable_path.to_string_lossy());

    Ok(())
}

async fn handle_install_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: InstallCommand,
) -> Result<(), ErrBox> {
    let config_file = get_config_file_or_error(environment)?;
    let shim_dir = utils::get_shim_dir(environment)?;
    let mut plugin_manifest = plugins::read_manifest(environment)?;

    for entry in config_file.binaries.iter() {
        let is_installed = plugin_manifest
            .get_identifier_from_url(&entry.url)
            .map(|identifier| plugin_manifest.get_binary(&identifier).is_some())
            .unwrap_or(false);

        if command.force || !is_installed {
            // setup the plugin
            let binary_item = plugins::setup_plugin(environment, &mut plugin_manifest, &entry, &shim_dir).await?;
            let identifier = binary_item.get_identifier();
            // check if there is a global binary location set and if not, set it
            for command_name in binary_item.get_command_names() {
                set_global_binary_if_not_set(environment, &mut plugin_manifest, &identifier, &command_name)?;
            }
            plugins::write_manifest(environment, &plugin_manifest)?; // write for every setup plugin in case a further one fails
        }
    }

    if command.use_command {
        for entry in config_file.binaries.iter() {
            let identifier = plugin_manifest.get_identifier_from_url(&entry.url).unwrap().clone();
            let binary = plugin_manifest.get_binary(&identifier).unwrap();
            for command_name in binary.get_command_names() {
                plugin_manifest
                    .use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()));
            }
        }
        plugins::write_manifest(environment, &plugin_manifest)?;
    }

    if let Some(post_install) = &config_file.post_install {
        environment.run_shell_command(&environment.cwd()?, post_install)?;
    }

    Ok(())
}

async fn handle_install_url_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: InstallUrlCommand,
) -> Result<(), ErrBox> {
    let checksum_url = utils::parse_checksum_url(&command.url);
    let shim_dir = utils::get_shim_dir(environment)?;
    let mut plugin_manifest = plugins::read_manifest(environment)?;
    let identifier = plugin_manifest
        .get_identifier_from_url(&command.url)
        .map(|identifier| identifier.clone());
    let is_installed = if let Some(identifier) = &identifier {
        plugin_manifest.get_binary(&identifier).is_some()
    } else {
        false
    };

    if is_installed && !command.force {
        environment.log_error("Already installed. Provide the `--force` flag to reinstall.");
    } else {
        // remove the existing binary from the cache (the setup_plugin function will delete it from the disk)
        let previous_global_command_names = if let Some(identifier) = identifier {
            let previous_global_command_names = plugin_manifest.get_global_command_names(&identifier);
            plugin_manifest.remove_binary(&identifier);
            plugin_manifest.remove_url(&command.url);
            plugins::write_manifest(environment, &plugin_manifest)?;
            // check if this is the last binary with this name. If so, delete the shim
            for command_name in previous_global_command_names.iter() {
                if !plugin_manifest.has_binary_with_command(&command_name) {
                    environment.remove_file(&plugins::get_shim_path(&shim_dir, &command_name))?;
                }
            }
            previous_global_command_names
        } else {
            Vec::new()
        };

        let binary_item = plugins::setup_plugin(environment, &mut plugin_manifest, &checksum_url, &shim_dir).await?;
        let identifier = binary_item.get_identifier();
        let binary_name = binary_item.get_binary_name();
        let version = binary_item.version.clone();
        let command_names = binary_item.get_command_names();

        // set this back as being the global version if setup is successful
        for command_name in previous_global_command_names {
            if command_names.contains(&command_name) {
                plugin_manifest
                    .use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()));
            }
        }

        if !command.use_command {
            let mut not_set_command_name = false;
            for command_name in command_names.iter() {
                if !set_global_binary_if_not_set(environment, &mut plugin_manifest, &identifier, &command_name)? {
                    not_set_command_name = true;
                }
            }
            if not_set_command_name {
                environment.log_error(&format!(
                    "Installed. Run `bvm use {} {}` to use it on the path as {}.",
                    binary_name.display_toggled_owner(!plugin_manifest.binary_name_has_same_owner(&binary_name)),
                    version,
                    command_names
                        .into_iter()
                        .map(|c| format!("'{}'", c.display()))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
        }
    }

    if command.use_command {
        let identifier = plugin_manifest
            .get_identifier_from_url(&command.url)
            .map(|identifier| identifier.clone())
            .unwrap();
        let command_names = plugin_manifest.get_binary(&identifier).unwrap().get_command_names();
        for command_name in command_names {
            plugin_manifest.use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()));
        }
    }

    plugins::write_manifest(environment, &plugin_manifest)?;

    Ok(())
}

fn set_global_binary_if_not_set(
    environment: &impl Environment,
    plugin_manifest: &mut plugins::PluginsManifest,
    identifier: &plugins::BinaryIdentifier,
    command_name: &CommandName,
) -> Result<bool, ErrBox> {
    Ok(if plugin_manifest.get_global_binary_location(&command_name).is_none() {
        if utils::get_path_executable_path(environment, &command_name)?.is_some() {
            plugin_manifest.use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Path);
            false
        } else {
            plugin_manifest.use_global_version(
                command_name.clone(),
                plugins::GlobalBinaryLocation::Bvm(identifier.clone()),
            );
            true
        }
    } else {
        plugin_manifest.is_global_version(identifier, command_name)
    })
}

fn handle_uninstall_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    uninstall_command: UninstallCommand,
) -> Result<(), ErrBox> {
    let shim_dir = utils::get_shim_dir(environment)?;
    let mut plugin_manifest = plugins::read_manifest(environment)?;
    let binary = get_binary_with_name_and_version(
        &plugin_manifest,
        &uninstall_command.binary_name,
        &uninstall_command.version,
    )?;
    let command_names = binary.get_command_names();
    let plugin_dir = plugins::get_plugin_dir(environment, &binary.owner, &binary.name, &binary.version)?;
    let binary_identifier = binary.get_identifier();

    // remove the plugin from the manifest first
    plugin_manifest.remove_binary(&binary_identifier);
    plugins::write_manifest(environment, &plugin_manifest)?;

    // check if this is the last binary using these command names. If so, delete the shim
    for command_name in command_names.iter() {
        if !plugin_manifest.has_binary_with_command(&command_name) {
            environment.remove_file(&plugins::get_shim_path(&shim_dir, &command_name))?;
        }
    }

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
    let mut plugin_manifest = plugins::read_manifest(environment)?;
    let config_file = get_config_file_or_error(environment)?;

    for entry in config_file.binaries.iter() {
        let mut was_installed = false;
        let identifier = plugin_manifest
            .get_identifier_from_url(&entry.url)
            .map(|i| i.to_owned());
        if let Some(identifier) = identifier {
            let binary = plugin_manifest.get_binary(&identifier);
            if let Some(binary) = binary {
                for command_name in binary.get_command_names() {
                    plugin_manifest
                        .use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()));
                }
                was_installed = true;
            }
        }

        if !was_installed {
            return err!("Ensure binaries are installed before using. Run `bvm install` first then `bvm use`.");
        }
    }

    plugins::write_manifest(environment, &plugin_manifest)?;
    Ok(())
}

fn handle_use_binary_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    use_command: UseBinaryCommand,
) -> Result<(), ErrBox> {
    let mut plugin_manifest = plugins::read_manifest(environment)?;
    let command_names = if use_command.version.to_lowercase() == "path" {
        let global_location =
            plugin_manifest.get_global_binary_location(&CommandName::from_string(use_command.binary_name.name.clone()));
        let identifier = match global_location {
            Some(plugins::GlobalBinaryLocation::Bvm(identifier)) => identifier,
            None | Some(plugins::GlobalBinaryLocation::Path) => return Ok(()), // already done
        };
        plugin_manifest.get_global_command_names(&identifier)
    } else {
        let binary =
            get_binary_with_name_and_version(&plugin_manifest, &use_command.binary_name, &use_command.version)?;
        binary.get_command_names()
    };
    for command_name in command_names {
        let is_binary_in_config_file =
            get_executable_path_from_config_file(environment, &plugin_manifest, &command_name)?
                .map(|info| info.executable_path)
                .flatten()
                .is_some();
        if use_command.version.to_lowercase() == "path" {
            if !plugin_manifest.has_binary_with_name(&use_command.binary_name) {
                return err!(
                    "Could not find any installed binaries named '{}'.",
                    use_command.binary_name.display()
                );
            }
            if utils::get_path_executable_path(environment, &command_name)?.is_none() {
                return err!(
                    "Could not find any installed binaries on the path that matched '{}'.",
                    command_name.display()
                );
            }
            plugin_manifest.use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Path);
        } else {
            let binary =
                get_binary_with_name_and_version(&plugin_manifest, &use_command.binary_name, &use_command.version)?;
            let identifier = binary.get_identifier();
            plugin_manifest.use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Bvm(identifier));
        }

        if is_binary_in_config_file {
            environment.log_error(&format!("Updated globally used version of '{}', but local version remains using version specified in the current working directory's config file. If you wish to change the local version, then update your configuration file (check the cwd and ancestor directories for a .bvmrc.json file).", command_name.display()));
        }
    }
    plugins::write_manifest(environment, &plugin_manifest)?;

    Ok(())
}

fn handle_list_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = plugins::read_manifest(environment)?;
    let mut binaries = plugin_manifest.binaries().collect::<Vec<_>>();
    if !binaries.is_empty() {
        binaries.sort_by(|a, b| a.compare(b));
        let lines = binaries
            .into_iter()
            .map(|b| format!("{}/{} {}", b.owner, b.name, b.version))
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
    } else {
        environment.write_file_text(&config_path, "{\n  \"binaries\": [\n  ]\n}\n")?;
        environment.log(&format!("Created {}", configuration::CONFIG_FILE_NAME));
        Ok(())
    }
}

struct ConfigFileExecutableInfo {
    executable_path: Option<PathBuf>,
    had_uninstalled_binary: bool,
}

fn get_executable_path_from_config_file<TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &plugins::PluginsManifest,
    command_name: &CommandName,
) -> Result<Option<ConfigFileExecutableInfo>, ErrBox> {
    Ok(if let Some(config_file) = get_config_file(environment)? {
        let mut had_uninstalled_binary = false;
        let mut executable_path = None;

        for url in config_file.binaries.iter() {
            if let Some(identifier) = plugin_manifest.get_identifier_from_url(&url.url) {
                if let Some(cache_item) = plugin_manifest.get_binary(&identifier) {
                    for command in cache_item.commands.iter() {
                        if command.name == command_name.as_str() {
                            let plugin_cache_dir = plugins::get_plugin_dir(
                                environment,
                                &cache_item.owner,
                                &cache_item.name,
                                &cache_item.version,
                            )?;
                            executable_path = Some(plugin_cache_dir.join(&command.path));
                            break;
                        }
                    }
                } else {
                    had_uninstalled_binary = true;
                }
            } else {
                had_uninstalled_binary = true;
            }
        }

        Some(ConfigFileExecutableInfo {
            executable_path,
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
                display_binaries_versions(binaries).join("\n "),
            )
        }
    } else if binaries.len() > 1 {
        return err!(
            "There were multiple binaries with the specified name '{}' with version '{}'. Please include the owner to uninstall.\n\nInstalled versions:\n  {}",
            binary_name.display(),
            version,
            display_binaries_versions(binaries).join("\n  "),
        );
    } else {
        Ok(binaries[0])
    }
}

fn display_binaries_versions(binaries: Vec<&plugins::BinaryManifestItem>) -> Vec<String> {
    if binaries.is_empty() {
        return Vec::new();
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

    return lines;

    fn get_have_same_owner(binaries: &Vec<&plugins::BinaryManifestItem>) -> bool {
        let first_owner = &binaries[0].owner;
        binaries.iter().all(|b| &b.owner == first_owner)
    }
}

fn get_global_binary_file_name(
    environment: &impl Environment,
    plugin_manifest: &plugins::PluginsManifest,
    command_name: &CommandName,
) -> Result<PathBuf, ErrBox> {
    match plugin_manifest.get_global_binary_location(command_name) {
        Some(location) => match location {
            plugins::GlobalBinaryLocation::Path => {
                if let Some(path_executable_path) = utils::get_path_executable_path(environment, command_name)? {
                    Ok(path_executable_path)
                } else {
                    err!("Binary '{}' is configured to use the executable on the path, but only the bvm version exists on the path. Run `bvm use {0} <some other version>` to select a version to run.", command_name.display())
                }
            }
            plugins::GlobalBinaryLocation::Bvm(identifier) => {
                if let Some(item) = plugin_manifest.get_binary(&identifier) {
                    let plugin_cache_dir =
                        plugins::get_plugin_dir(environment, &item.owner, &item.name, &item.version)?;
                    let command = item
                        .commands
                        .iter()
                        .filter(|c| c.name == command_name.as_str())
                        .next()
                        .expect("Expected to have command.");
                    Ok(plugin_cache_dir.join(&command.path))
                } else {
                    err!("Should have found executable path for global binary. Report this as a bug and update the version used by running `bvm use {} <some other version>`", command_name.display())
                }
            }
        },
        None => {
            // use the executable on the path
            if let Some(path_executable_path) = utils::get_path_executable_path(environment, command_name)? {
                Ok(path_executable_path)
            } else {
                let binaries = plugin_manifest.get_binaries_with_command(command_name);
                if binaries.is_empty() {
                    err!(
                        "Could not find binary on the path for command '{}'",
                        command_name.display()
                    )
                } else {
                    err!(
                        "No binary is set on the path for command '{}'. Run `bvm use {0} <version>` to set a global version.\n\nInstalled versions:\n  {}",
                        command_name.display(),
                        display_binaries_versions(binaries).join("\n "),
                    )
                }
            }
        }
    }
}

fn get_config_file_or_error(environment: &impl Environment) -> Result<configuration::ConfigFile, ErrBox> {
    match get_config_file(environment)? {
        Some(config_file) => Ok(config_file),
        None => return err!("Could not find .bvmrc.json in the current directory or its ancestors."),
    }
}

fn get_config_file(environment: &impl Environment) -> Result<Option<configuration::ConfigFile>, ErrBox> {
    if let Some(config_file_path) = configuration::find_config_file(environment)? {
        let config_file_text = environment.read_file_text(&config_file_path)?;
        Ok(Some(configuration::read_config_file(&config_file_text)?))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use bytes::Bytes;
    use pretty_assertions::assert_eq;
    use std::io::Write;
    use std::path::PathBuf;

    use super::run;
    use crate::environment::{Environment, TestEnvironment};
    use crate::types::ErrBox;

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
            run_cli(vec!["resolve", $name], &$environment).await.unwrap();
            let logged_messages = $environment.take_logged_messages();
            assert_eq!(logged_messages, vec![$binary_path.clone()]);
        };
    }

    macro_rules! assert_resolves {
        ($environment:expr, $binary_path:expr) => {
            assert_resolves_name!($environment, "name", $binary_path)
        };
    }

    macro_rules! install_url {
        ($environment:expr, $url:expr) => {
            run_cli(vec!["install", $url], &$environment).await.unwrap();
        };
    }

    #[tokio::test]
    async fn should_output_version() {
        let environment = TestEnvironment::new();
        run_cli(vec!["--version"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(logged_messages, vec![format!("bvm {}", env!("CARGO_PKG_VERSION"))]);
    }

    #[tokio::test]
    async fn should_init() {
        let environment = TestEnvironment::new();
        run_cli(vec!["init"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(logged_messages, vec!["Created .bvmrc.json"]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from(".bvmrc.json")).unwrap(),
            "{\n  \"binaries\": [\n  ]\n}\n"
        );
    }

    #[tokio::test]
    async fn should_error_if_init_has_file() {
        let environment = TestEnvironment::new();
        environment.write_file_text(&PathBuf::from(".bvmrc.json"), "").unwrap();
        let error_text = run_cli(vec!["init"], &environment).await.err().unwrap();
        assert_eq!(
            error_text.to_string(),
            "A .bvmrc.json file already exists in the current directory."
        );
    }

    #[tokio::test]
    async fn install_url_command_no_path() {
        let environment = TestEnvironment::new();
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");

        // install the package
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 1.0.0..."]);

        // check setup was correct
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_has_path!(environment, &binary_path);
        assert_has_path!(environment, &get_shim_path("name"));

        // try to resolve the command globally
        assert_resolves!(environment, binary_path);

        // try to use the path version, it should fail
        let error_message = run_cli(vec!["use", "name", "path"], &environment).await.err().unwrap();
        assert_eq!(
            error_message.to_string(),
            "Could not find any installed binaries on the path that matched 'name'."
        );
    }

    #[tokio::test]
    async fn install_url_command_path() {
        let environment = TestEnvironment::new();
        let path_exe_path = add_binary_to_path(&environment, "name");
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");

        // install the package
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            vec![
                "Installing owner/name 1.0.0...",
                "Installed. Run `bvm use name 1.0.0` to use it on the path as 'name'."
            ]
        );

        // try to resolve globally, it should use command on path
        assert_resolves!(environment, path_exe_path);

        // now use the installed version
        run_cli(vec!["use", "name", "1.0.0"], &environment).await.unwrap();
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // switch back to the path
        run_cli(vec!["use", "name", "path"], &environment).await.unwrap();
        assert_resolves!(&environment, path_exe_path);
    }

    #[tokio::test]
    async fn install_url_command_previous_install() {
        let environment = TestEnvironment::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        let third_binary_path = get_binary_path("owner", "name", "3.0.0");
        let fourth_binary_path = get_binary_path("owner", "name", "4.0.0");
        let fourth_binary_path_second = get_binary_path_second("owner", "name", "4.0.0");

        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        create_remote_zip_package(&environment, "http://localhost/package2.json", "owner", "name", "2.0.0");
        create_remote_zip_package(&environment, "http://localhost/package3.json", "owner", "name", "3.0.0");
        create_remote_zip_multiple_commands_package(
            &environment,
            "http://localhost/package4.json",
            "owner",
            "name",
            "4.0.0",
        );

        // install the first package
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 1.0.0...",]);

        // now install the second
        install_url!(environment, "http://localhost/package2.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            vec![
                "Installing owner/name 2.0.0...",
                "Installed. Run `bvm use name 2.0.0` to use it on the path as 'name'."
            ]
        );
        assert_resolves!(&environment, first_binary_path);

        // use the second package
        run_cli(vec!["use", "name", "2.0.0"], &environment).await.unwrap();
        assert_resolves!(&environment, second_binary_path);

        // install the third package with --use
        run_cli(vec!["install", "--use", "http://localhost/package3.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 3.0.0...",]);
        assert_resolves!(&environment, third_binary_path);

        // install the fourth package
        install_url!(environment, "http://localhost/package4.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            vec![
                "Installing owner/name 4.0.0...",
                "Installed. Run `bvm use name 4.0.0` to use it on the path as 'name', 'name-second'."
            ]
        );
        assert_resolves!(&environment, third_binary_path);

        // now install the fourth package again, but with --use
        run_cli(vec!["install", "--use", "http://localhost/package4.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            vec!["Already installed. Provide the `--force` flag to reinstall."]
        );
        assert_resolves!(&environment, fourth_binary_path);
        assert_resolves_name!(&environment, "name-second", fourth_binary_path_second);

        // now install with --force
        run_cli(
            vec!["install", "--force", "http://localhost/package4.json"],
            &environment,
        )
        .await
        .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 4.0.0...",]);
        assert_resolves!(&environment, fourth_binary_path);
        assert_resolves_name!(&environment, "name-second", fourth_binary_path_second);
    }

    #[tokio::test]
    async fn install_url_command_tar_gz() {
        let environment = TestEnvironment::new();
        let binary_path = get_binary_path("owner", "name", "1.0.0");

        create_remote_tar_gz_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");

        // install and check setup
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 1.0.0...",]);
        assert_has_path!(environment, &binary_path);
        assert_has_path!(environment, &get_shim_path("name"));

        // yeah, this isn't realistic, but it's just some dummy data to ensure the file was extracted correctly
        assert_eq!(
            environment.read_file_text(&PathBuf::from(binary_path)).unwrap(),
            "test-https://github.com/dsherret/bvm/releases/download/1.0.0/name-windows.tar.gz"
        );
    }

    #[tokio::test]
    async fn install_command_no_existing_binary() {
        let environment = TestEnvironment::new();
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        create_remote_zip_package(&environment, "http://localhost/package2.json", "owner", "name", "2.0.0");
        create_bvmrc(&environment, vec!["http://localhost/package.json"]);

        // attempt to install in directory that doesn't have the config file
        let error_text = run_cli(vec!["install"], &environment).await.err().unwrap().to_string();
        assert_eq!(
            error_text,
            "Could not find .bvmrc.json in the current directory or its ancestors."
        );

        // move to the correct dir, then try again
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 1.0.0..."]);

        // now try to resolve the binary
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // go up a directory and it should resolve
        environment.set_cwd("/");
        assert_resolves!(environment, binary_path);
    }

    #[tokio::test]
    async fn install_command_previous_install_binary() {
        let environment = TestEnvironment::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        create_remote_zip_package(&environment, "http://localhost/package2.json", "owner", "name", "2.0.0");
        create_bvmrc(&environment, vec!["http://localhost/package2.json"]);

        // install a package globally
        run_cli(vec!["install", "http://localhost/package.json"], &environment)
            .await
            .unwrap();
        environment.clear_logs();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 2.0.0..."]);

        // now try to resolve the binary
        assert_resolves!(environment, second_binary_path);

        // try reinstalling, it should not output anything
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors.len(), 0);

        // try reinstalling, but provide --force
        run_cli(vec!["install", "--force"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 2.0.0..."]);

        // go up a directory and it should resolve to the previously set global
        environment.set_cwd("/");
        assert_resolves!(environment, first_binary_path);

        // go back and provide --use
        environment.set_cwd("/project");
        run_cli(vec!["install", "--use"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors.len(), 0);

        // go up a directory and it should use the path from the config globally now
        environment.set_cwd("/");
        assert_resolves!(environment, second_binary_path);
    }

    #[tokio::test]
    async fn install_command_binary_on_path() {
        let environment = TestEnvironment::new();
        let path_exe_path = add_binary_to_path(&environment, "name");
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        create_bvmrc(&environment, vec!["http://localhost/package.json"]);

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 1.0.0..."]);

        // now try to resolve the binary
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // go up a directory and it should resolve to binary on the path still
        environment.set_cwd("/");
        assert_resolves!(environment, path_exe_path);
    }

    #[tokio::test]
    async fn install_command_post_install() {
        let environment = TestEnvironment::new();
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        environment
            .write_file_text(
                &PathBuf::from("/project/.bvmrc.json"),
                r#"{"postInstall": "echo \"Hello world!\"", "binaries": ["http://localhost/package.json"]}"#,
            )
            .unwrap();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, vec!["Installing owner/name 1.0.0..."]);
        let logged_shell_commands = environment.take_run_shell_commands();
        assert_eq!(
            logged_shell_commands,
            vec![("/project".to_string(), "echo \"Hello world!\"".to_string())]
        );
    }

    #[tokio::test]
    async fn install_unknown_config_key() {
        let environment = TestEnvironment::new();
        environment
            .write_file_text(
                &PathBuf::from("/.bvmrc.json"),
                r#"{"test": "", "binaries": ["http://localhost/package.json"]}"#,
            )
            .unwrap();

        let error_message = run_cli(vec!["install"], &environment).await.err().unwrap();
        assert_eq!(error_message.to_string(), "Unknown key in configuration file: test");
    }

    #[tokio::test]
    async fn uninstall_command_binary_on_path() {
        let environment = TestEnvironment::new();
        let path_exe_path = add_binary_to_path(&environment, "name");
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");

        // install and use the package
        run_cli(vec!["install", "--use", "http://localhost/package.json"], &environment)
            .await
            .unwrap();
        environment.clear_logs();
        assert_has_path!(environment, &get_shim_path("name"));
        run_cli(vec!["uninstall", "name", "1.0.0"], &environment).await.unwrap();

        // ensure it resolves the previous binary on the path
        assert_resolves!(environment, path_exe_path);
        assert_not_has_path!(environment, &get_shim_path("name"));
        assert_not_has_path!(environment, &get_binary_path("owner", "name", "1.0.0"));
    }

    #[tokio::test]
    async fn uninstall_command_multiple_binaries() {
        let environment = TestEnvironment::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        create_remote_zip_package(&environment, "http://localhost/package2.json", "owner", "name", "2.0.0");
        create_remote_zip_multiple_commands_package(
            &environment,
            "http://localhost/package3.json",
            "owner",
            "name",
            "3.0.0",
        );

        // install and the first package
        install_url!(environment, "http://localhost/package.json");
        environment.clear_logs();

        // install and use the second package
        run_cli(vec!["install", "--use", "http://localhost/package2.json"], &environment)
            .await
            .unwrap();
        environment.clear_logs();
        assert_has_path!(environment, &get_shim_path("name"));

        // now install the second package
        run_cli(vec!["uninstall", "name", "2.0.0"], &environment).await.unwrap();

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
        run_cli(vec!["install", "--use", "http://localhost/package3.json"], &environment)
            .await
            .unwrap();
        environment.clear_logs();
        assert_has_path!(environment, &get_shim_path("name"));
        assert_has_path!(environment, &get_shim_path("name-second"));
        run_cli(vec!["uninstall", "name", "3.0.0"], &environment).await.unwrap();
        assert_has_path!(environment, &get_shim_path("name"));
        assert_not_has_path!(environment, &get_shim_path("name-second"));

        // uninstall the first package and it should no longer have the shim
        run_cli(vec!["uninstall", "name", "1.0.0"], &environment).await.unwrap();
        assert_not_has_path!(environment, &get_shim_path("name"));
        assert_not_has_path!(environment, &first_binary_path);
        assert_eq!(environment.is_dir_deleted(&name_dir), true);
    }

    #[tokio::test]
    async fn list_command_with_no_installs() {
        let environment = TestEnvironment::new();
        run_cli(vec!["list"], &environment).await.unwrap();
        assert_eq!(environment.take_logged_messages().len(), 0);
    }

    #[tokio::test]
    async fn list_command_with_installs() {
        let environment = TestEnvironment::new();
        create_remote_zip_package(&environment, "http://localhost/package.json", "owner", "name", "1.0.0");
        create_remote_zip_package(&environment, "http://localhost/package2.json", "owner", "b", "2.0.0");
        create_remote_zip_package(&environment, "http://localhost/package3.json", "owner", "name", "2.0.0");
        create_remote_zip_package(&environment, "http://localhost/package4.json", "owner", "name", "2.0.0"); // same version as above
        create_remote_zip_package(&environment, "http://localhost/package5.json", "david", "c", "2.1.1");

        // install the packages
        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        install_url!(environment, "http://localhost/package4.json");
        install_url!(environment, "http://localhost/package5.json");
        environment.clear_logs();

        // check list
        run_cli(vec!["list"], &environment).await.unwrap();
        assert_eq!(
            environment.take_logged_messages(),
            vec!["david/c 2.1.1\nowner/b 2.0.0\nowner/name 1.0.0\nowner/name 2.0.0"]
        );
    }

    #[tokio::test]
    async fn use_command_multiple_command_binaries() {
        let environment = TestEnvironment::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let first_binary_path_second = get_binary_path_second("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        let second_binary_path_second = get_binary_path_second("owner", "name", "2.0.0");

        create_remote_zip_multiple_commands_package(
            &environment,
            "http://localhost/package.json",
            "owner",
            "name",
            "1.0.0",
        );
        create_remote_zip_multiple_commands_package(
            &environment,
            "http://localhost/package2.json",
            "owner",
            "name",
            "2.0.0",
        );

        // install the packages
        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();

        assert_resolves!(&environment, first_binary_path);
        assert_resolves_name!(&environment, "name-second", first_binary_path_second);

        // use the second package
        run_cli(vec!["use", "name", "2.0.0"], &environment).await.unwrap();
        assert_resolves!(&environment, second_binary_path);
        assert_resolves_name!(&environment, "name-second", second_binary_path_second);
    }

    fn add_binary_to_path(environment: &TestEnvironment, name: &str) -> String {
        let path_dir = PathBuf::from("/path-dir");
        if !environment.get_system_path_dirs().contains(&path_dir) {
            environment.add_path_dir(path_dir);
        }
        let path_exe_path = if cfg!(target_os = "windows") {
            format!("/path-dir\\{}.bat", name)
        } else {
            format!("/path-dir/{}", name)
        };
        environment.write_file_text(&PathBuf::from(&path_exe_path), "").unwrap();
        path_exe_path
    }

    fn get_shim_path(name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/data/shims/{}.bat", name)
        } else {
            format!("/data/shims/{}", name)
        }
    }

    fn get_binary_path(owner: &str, name: &str, version: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/data\\binaries\\{}\\{}\\{}\\binary.exe", owner, name, version)
        } else {
            format!("/data/binaries/{}/{}/{}/binary", owner, name, version)
        }
    }

    fn get_binary_path_second(owner: &str, name: &str, version: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/data\\binaries\\{}\\{}\\{}\\second-binary.exe", owner, name, version)
        } else {
            format!("/data/binaries/{}/{}/{}/second-binary", owner, name, version)
        }
    }

    fn create_bvmrc(environment: &TestEnvironment, binaries: Vec<&str>) {
        let mut text = String::new();
        for (i, binary) in binaries.into_iter().enumerate() {
            if i > 0 {
                text.push_str(",");
            }
            text.push_str(&format!("\"{}\"", binary));
        }
        environment
            .write_file_text(
                &PathBuf::from("/project/.bvmrc.json"),
                &format!(r#"{{"binaries": [{}]}}"#, text),
            )
            .unwrap();
    }

    fn create_remote_zip_package(environment: &TestEnvironment, url: &str, owner: &str, name: &str, version: &str) {
        let windows_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-windows.zip",
            version, name
        );
        let windows_checksum = create_remote_zip(environment, &windows_zip_url, true);
        let mac_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-mac.zip",
            version, name
        );
        let mac_checksum = create_remote_zip(environment, &mac_zip_url, false);
        let linux_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-linux.zip",
            version, name
        );
        let linux_checksum = create_remote_zip(environment, &linux_zip_url, false);

        let file_text = format!(
            r#"{{
    "schemaVersion": 1,
    "owner": "{}",
    "name": "{}",
    "version": "{}",
    "windows-x86_64": {{
        "url": "{}",
        "type": "zip",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary.exe"
        }}]
    }},
    "linux-x86_64": {{
        "url": "{}",
        "type": "zip",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary"
        }}]
    }},
    "darwin-x86_64": {{
        "url": "{}",
        "type": "zip",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary"
        }}]
    }}
}}"#,
            owner,
            name,
            version,
            windows_zip_url,
            windows_checksum,
            linux_zip_url,
            linux_checksum,
            mac_zip_url,
            mac_checksum
        );
        environment.add_remote_file(url, Bytes::from(file_text));
    }

    fn create_remote_zip_multiple_commands_package(
        environment: &TestEnvironment,
        url: &str,
        owner: &str,
        name: &str,
        version: &str,
    ) {
        let windows_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-windows.zip",
            version, name
        );
        let windows_checksum = create_remote_zip(environment, &windows_zip_url, true);
        let mac_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-mac.zip",
            version, name
        );
        let mac_checksum = create_remote_zip(environment, &mac_zip_url, false);
        let linux_zip_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-linux.zip",
            version, name
        );
        let linux_checksum = create_remote_zip(environment, &linux_zip_url, false);

        let file_text = format!(
            r#"{{
    "schemaVersion": 1,
    "owner": "{}",
    "name": "{}",
    "version": "{}",
    "windows-x86_64": {{
        "url": "{}",
        "type": "zip",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary.exe"
        }}, {{
            "name": "{1}-second",
            "path": "second-binary.exe"
        }}]
    }},
    "linux-x86_64": {{
        "url": "{}",
        "type": "zip",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary"
        }}, {{
            "name": "{1}-second",
            "path": "second-binary"
        }}]
    }},
    "darwin-x86_64": {{
        "url": "{}",
        "type": "zip",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary"
        }}, {{
            "name": "{1}-second",
            "path": "second-binary"
        }}]
    }}
}}"#,
            owner,
            name,
            version,
            windows_zip_url,
            windows_checksum,
            linux_zip_url,
            linux_checksum,
            mac_zip_url,
            mac_checksum
        );
        environment.add_remote_file(url, Bytes::from(file_text));
    }

    fn create_remote_zip(environment: &TestEnvironment, url: &str, is_windows: bool) -> String {
        let buf: Vec<u8> = Vec::new();
        let w = std::io::Cursor::new(buf);
        let mut zip = zip::ZipWriter::new(w);
        let options = zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let file_name = if is_windows { "binary.exe" } else { "binary" };
        zip.start_file(file_name, options).unwrap();
        zip.write(format!("test-{}", url).as_bytes()).unwrap();
        let file_name = if is_windows {
            "second-binary.exe"
        } else {
            "second-binary"
        };
        zip.start_file(file_name, options).unwrap();
        zip.write(format!("test-{}2", url).as_bytes()).unwrap();
        let result = zip.finish().unwrap().into_inner();
        let zip_file_checksum = crate::utils::get_sha256_checksum(&result);
        environment.add_remote_file(url, Bytes::from(result));
        zip_file_checksum
    }

    fn create_remote_tar_gz_package(environment: &TestEnvironment, url: &str, owner: &str, name: &str, version: &str) {
        let windows_tar_gz_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-windows.tar.gz",
            version, name
        );
        let windows_checksum = create_remote_tar_gz(environment, &windows_tar_gz_url, true);
        let mac_tar_gz_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-mac.tar.gz",
            version, name
        );
        let mac_checksum = create_remote_tar_gz(environment, &mac_tar_gz_url, false);
        let linux_tar_gz_url = format!(
            "https://github.com/dsherret/bvm/releases/download/{}/{}-linux.tar.gz",
            version, name
        );
        let linux_checksum = create_remote_tar_gz(environment, &linux_tar_gz_url, false);

        let file_text = format!(
            r#"{{
    "schemaVersion": 1,
    "owner": "{}",
    "name": "{}",
    "version": "{}",
    "windows-x86_64": {{
        "url": "{}",
        "type": "tar.gz",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary.exe"
        }}]
    }},
    "linux-x86_64": {{
        "url": "{}",
        "type": "tar.gz",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary"
        }}]
    }},
    "darwin-x86_64": {{
        "url": "{}",
        "type": "tar.gz",
        "checksum": "{}",
        "commands": [{{
            "name": "{1}",
            "path": "binary"
        }}]
    }}
}}"#,
            owner,
            name,
            version,
            windows_tar_gz_url,
            windows_checksum,
            linux_tar_gz_url,
            linux_checksum,
            mac_tar_gz_url,
            mac_checksum
        );
        environment.add_remote_file(url, Bytes::from(file_text));
    }

    fn create_remote_tar_gz(environment: &TestEnvironment, url: &str, is_windows: bool) -> String {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let buf: Vec<u8> = Vec::new();
        let w = std::io::Cursor::new(buf);
        let mut archive = tar::Builder::new(w);
        let file_name = if is_windows { "binary.exe" } else { "binary" };
        let data = format!("test-{}", url);
        let mut header = tar::Header::new_gnu();
        header.set_path(file_name).unwrap();
        header.set_size(data.len() as u64);
        header.set_cksum();
        archive.append(&header, data.as_bytes()).unwrap();
        archive.finish().unwrap();

        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(&archive.into_inner().unwrap().into_inner()).unwrap();
        let result = e.finish().unwrap();

        let zip_file_checksum = crate::utils::get_sha256_checksum(&result);
        environment.add_remote_file(url, Bytes::from(result));
        zip_file_checksum
    }

    async fn run_cli(args: Vec<&str>, environment: &TestEnvironment) -> Result<(), ErrBox> {
        let mut args: Vec<String> = args.into_iter().map(String::from).collect();
        args.insert(0, String::from(""));
        run(environment, args).await
    }
}
