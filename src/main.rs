#[macro_use]
mod types;
mod arg_parser;
mod configuration;
mod plugins;
mod utils;

use arg_parser::*;
use types::ErrBox;

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
        SubCommand::Use(use_command) => handle_use_command(use_command)?,
    }

    Ok(())
}

fn handle_resolve_command(resolve_command: ResolveCommand) -> Result<(), ErrBox> {
    let config_file_path = configuration::find_config_file()?;
    let plugin_manifest = plugins::read_manifest()?;
    let executable_path = if let Some(config_file_path) = config_file_path {
        // todo: cleanup :)
        let config_file_text = std::fs::read_to_string(&config_file_path)?;
        let config_file = configuration::read_config_file(&config_file_text)?;
        let mut had_uninstalled_binary = false;
        let mut config_file_binary = None;

        for url in config_file.binaries.iter() {
            if let Some(identifier) = plugin_manifest.get_identifier_from_url(url) {
                if let Some(cache_item) = plugin_manifest.get_binary(&identifier) {
                    if cache_item.name == resolve_command.binary_name {
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

        if config_file_binary.is_none() && had_uninstalled_binary {
            eprintln!(
                "[gvm warning]: There were uninstalled binaries (run `gvm install`). Resolving global '{}'.",
                resolve_command.binary_name
            );
        }

        if let Some(config_file_binary) = config_file_binary {
            Some(config_file_binary.file_name.clone())
        } else {
            None
        }
    } else {
        None
    };
    let executable_path = match executable_path {
        Some(path) => path,
        None => match plugin_manifest.get_global_binary(&resolve_command.binary_name) {
            Some(manifest_item) => manifest_item.file_name.clone(),
            None => return err!("Could not find binary '{}'", resolve_command.binary_name),
        },
    };

    println!("{}", executable_path);

    Ok(())
}

async fn handle_install_command() -> Result<(), ErrBox> {
    let config_file_path = match configuration::find_config_file()? {
        Some(file_path) => file_path,
        None => {
            return err!("Could not find .gvmrc.json in the current directory or its ancestors.")
        }
    };
    let config_file_text = std::fs::read_to_string(&config_file_path)?;
    let config_file = configuration::read_config_file(&config_file_text)?;
    let bin_dir = utils::get_bin_dir()?;
    let mut plugin_manifest = plugins::read_manifest()?;

    for url in config_file.binaries.iter() {
        let is_installed = plugin_manifest
            .get_identifier_from_url(&url)
            .map(|identifier| plugin_manifest.get_binary(&identifier).is_some())
            .unwrap_or(false);

        if !is_installed {
            plugins::setup_plugin(&mut plugin_manifest, &url, &bin_dir).await?;
            plugins::write_manifest(&plugin_manifest)?; // write for every setup plugin in case a further one fails
        }
    }

    Ok(())
}

async fn handle_install_url_command(url: String) -> Result<(), ErrBox> {
    let bin_dir = utils::get_bin_dir()?;
    let mut plugin_manifest = plugins::read_manifest()?;

    // todo: require `--force` if already installed

    // remove the existing binary from the cache (the setup_plugin function will delete it from the disk)
    if let Some(identifier) = plugin_manifest
        .get_identifier_from_url(&url)
        .map(|identifier| identifier.clone())
    {
        plugin_manifest.remove_binary(&identifier);
        plugin_manifest.remove_url(&url);
        plugins::write_manifest(&plugin_manifest)?;
    }

    plugins::setup_plugin(&mut plugin_manifest, &url, &bin_dir).await?;
    plugins::write_manifest(&plugin_manifest)
}

fn handle_use_command(use_command: UseCommand) -> Result<(), ErrBox> {
    let mut plugin_manifest = plugins::read_manifest()?;
    // todo: handle multiple binaries with the same name and version
    let binary = match plugin_manifest
        .get_binary_by_name_and_version(&use_command.binary_name, &use_command.version)
    {
        Some(binary) => binary,
        None => {
            let mut versions = plugin_manifest
                .binaries()
                .filter(|b| b.name == use_command.binary_name)
                .map(|b| b.version.to_string())
                .collect::<Vec<_>>();
            versions.sort();
            return if versions.is_empty() {
                err!(
                    "Could not find any installed binaries named '{}'",
                    use_command.binary_name
                )
            } else {
                err!(
                    "Could not find binary '{}' with version '{}'\n\nInstalled versions:\n  {}",
                    use_command.binary_name,
                    use_command.version,
                    versions.join("\n  ")
                )
            };
        }
    };
    let binary_name = binary.name.clone(); // clone to prevent mutating while borrowing
    let identifier = binary.get_identifier();
    plugin_manifest.use_global_version(binary_name, identifier);

    plugins::write_manifest(&plugin_manifest)?;

    return Ok(());
}
