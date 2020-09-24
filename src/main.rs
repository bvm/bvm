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
use std::collections::HashSet;
use std::path::PathBuf;

use arg_parser::*;
use dprint_cli_core::checksums::{get_sha256_checksum, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;
use environment::{Environment, SYS_PATH_DELIMITER};
use plugins::{helpers as plugin_helpers, PluginsManifest, PluginsMut, UrlInstallAction};
use types::{CommandName, PathOrVersionSelector, VersionSelector};

#[tokio::main]
async fn main() -> Result<(), ErrBox> {
    let environment = environment::RealEnvironment::new(false);
    let args = std::env::args().collect();
    match run(&environment, args).await {
        Ok(_) => {}
        Err(err) => {
            eprintln!("{}", err.to_string());
            environment.exit(1).unwrap();
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
        SubCommand::Use => handle_use_command(environment).await?,
        SubCommand::UseBinary(command) => handle_use_binary_command(environment, command)?,
        SubCommand::List => handle_list_command(environment)?,
        SubCommand::Init => handle_init_command(environment)?,
        SubCommand::ClearUrlCache => handle_clear_url_cache(environment)?,
        SubCommand::Registry(command) => handle_registry_command(environment, command).await?,
        SubCommand::Add(command) => handle_add_command(environment, command).await?,
        SubCommand::Shell(command) => handle_shell_command(environment, command)?,
    }

    Ok(())
}

fn handle_resolve_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    resolve_command: ResolveCommand,
) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment)?;
    let command_name = CommandName::from_string(resolve_command.binary_name);
    let info = get_executable_path_from_config_file(environment, &plugin_manifest, &command_name)?;
    let executable_path = if let Some(info) = info {
        if let Some(executable_path) = info.executable_path {
            Some(executable_path.clone())
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
    let executable_path = match executable_path {
        Some(path) => path,
        None => plugin_helpers::get_global_binary_file_name(environment, &plugin_manifest, &command_name)?,
    };

    environment.log(&executable_path.to_string_lossy());

    Ok(())
}

async fn handle_install_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: InstallCommand,
) -> Result<(), ErrBox> {
    let (_, config_file) = get_config_file_or_error(environment)?;
    let mut plugins = PluginsMut::load(environment)?;

    if let Some(pre_install) = &config_file.on_pre_install {
        environment.run_shell_command(&environment.cwd()?, pre_install)?;
    }

    for binary in config_file.binaries.iter() {
        match install_binary(&mut plugins, &binary.path, binary.version.as_ref(), command.force).await {
            Err(err) => return err!("Error installing {}: {}", &binary.path.path_or_url, err.to_string()),
            _ => {}
        }
    }

    if command.use_command {
        for entry in config_file.binaries.iter() {
            if let Some(binary) = plugins.get_installed_binary_for_config_binary(&entry).await? {
                let identifier = binary.get_identifier();
                for command_name in binary.get_command_names() {
                    plugins.use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
                }
            }
        }
        plugins.save()?;
    }

    if let Some(post_install) = &config_file.on_post_install {
        environment.run_shell_command(&environment.cwd()?, post_install)?;
    }

    Ok(())
}

async fn install_binary<TEnvironment: Environment>(
    plugins: &mut PluginsMut<TEnvironment>,
    checksum_url: &ChecksumPathOrUrl,
    version_selector: Option<&VersionSelector>,
    force: bool,
) -> Result<(), ErrBox> {
    let install_action = plugins
        .get_url_install_action(checksum_url, version_selector, force)
        .await?;
    if let UrlInstallAction::Install(plugin_file) = install_action {
        // setup the plugin
        let binary_item = plugins.setup_plugin(&plugin_file).await?;
        let identifier = binary_item.get_identifier();
        // check if there is a global binary location set and if not, set it
        for command_name in binary_item.get_command_names() {
            plugins.set_global_binary_if_not_set(&identifier, &command_name)?;
        }
        plugins.save()?; // write for every setup plugin in case a further one fails
    }
    Ok(())
}

async fn handle_install_url_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: InstallUrlCommand,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment)?;
    let url = resolve_url_or_name(environment, &command.url_or_name).await?;

    let result = install_url(environment, &mut plugins, &url, &command).await;
    match result {
        Ok(()) => {}
        Err(err) => return err!("Error installing {}. {}", url.path_or_url, err.to_string()),
    }

    if command.use_command {
        let identifier = plugins
            .manifest
            .get_identifier_from_url(&url)
            .map(|identifier| identifier.clone())
            .unwrap();
        let command_names = plugins.manifest.get_binary(&identifier).unwrap().get_command_names();
        for command_name in command_names {
            let is_command_in_config_file =
                get_is_command_in_config_file(environment, &plugins.manifest, &command_name);
            plugins.use_global_version(
                command_name.clone(),
                plugins::GlobalBinaryLocation::Bvm(identifier.clone()),
            )?;
            if is_command_in_config_file {
                display_command_in_config_file_error(environment, &command_name);
            }
        }
    }

    plugins.save()?;

    return Ok(());

    async fn install_url<TEnvironment: Environment>(
        environment: &TEnvironment,
        plugins: &mut PluginsMut<TEnvironment>,
        url: &ChecksumPathOrUrl,
        command: &InstallUrlCommand,
    ) -> Result<(), ErrBox> {
        let install_action = plugins.get_url_install_action(url, None, command.force).await?;

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

                let binary_item = plugins.setup_plugin(&plugin_file).await?;
                let identifier = binary_item.get_identifier();
                let binary_name = binary_item.name.clone();
                let version = binary_item.version.clone();
                let command_names = binary_item.get_command_names();

                // set this back as being the global version if setup is successful
                for command_name in previous_global_command_names {
                    if command_names.contains(&command_name) {
                        plugins
                            .use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
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
                            command_names
                                .into_iter()
                                .map(|c| format!("'{}'", c))
                                .collect::<Vec<_>>()
                                .join(", "),
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

async fn resolve_url_or_name<TEnvironment: Environment>(
    environment: &TEnvironment,
    url_or_name: &UrlOrName,
) -> Result<ChecksumPathOrUrl, ErrBox> {
    return match url_or_name {
        UrlOrName::Url(url) => Ok(url.to_owned()),
        UrlOrName::Name(name) => {
            let registry = registry::Registry::load(environment)?;
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

            // now get the url
            let urls = url_results.into_iter().map(|r| r.url).collect();
            let selected_url = if let Some(version) = &name.version_selector {
                find_url(environment, &urls, |item| version.matches(&item.version)).await?
            } else {
                find_latest_url(environment, &urls).await?
            };
            if let Some(selected_url) = selected_url {
                Ok(selected_url)
            } else {
                if let Some(version) = &name.version_selector {
                    err!(
                        "Could not find binary {} matching {} in any registry.",
                        name.name_selector,
                        version
                    )
                } else {
                    return err!("Could not find binary {} in any registry.", name.name_selector);
                }
            }
        }
    };

    async fn find_url<TEnvironment: Environment>(
        environment: &TEnvironment,
        urls: &Vec<String>,
        is_match: impl Fn(&registry::RegistryVersionInfo) -> bool,
    ) -> Result<Option<ChecksumPathOrUrl>, ErrBox> {
        let mut best_match: Option<registry::RegistryVersionInfo> = None;
        for url in urls.iter() {
            let registry_file = registry::download_registry_file(environment, &url).await?;
            for version_info in registry_file.versions {
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

        Ok(best_match.map(|version_info| version_info.get_url()))
    }

    async fn find_latest_url<TEnvironment: Environment>(
        environment: &TEnvironment,
        urls: &Vec<String>,
    ) -> Result<Option<ChecksumPathOrUrl>, ErrBox> {
        let mut latest_pre_release: Option<registry::RegistryVersionInfo> = None;
        let mut latest_release: Option<registry::RegistryVersionInfo> = None;
        for url in urls.iter() {
            let registry_file = registry::download_registry_file(environment, &url).await?;
            for item in registry_file.versions {
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

        Ok(latest_release.or(latest_pre_release).map(|item| item.get_url()))
    }
}

fn handle_uninstall_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    uninstall_command: UninstallCommand,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment)?;
    let binary = plugin_helpers::get_binary_with_name_and_version(
        &plugins.manifest,
        &uninstall_command.name_selector,
        &uninstall_command.version.to_selector(),
    )?;
    let plugin_dir = plugins::get_plugin_dir(environment, &binary.name, &binary.version)?;
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

async fn handle_use_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    // use all the binaries in the current configuration file
    let mut plugins = PluginsMut::load(environment)?;
    let (_, config_file) = get_config_file_or_error(environment)?;
    let mut found_not_installed = false;

    for entry in config_file.binaries.iter() {
        if let Some(binary) = plugins.get_installed_binary_for_config_binary(&entry).await? {
            let identifier = binary.get_identifier();
            for command_name in binary.get_command_names() {
                plugins.use_global_version(command_name, plugins::GlobalBinaryLocation::Bvm(identifier.clone()))?;
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
    let mut plugins = PluginsMut::load(environment)?;
    let command_names = match &use_command.version {
        PathOrVersionSelector::Path => {
            // get the current binaries for the selector
            let binaries = plugins.manifest.get_binaries_matching_name(&use_command.name_selector);
            let have_same_owner = plugin_helpers::get_have_same_owner(&binaries);
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
                    use_command.name_selector.name,
                    binary_names.join("\n  ")
                );
            }

            let mut pre_release_binaries = binaries
                .iter()
                .filter(|b| b.version.is_prerelease())
                .collect::<Vec<_>>();
            let mut binaries = binaries
                .iter()
                .filter(|b| !b.version.is_prerelease())
                .collect::<Vec<_>>();
            if let Some(latest_binary) = binaries.pop().or(pre_release_binaries.pop()) {
                latest_binary.get_command_names()
            } else {
                return err!(
                    "Could not find any installed binaries named '{}'.",
                    use_command.name_selector
                );
            }
        }
        PathOrVersionSelector::Version(version_selector) => {
            // todo: select version based on version selector
            let binary = plugin_helpers::get_binary_with_name_and_version(
                &plugins.manifest,
                &use_command.name_selector,
                &version_selector,
            )?;
            binary.get_command_names()
        }
    };

    for command_name in command_names {
        let is_command_in_config_file = get_is_command_in_config_file(environment, &plugins.manifest, &command_name);
        match &use_command.version {
            PathOrVersionSelector::Path => {
                if utils::get_path_executable_path(environment, &command_name)?.is_none() {
                    return err!(
                        "Could not find any installed binaries on the path that matched '{}'.",
                        command_name
                    );
                }
                plugins.use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Path)?;
            }
            PathOrVersionSelector::Version(version_selector) => {
                let binary = plugin_helpers::get_binary_with_name_and_version(
                    &plugins.manifest,
                    &use_command.name_selector,
                    &version_selector,
                )?;
                let identifier = binary.get_identifier();
                plugins.use_global_version(command_name.clone(), plugins::GlobalBinaryLocation::Bvm(identifier))?;
            }
        }

        if is_command_in_config_file {
            display_command_in_config_file_error(environment, &command_name);
        }
    }
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
        Ok(result) => result.map(|info| info.executable_path).flatten().is_some(),
        Err(_) => false,
    }
}

fn display_command_in_config_file_error(environment: &impl Environment, command_name: &CommandName) {
    let message = format!(
        concat!(
            "Updated globally used version of '{}', but local version remains using version specified ",
            "in the current working directory's config file. If you wish to change the local version, ",
            "then update your configuration file (check the cwd and ancestor directories for a .bvmrc.json file)."
        ),
        command_name
    );
    environment.log_error(&message);
}

fn handle_list_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment)?;
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
    } else {
        environment.write_file_text(&config_path, "{\n  \"binaries\": [\n  ]\n}\n")?;
        environment.log(&format!("Created {}", configuration::CONFIG_FILE_NAME));
        Ok(())
    }
}

fn handle_clear_url_cache<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment)?;
    plugins.clear_cached_urls();
    plugins.save()?;
    Ok(())
}

async fn handle_registry_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    sub_command: RegistrySubCommand,
) -> Result<(), ErrBox> {
    match sub_command {
        RegistrySubCommand::Add(command) => handle_registry_add_command(environment, command).await,
        RegistrySubCommand::Remove(command) => handle_registry_remove_command(environment, command),
        RegistrySubCommand::List => handle_registry_list_command(environment),
    }
}

async fn handle_add_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: AddCommand,
) -> Result<(), ErrBox> {
    let url = resolve_url_or_name(environment, &command.url_or_name).await?;
    let (config_file_path, config_file) = get_config_file_or_error(environment)?;
    let mut plugins = PluginsMut::load(environment)?;

    // install the binary
    install_binary(&mut plugins, &url, None, false).await?;
    let binary_identifier = plugins.manifest.get_identifier_from_url(&url).unwrap().clone();
    let binary_name = binary_identifier.get_binary_name();

    // get the replace index if this binary name is already in the config file
    let mut replace_index = None;
    for (i, config_binary) in config_file.binaries.iter().enumerate() {
        // ignore errors when associating
        if let Err(err) = plugins.ensure_url_associated(&config_binary.path).await {
            environment.log_error(&format!(
                "Error associating {}. {}",
                &config_binary.path.path_or_url,
                err.to_string()
            ));
        } else {
            let config_binary_name = plugins
                .manifest
                .get_identifier_from_url(&config_binary.path)
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
    let checksum = match url.checksum {
        Some(checksum) => checksum,
        None => {
            let url_file_bytes = environment.download_file(&url.path_or_url).await?;
            get_sha256_checksum(&url_file_bytes)
        }
    };

    configuration::add_binary_to_config_file(
        environment,
        &config_file_path,
        &configuration::ConfigFileBinary {
            path: ChecksumPathOrUrl {
                path_or_url: url.path_or_url,
                checksum: Some(checksum),
            },
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

async fn handle_registry_add_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: RegistryAddCommand,
) -> Result<(), ErrBox> {
    let mut registry = registry::Registry::load(environment)?;
    let registry_file = registry::download_registry_file(environment, &command.url).await?;
    registry.add_url(registry_file.get_binary_name(), command.url);
    registry.save(environment)?;
    Ok(())
}

fn handle_registry_remove_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: RegistryRemoveCommand,
) -> Result<(), ErrBox> {
    let mut registry = registry::Registry::load(environment)?;
    registry.remove_url(&command.url);
    registry.save(environment)?;
    Ok(())
}

fn handle_registry_list_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let registry = registry::Registry::load(environment)?;
    let mut items = registry.items();

    items.sort_by(|a, b| a.compare(b));

    let lines = items.into_iter().map(|item| item.display()).collect::<Vec<_>>();

    if !lines.is_empty() {
        environment.log(&lines.join("\n"));
    }
    Ok(())
}

fn handle_shell_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: ShellSubCommand,
) -> Result<(), ErrBox> {
    match command {
        ShellSubCommand::GetNewPath(command) => handle_shell_get_new_path_command(environment, command),
        ShellSubCommand::ClearPendingChanges => handle_shell_clear_pending_env_changes_command(environment),
        ShellSubCommand::GetPaths => handle_shell_get_paths_command(environment),
        #[cfg(target_os = "windows")]
        ShellSubCommand::WindowsInstall(command) => handle_shell_windows_install_command(environment, command),
        #[cfg(target_os = "windows")]
        ShellSubCommand::WindowsUninstall(command) => handle_shell_windows_uninstall_command(environment, command),
    }
}

fn handle_shell_get_new_path_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: ShellGetNewPathCommand,
) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment)?;
    let local_data_dir = environment.get_local_user_data_dir()?;
    let mut paths = command
        .current_sys_path
        .split(&SYS_PATH_DELIMITER)
        .map(String::from)
        .collect::<Vec<_>>();

    for path in plugin_manifest.get_relative_pending_added_paths() {
        let path = local_data_dir.join(path).to_string_lossy().to_string();
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    for path in plugin_manifest.get_relative_pending_removed_paths() {
        let path = local_data_dir.join(path).to_string_lossy().to_string();
        if let Some(pos) = paths.iter().position(|x| x == &path) {
            paths.remove(pos);
        }
    }

    environment.log(
        &paths
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(SYS_PATH_DELIMITER),
    );

    Ok(())
}

fn handle_shell_clear_pending_env_changes_command<TEnvironment: Environment>(
    environment: &TEnvironment,
) -> Result<(), ErrBox> {
    let mut plugins = PluginsMut::load(environment)?;
    plugins.clear_pending_env_changes();
    plugins.save()?;

    Ok(())
}

fn handle_shell_get_paths_command<TEnvironment: Environment>(environment: &TEnvironment) -> Result<(), ErrBox> {
    let plugin_manifest = PluginsManifest::load(environment)?;
    let local_data_dir = environment.get_local_user_data_dir()?;
    let path_text = plugin_manifest
        .get_bin_env_paths()
        .iter()
        .map(|path| local_data_dir.join(path).to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(SYS_PATH_DELIMITER);

    environment.log(&path_text);

    Ok(())
}

#[cfg(target_os = "windows")]
fn handle_shell_windows_install_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: ShellWindowsInstallCommand,
) -> Result<(), ErrBox> {
    let data_dir = environment.get_user_data_dir()?;
    environment.ensure_system_path_pre(&PathBuf::from(&command.install_path).to_string_lossy())?;
    environment.ensure_system_path_pre(&PathBuf::from(data_dir).join("shims").to_string_lossy())?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn handle_shell_windows_uninstall_command<TEnvironment: Environment>(
    environment: &TEnvironment,
    command: ShellWindowsUninstallCommand,
) -> Result<(), ErrBox> {
    let data_dir = environment.get_user_data_dir()?;
    environment.remove_system_path(&PathBuf::from(&command.install_path).to_string_lossy())?;
    environment.remove_system_path(&PathBuf::from(data_dir).join("shims").to_string_lossy())?;
    Ok(())
}

struct ConfigFileExecutableInfo {
    executable_path: Option<PathBuf>,
    had_uninstalled_binary: bool,
}

fn get_executable_path_from_config_file<TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &PluginsManifest,
    command_name: &CommandName,
) -> Result<Option<ConfigFileExecutableInfo>, ErrBox> {
    Ok(if let Some((_, config_file)) = get_config_file(environment)? {
        let mut had_uninstalled_binary = false;
        let mut executable_path = None;

        for config_binary in config_file.binaries.iter() {
            let binary =
                plugin_helpers::get_installed_binary_if_associated_config_file_binary(plugin_manifest, &config_binary);
            if let Some(binary) = binary {
                for command in binary.commands.iter() {
                    if &command.name == command_name {
                        let plugin_cache_dir = plugins::get_plugin_dir(environment, &binary.name, &binary.version)?;
                        executable_path = Some(plugin_cache_dir.join(&command.path));
                        break;
                    }
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

fn get_config_file_or_error(environment: &impl Environment) -> Result<(PathBuf, configuration::ConfigFile), ErrBox> {
    match get_config_file(environment)? {
        Some(config_file) => Ok(config_file),
        None => return err!("Could not find .bvmrc.json in the current directory or its ancestors. Perhaps created one with `bvm init`?"),
    }
}

fn get_config_file(environment: &impl Environment) -> Result<Option<(PathBuf, configuration::ConfigFile)>, ErrBox> {
    if let Some(config_file_path) = configuration::find_config_file(environment)? {
        let config_file_text = environment.read_file_text(&config_file_path)?;
        match configuration::read_config_file(&config_file_text) {
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
    use crate::environment::{Environment, TestEnvironment, PATH_SEPARATOR, SYS_PATH_DELIMITER};
    use crate::test_builders::{EnvironmentBuilder, PluginDownloadType};
    use dprint_cli_core::types::ErrBox;

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
        assert_eq!(logged_messages, [format!("bvm {}", env!("CARGO_PKG_VERSION"))]);
    }

    #[tokio::test]
    async fn should_init() {
        let environment = TestEnvironment::new();
        run_cli(vec!["init"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(logged_messages, ["Created .bvmrc.json"]);
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
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let environment = builder.build();

        // install the package
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);

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
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let path_exe_path = builder.add_binary_to_path("name");
        let environment = builder.build();

        // install the package
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 1.0.0...",
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
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0...",]);

        // now install the second
        install_url!(environment, "http://localhost/package2.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 2.0.0...",
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
        assert_eq!(logged_errors, ["Extracting archive for owner/name 3.0.0...",]);
        assert_resolves!(&environment, third_binary_path);

        // install the fourth package
        install_url!(environment, "http://localhost/package4.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 4.0.0...",
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
            ["Already installed. Provide the `--force` flag to reinstall."]
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
        assert_eq!(logged_errors, ["Extracting archive for owner/name 4.0.0...",]);
        assert_resolves!(&environment, fourth_binary_path);
        assert_resolves_name!(&environment, "name-second", fourth_binary_path_second);
    }

    #[tokio::test]
    async fn install_url_command_tar_gz() {
        let builder = EnvironmentBuilder::new();
        let binary_path = get_binary_path("owner", "name", "1.0.0");

        builder.create_remote_tar_gz_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let environment = builder.build();

        // install and check setup
        install_url!(environment, "http://localhost/package.json");
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0...",]);
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

    #[tokio::test]
    async fn install_url_command_use_with_config_file_same_command() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner2", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // install the package
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        environment.clear_logs();

        // install and use the other package
        run_cli(vec!["install", "--use", "http://localhost/package2.json"], &environment)
            .await
            .unwrap();
        assert_eq!(
            environment.take_logged_errors(),
            [
                "Extracting archive for owner2/name 2.0.0...",
                concat!(
                    "Updated globally used version of 'name', but local version remains using version specified ",
                    "in the current working directory's config file. If you wish to change the local version, ",
                    "then update your configuration file (check the cwd and ancestor directories for a .bvmrc.json file)."
                )
            ]
        );

        // should still resolve to the cwd's binary
        assert_resolves!(&environment, first_binary_path);
    }

    #[tokio::test]
    async fn install_url_pre_and_post_install() {
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
        assert_eq!(
            environment.take_logged_errors(),
            ["Extracting archive for owner/name 1.0.0..."]
        );
        assert_eq!(
            environment.take_run_shell_commands(),
            [
                (first_bin_dir.clone(), "command1".to_string()),
                (first_bin_dir.clone(), "command2".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn install_command_no_existing_binary() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // attempt to install in directory that doesn't have the config file
        let error_text = run_cli(vec!["install"], &environment).await.err().unwrap().to_string();
        assert_eq!(
            error_text,
            "Could not find .bvmrc.json in the current directory or its ancestors. Perhaps created one with `bvm init`?"
        );

        // move to the correct dir, then try again
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);

        // now try to resolve the binary
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // go up a directory and it should resolve
        environment.set_cwd("/");
        assert_resolves!(environment, binary_path);
    }

    #[tokio::test]
    async fn install_command_previous_install_binary() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        let second_binary_path = get_binary_path("owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package2.json"]);
        let environment = builder.build();

        // install a package globally
        run_cli(vec!["install", "http://localhost/package.json"], &environment)
            .await
            .unwrap();
        environment.clear_logs();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 2.0.0..."]);

        // now try to resolve the binary
        assert_resolves!(environment, second_binary_path);

        // try reinstalling, it should not output anything
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors.len(), 0);

        // try reinstalling, but provide --force
        run_cli(vec!["install", "--force"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 2.0.0..."]);

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
        let builder = EnvironmentBuilder::new();
        let path_exe_path = builder.add_binary_to_path("name");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // run the install command in the correct directory
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);

        // now try to resolve the binary
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);

        // go up a directory and it should resolve to binary on the path still
        environment.set_cwd("/");
        assert_resolves!(environment, path_exe_path);
    }

    #[tokio::test]
    async fn install_command_pre_post_install() {
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
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
        let logged_shell_commands = environment.take_run_shell_commands();
        assert_eq!(
            logged_shell_commands,
            [
                ("/project".to_string(), "echo \"Test\"".to_string()),
                ("/project".to_string(), "echo \"Hello world!\"".to_string())
            ]
        );
    }

    #[tokio::test]
    async fn install_command_binary_object() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object("http://localhost/package.json", None, None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
    }

    #[tokio::test]
    async fn install_command_binary_object_checksum() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object("http://localhost/package.json", Some(checksum.as_str()), None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
    }

    #[tokio::test]
    async fn install_command_binary_object_checksum_incorrect() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object("http://localhost/package.json", Some("incorrect-checksum"), None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        let error = run_cli(vec!["install"], &environment).await.err().unwrap();
        assert_eq!(error.to_string(), format!("Error installing http://localhost/package.json: The checksum {} did not match the expected checksum of incorrect-checksum.", checksum));
    }

    #[tokio::test]
    async fn install_command_binary_object_url_checksum_incorrect() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json@incorrect-checksum"), None, None)
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        let error = run_cli(vec!["install"], &environment).await.err().unwrap();
        assert_eq!(error.to_string(), format!("Error installing http://localhost/package.json: The checksum {} did not match the expected checksum of incorrect-checksum.", checksum));
    }

    #[tokio::test]
    async fn install_command_binary_object_existing_matching_version() {
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
        run_cli(vec!["install"], &environment).await.unwrap();
        assert_eq!(environment.take_logged_errors().is_empty(), true);
        assert_resolves!(environment, get_binary_path("owner", "name", "1.1.0"));
    }

    #[tokio::test]
    async fn install_command_binary_object_existing_matching_version_major_minor() {
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
        run_cli(vec!["install"], &environment).await.unwrap();
        assert_eq!(environment.take_logged_errors().is_empty(), true);
        assert_resolves!(environment, get_binary_path("owner", "name", "1.3.0"));
    }

    #[tokio::test]
    async fn install_command_binary_object_non_existing_matching_version() {
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
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
        assert_resolves!(environment, get_binary_path("owner", "name", "1.0.0"));
    }

    #[tokio::test]
    async fn install_command_binary_object_version_not_match_path_errors() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .add_binary_object(&format!("http://localhost/package.json"), None, Some("1.1"))
            .build();
        let environment = builder.build();
        environment.set_cwd("/project");

        let err = run_cli(vec!["install"], &environment).await.err().unwrap();
        assert_eq!(err.to_string(), "Error installing http://localhost/package.json: The specified version '1.1' did not match '1.0.0' in the path file. Please specify a different path or update the version.");

        // should still resolve when installed without error
        install_url!(environment, "http://localhost/package.json");
        environment.clear_logs();
        assert_resolves!(environment, get_binary_path("owner", "name", "1.0.0"));
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
        assert_eq!(
            error_message.to_string(),
            "Error reading /.bvmrc.json: Unknown key 'test'"
        );
    }

    #[tokio::test]
    async fn uninstall_command_binary_on_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let path_exe_path = builder.add_binary_to_path("name");
        let environment = builder.build();

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
        run_cli(vec!["list"], &environment).await.unwrap();
        assert_eq!(
            environment.take_logged_messages(),
            ["david/c 2.1.1\nowner/b 2.0.0\nowner/name 1.0.0\nowner/name 2.0.0"]
        );
    }

    #[tokio::test]
    async fn use_command_multiple_command_binaries() {
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
        run_cli(vec!["use", "name", "2.1.0"], &environment).await.unwrap();
        assert_resolves!(&environment, third_binary_path);
        assert_resolves_name!(&environment, "name-second", third_binary_path_second);

        // specify only major
        run_cli(vec!["use", "name", "2"], &environment).await.unwrap();
        assert_resolves!(&environment, fourth_binary_path);

        // specify minor
        run_cli(vec!["use", "name", "2.0"], &environment).await.unwrap();
        assert_resolves!(&environment, second_binary_path);
        run_cli(vec!["use", "name", "2.1"], &environment).await.unwrap();
        assert_resolves!(&environment, fourth_binary_path);

        // specify caret
        run_cli(vec!["use", "name", "^2.0"], &environment).await.unwrap();
        assert_resolves!(&environment, fourth_binary_path);

        // specify tilde
        run_cli(vec!["use", "name", "~2.0"], &environment).await.unwrap();
        assert_resolves!(&environment, second_binary_path);

        // specify none (use latest, but not pre releases)
        run_cli(vec!["use", "name"], &environment).await.unwrap();
        assert_resolves!(&environment, fourth_binary_path);
    }

    #[tokio::test]
    async fn use_command_config_file_same_command() {
        let builder = EnvironmentBuilder::new();
        let first_binary_path = get_binary_path("owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_remote_zip_package("http://localhost/package2.json", "owner2", "name", "2.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();

        // install the package
        environment.set_cwd("/project");
        run_cli(vec!["install"], &environment).await.unwrap();

        // install the other package
        install_url!(environment, "http://localhost/package2.json");
        environment.clear_logs();

        // now try to use it
        run_cli(vec!["use", "name", "2.0.0"], &environment).await.unwrap();
        assert_eq!(
            environment.take_logged_errors(),
            [concat!(
                "Updated globally used version of 'name', but local version remains using version specified ",
                "in the current working directory's config file. If you wish to change the local version, ",
                "then update your configuration file (check the cwd and ancestor directories for a .bvmrc.json file)."
            )]
        );

        // should still resolve to the cwd's binary
        assert_resolves!(&environment, first_binary_path);
    }

    #[tokio::test]
    async fn use_command_different_owners_path() {
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

        run_cli(vec!["use", "owner2/name", "1.0.0"], &environment)
            .await
            .unwrap();

        assert_resolves!(&environment, second_binary_path);
        assert_resolves_name!(&environment, "name-second", second_binary_path_second);

        // error when not specifying the owner and there are multiple on the path
        let err_message = run_cli(vec!["use", "name", "path"], &environment).await.err().unwrap();
        assert_eq!(err_message.to_string(), "There were multiple binaries with the name 'name'. Please include the owner in the name:\n  owner/name\n  owner2/name");

        // should be ok when specifying other one
        run_cli(vec!["use", "owner/name", "path"], &environment).await.unwrap();
        assert_resolves!(&environment, path_binary_path);
        assert_resolves_name!(&environment, "name-second", path_second_binary_path);
    }

    #[tokio::test]
    async fn use_command_on_use_on_stop_use() {
        let builder = EnvironmentBuilder::new();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package.json", "owner", "name", "1.0.0");
        plugin_builder.on_use("command1");
        plugin_builder.on_stop_use("command2");
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "3.0.0");
        let environment = builder.build();
        let first_bin_dir = get_binary_dir("owner", "name", "1.0.0");

        // install the packages
        install_url!(environment, "http://localhost/package.json");
        let commands = environment.take_run_shell_commands();
        assert_eq!(commands, [(first_bin_dir.clone(), "command1".to_string())]); // should use
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        environment.clear_logs();

        run_cli(vec!["use", "name", "2.0.0"], &environment).await.unwrap();
        let commands = environment.take_run_shell_commands();
        assert_eq!(commands, [(first_bin_dir.clone(), "command2".to_string())]);
        run_cli(vec!["use", "name", "3.0.0"], &environment).await.unwrap();
        let commands = environment.take_run_shell_commands();
        assert_eq!(commands.is_empty(), true);
        run_cli(vec!["use", "name", "1.0.0"], &environment).await.unwrap();
        let commands = environment.take_run_shell_commands();
        assert_eq!(commands, [(first_bin_dir, "command1".to_string())]);
    }

    #[tokio::test]
    async fn clear_url_cache_command_path() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_bvmrc(vec!["http://localhost/package.json"]);
        let environment = builder.build();
        environment.set_cwd("/project");

        // install
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);

        // clear the url cache
        run_cli(vec!["clear-url-cache"], &environment).await.unwrap();

        // ensure it still resolves, but it will error
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["[bvm warning]: There were some not installed binaries in the current directory (run `bvm install`). Resolving global 'name'."]);

        // install again, but it shouldn't install because already installed
        run_cli(vec!["install"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors.len(), 0);

        // should resolve without error now
        let binary_path = get_binary_path("owner", "name", "1.0.0");
        assert_resolves!(environment, binary_path);
    }

    #[tokio::test]
    async fn registry_add_remove_list_command_path() {
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

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();
        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap(); // add twice
        run_cli(vec!["registry", "add", "http://localhost/registry2.json"], &environment)
            .await
            .unwrap();
        run_cli(vec!["registry", "add", "http://localhost/registry3.json"], &environment)
            .await
            .unwrap();
        run_cli(vec!["registry", "list"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(logged_messages, ["owner/name - http://localhost/registry.json\nowner/name - http://localhost/registry2.json\nowner2/name2 - http://localhost/registry3.json"]);
        run_cli(
            vec!["registry", "remove", "http://localhost/registry.json"],
            &environment,
        )
        .await
        .unwrap();
        run_cli(
            vec!["registry", "remove", "http://localhost/registry.json"],
            &environment,
        )
        .await
        .unwrap(); // remove twice should silently ignore
        run_cli(vec!["registry", "list"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(
            logged_messages,
            ["owner/name - http://localhost/registry2.json\nowner2/name2 - http://localhost/registry3.json"]
        );
        run_cli(
            vec!["registry", "remove", "http://localhost/registry2.json"],
            &environment,
        )
        .await
        .unwrap();
        run_cli(vec!["registry", "list"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(logged_messages, vec!["owner2/name2 - http://localhost/registry3.json"]);
        run_cli(
            vec!["registry", "remove", "http://localhost/registry3.json"],
            &environment,
        )
        .await
        .unwrap();
        run_cli(vec!["registry", "list"], &environment).await.unwrap();
        let logged_messages = environment.take_logged_messages();
        assert_eq!(logged_messages.len(), 0);
    }

    #[tokio::test]
    async fn registry_install_command() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let checksum2 = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.0.1");
        let checksum3 = builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "1.1.0");
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

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();

        run_cli(vec!["install", "name", "1.0.0"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);

        // install latest when only specifying major
        run_cli(vec!["install", "name", "1"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 1.1.0...",
                "Installed. Run `bvm use name 1.1.0` to use it on the path as 'name'.",
            ]
        );

        // install latest patch when specifying minor
        run_cli(vec!["install", "name", "1.0"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 1.0.1...",
                "Installed. Run `bvm use name 1.0.1` to use it on the path as 'name'.",
            ]
        );

        run_cli(vec!["uninstall", "name", "1.0.1"], &environment).await.unwrap();
        run_cli(vec!["uninstall", "name", "1.1.0"], &environment).await.unwrap();
        environment.clear_logs();

        // install when specifying caret
        run_cli(vec!["install", "name", "^1.0.0"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 1.1.0...",
                "Installed. Run `bvm use name 1.1.0` to use it on the path as 'name'.",
            ]
        );

        // install when specifying tilde
        run_cli(vec!["install", "name", "~1.0.0"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(
            logged_errors,
            [
                "Extracting archive for owner/name 1.0.1...",
                "Installed. Run `bvm use name 1.0.1` to use it on the path as 'name'.",
            ]
        );
    }

    #[tokio::test]
    async fn registry_install_command_latest() {
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

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();

        run_cli(vec!["install", "name"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 2.0.1..."]);
    }

    #[tokio::test]
    async fn registry_install_command_latest_all_pre_releases() {
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

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();

        run_cli(vec!["install", "name"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0-beta..."]);
    }

    #[tokio::test]
    async fn registry_install_command_incorrect_checksum() {
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

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();

        let err = run_cli(vec!["install", "name", "1.0.0"], &environment)
            .await
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            format!(
                "Error installing http://localhost/package.json. The checksum {} did not match the expected checksum of wrong-checksum.",
                checksum
            )
        );
    }

    #[tokio::test]
    async fn registry_install_command_no_registry() {
        let environment = TestEnvironment::new();
        let err = run_cli(vec!["install", "name", "1.0.0"], &environment)
            .await
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            "There were no registries found for the provided binary. Did you mean to add one using `bvm registry add <url>`?",
        );
    }

    #[tokio::test]
    async fn registry_install_command_multiple_owners() {
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

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();
        run_cli(vec!["registry", "add", "http://localhost/registry2.json"], &environment)
            .await
            .unwrap();

        let error = run_cli(vec!["install", "name", "1.0.0"], &environment)
            .await
            .err()
            .unwrap();
        assert_eq!(error.to_string(), "There were multiple binaries with the name 'name'. Please include the owner in the name:\n  owner/name\n  owner2/name");
    }

    #[tokio::test]
    async fn binary_has_environment_variable() {
        let builder = EnvironmentBuilder::new();
        builder.add_binary_to_path("name");
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package.json", "owner", "name", "1.0.0");
        plugin_builder.add_env_path("dir");
        plugin_builder.download_type(PluginDownloadType::Zip);
        plugin_builder.build();
        let mut plugin_builder =
            builder.create_plugin_builder("http://localhost/package2.json", "owner", "name", "2.0.0");
        plugin_builder.add_env_path("dir2");
        plugin_builder.add_env_path(&format!("other{}path", PATH_SEPARATOR));
        plugin_builder.download_type(PluginDownloadType::TarGz);
        plugin_builder.build();
        builder.create_remote_zip_package("http://localhost/package3.json", "owner", "name", "3.0.0");
        let environment = builder.build();

        install_url!(environment, "http://localhost/package.json");
        install_url!(environment, "http://localhost/package2.json");
        install_url!(environment, "http://localhost/package3.json");
        environment.clear_logs();

        run_cli(vec!["use", "name", "1.0.0"], &environment).await.unwrap();

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

        // should have updated the environment with the new path
        run_cli(
            vec![
                "hidden-shell",
                "get-new-path",
                &format!("exiting/path{0}other/path", SYS_PATH_DELIMITER),
            ],
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(
            environment.take_logged_messages(),
            [format!(
                "exiting/path{0}other/path{0}{1}",
                SYS_PATH_DELIMITER, first_path_str
            )]
        );

        // only windows will have updated the system path
        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/path-dir"),
                    PathBuf::from(&first_path_str)
                ]
            );
        }

        // should output correctly when ends with delimiter
        run_cli(
            vec!["hidden-shell", "get-new-path", &format!("test{}", SYS_PATH_DELIMITER)],
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(
            environment.take_logged_messages(),
            [format!("test{0}{1}", SYS_PATH_DELIMITER, first_path_str)]
        );

        // clear the pending changes
        run_cli(vec!["hidden-shell", "clear-pending-changes"], &environment)
            .await
            .unwrap();

        // now the path should return as-is
        run_cli(vec!["hidden-shell", "get-new-path", "test"], &environment)
            .await
            .unwrap();
        assert_eq!(environment.take_logged_messages(), ["test"]);

        // ensure this exists in get-paths
        run_cli(vec!["hidden-shell", "get-paths"], &environment).await.unwrap();
        assert_eq!(environment.take_logged_messages(), [first_path_str]);

        // now switch
        run_cli(vec!["use", "name", "2.0.0"], &environment).await.unwrap();

        // get the new path based on the old one
        run_cli(
            vec![
                "hidden-shell",
                "get-new-path",
                &format!("exiting/path{0}other/path{0}{1}", SYS_PATH_DELIMITER, first_path_str),
            ],
            &environment,
        )
        .await
        .unwrap();

        assert_eq!(
            environment.take_logged_messages(),
            [format!(
                "exiting/path{0}other/path{0}{1}{0}{2}",
                SYS_PATH_DELIMITER, second_path_str1, second_path_str2
            )]
        );

        // clear the pending changes
        run_cli(vec!["hidden-shell", "clear-pending-changes"], &environment)
            .await
            .unwrap();

        // ensure the paths exist in get-paths now
        run_cli(vec!["hidden-shell", "get-paths"], &environment).await.unwrap();

        assert_eq!(
            environment.take_logged_messages(),
            [format!(
                "{}{}{}",
                second_path_str1, SYS_PATH_DELIMITER, second_path_str2
            )]
        );

        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [
                    PathBuf::from("/data/shims"),
                    PathBuf::from("/path-dir"),
                    PathBuf::from(&second_path_str1),
                    PathBuf::from(&second_path_str2)
                ]
            );
        }

        // now switch
        run_cli(vec!["use", "name", "3.0.0"], &environment).await.unwrap();

        // get the new path based on the old one
        run_cli(
            vec![
                "hidden-shell",
                "get-new-path",
                &format!(
                    "exiting/path{0}other/path{0}{1}{0}{2}",
                    SYS_PATH_DELIMITER, second_path_str1, second_path_str2
                ),
            ],
            &environment,
        )
        .await
        .unwrap();

        assert_eq!(
            environment.take_logged_messages(),
            [format!("exiting/path{0}other/path", SYS_PATH_DELIMITER)]
        );

        // clear the pending changes
        run_cli(vec!["hidden-shell", "clear-pending-changes"], &environment)
            .await
            .unwrap();

        // ensure the paths is empty
        run_cli(vec!["hidden-shell", "get-paths"], &environment).await.unwrap();
        assert_eq!(environment.take_logged_messages(), [""]);

        if cfg!(target_os = "windows") {
            assert_eq!(
                environment.get_system_path_dirs(),
                [PathBuf::from("/data/shims"), PathBuf::from("/path-dir")]
            );
        }

        // use the path version then go back to the first
        run_cli(vec!["use", "name", "path"], &environment).await.unwrap();
        run_cli(vec!["use", "name", "1.0.0"], &environment).await.unwrap();

        run_cli(
            vec![
                "hidden-shell",
                "get-new-path",
                &format!("exiting/path{0}other/path", SYS_PATH_DELIMITER),
            ],
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(
            environment.take_logged_messages(),
            [format!(
                "exiting/path{0}other/path{0}{1}",
                SYS_PATH_DELIMITER, first_path_str
            )]
        );
    }

    #[tokio::test]
    async fn add_command_url_no_binaries() {
        let builder = EnvironmentBuilder::new();
        let checksum = builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        builder.create_bvmrc_builder().build();
        let environment = builder.build();

        // run the add command
        environment.set_cwd("/project");
        run_cli(vec!["add", "http://localhost/package.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment
                .read_file_text(&PathBuf::from("/project/.bvmrc.json"))
                .unwrap(),
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

    #[tokio::test]
    async fn add_command_url_other_binary() {
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
        run_cli(vec!["add", "http://localhost/package.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment
                .read_file_text(&PathBuf::from("/project/.bvmrc.json"))
                .unwrap(),
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

    #[tokio::test]
    async fn add_command_registry() {
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
        builder.create_bvmrc_builder().path("/.bvmrc.json").build();
        let environment = builder.build();

        run_cli(vec!["registry", "add", "http://localhost/registry1.json"], &environment)
            .await
            .unwrap();
        run_cli(vec!["registry", "add", "http://localhost/registry2.json"], &environment)
            .await
            .unwrap();

        run_cli(vec!["add", "owner/name", "1.0.0"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);

        run_cli(vec!["add", "other"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/other 2.0.0..."]);

        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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
        run_cli(vec!["add", "other", "~1.0"], &environment).await.unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/other 1.0.0..."]);

        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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
        let err = run_cli(vec!["add", "other", "~1.1"], &environment).await.err().unwrap();
        assert_eq!(
            err.to_string(),
            "Could not find binary other matching ~1.1 in any registry."
        );
    }

    #[tokio::test]
    async fn add_command_existing_url() {
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
            .path("/.bvmrc.json")
            .add_binary_path("http://localhost/package.json")
            .build();
        let environment = builder.build();
        run_cli(vec!["install"], &environment).await.unwrap();
        environment.clear_logs();

        run_cli(vec!["registry", "add", "http://localhost/registry.json"], &environment)
            .await
            .unwrap();
        run_cli(vec!["add", "name", "1"], &environment).await.unwrap();

        // should replace it
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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

    #[tokio::test]
    async fn add_command_existing_package_different_url_replaces() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/.bvmrc.json")
            .add_binary_path("http://localhost/package.json")
            .build();
        let environment = builder.build();

        // should also associate the existing url if not associated
        run_cli(vec!["add", "http://localhost/package2.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 1.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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

    #[tokio::test]
    async fn add_command_existing_package_different_url_replaces_start() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "1.0.0");
        builder.create_remote_zip_package("http://localhost/final.json", "owner", "final", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/.bvmrc.json")
            .add_binary_object("http://localhost/package1.json", None, None)
            .add_binary_object("http://localhost/other.json", None, Some("~1.0.0"))
            .add_binary_object("http://localhost/final.json", None, Some("1"))
            .build();
        let environment = builder.build();

        run_cli(vec!["add", "http://localhost/package2.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 2.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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

    #[tokio::test]
    async fn add_command_existing_package_different_url_replaces_middle() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "1.0.0");
        builder.create_remote_zip_package("http://localhost/final.json", "owner", "final", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/.bvmrc.json")
            .add_binary_object("http://localhost/other.json", None, Some("~1.0.0"))
            .add_binary_object("http://localhost/package1.json", None, None)
            .add_binary_object("http://localhost/final.json", None, Some("1"))
            .build();
        let environment = builder.build();

        run_cli(vec!["add", "http://localhost/package2.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 2.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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

    #[tokio::test]
    async fn add_command_existing_package_different_url_replaces_end() {
        let builder = EnvironmentBuilder::new();
        builder.create_remote_zip_package("http://localhost/package1.json", "owner", "name", "1.0.0");
        let checksum = builder.create_remote_zip_package("http://localhost/package2.json", "owner", "name", "2.0.0");
        builder.create_remote_zip_package("http://localhost/other.json", "owner", "other", "1.0.0");
        builder.create_remote_zip_package("http://localhost/final.json", "owner", "final", "1.0.0");
        builder
            .create_bvmrc_builder()
            .path("/.bvmrc.json")
            .add_binary_object("http://localhost/other.json", None, Some("~1.0.0"))
            .add_binary_object("http://localhost/final.json", None, Some("1"))
            .add_binary_object("http://localhost/package1.json", None, None)
            .build();
        let environment = builder.build();

        run_cli(vec!["add", "http://localhost/package2.json"], &environment)
            .await
            .unwrap();
        let logged_errors = environment.take_logged_errors();
        assert_eq!(logged_errors, ["Extracting archive for owner/name 2.0.0..."]);
        assert_eq!(
            environment.read_file_text(&PathBuf::from("/.bvmrc.json")).unwrap(),
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

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn windows_install_command_installs() {
        let environment = TestEnvironment::new();
        environment.remove_system_path("/data/shims").unwrap();
        run_cli(
            vec!["hidden-shell", "windows-install", "C:\\test\\install\\dir\\bin"],
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(
            environment.get_system_path_dirs(),
            [
                PathBuf::from("/data\\shims"),
                PathBuf::from("C:\\test\\install\\dir\\bin"),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn windows_install_command_existing_paths_installs() {
        let environment = TestEnvironment::new();
        environment.remove_system_path("/data/shims").unwrap();
        environment.ensure_system_path_pre("/data\\shims").unwrap();
        environment.ensure_system_path_pre("/other-dir").unwrap();
        run_cli(
            vec!["hidden-shell", "windows-install", "C:\\test\\install\\dir\\bin"],
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(
            environment.get_system_path_dirs(),
            [
                PathBuf::from("/data\\shims"),
                PathBuf::from("C:\\test\\install\\dir\\bin"),
                PathBuf::from("/other-dir"),
            ]
        );
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn windows_uninstall_command_uninstalls() {
        let environment = TestEnvironment::new();
        environment.remove_system_path("/data/shims").unwrap();
        environment.ensure_system_path_pre("/other-dir").unwrap();
        environment.ensure_system_path_pre("/data\\shims").unwrap();
        environment
            .ensure_system_path_pre("C:\\test\\install\\dir\\bin")
            .unwrap();
        run_cli(
            vec!["hidden-shell", "windows-uninstall", "C:\\test\\install\\dir\\bin"],
            &environment,
        )
        .await
        .unwrap();
        assert_eq!(environment.get_system_path_dirs(), [PathBuf::from("/other-dir")]);
    }

    fn get_shim_path(name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("/data/shims/{}.bat", name)
        } else {
            format!("/data/shims/{}", name)
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

    async fn run_cli(args: Vec<&str>, environment: &TestEnvironment) -> Result<(), ErrBox> {
        let mut args: Vec<String> = args.into_iter().map(String::from).collect();
        args.insert(0, String::from(""));
        run(environment, args).await
    }
}
