use std::path::Path;
use std::process::{Command, Stdio};

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
        SubCommand::Run(run_command) => {
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
                            if cache_item.name == run_command.binary_name {
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
                        "[gvm warning]: There were uninstalled binaries (run `gvm install`). Using global '{}'.",
                        run_command.binary_name
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
                None => match plugin_manifest.get_global_binary(&run_command.binary_name) {
                    Some(manifest_item) => manifest_item.file_name.clone(),
                    None => return err!("Could not find binary '{}'", run_command.binary_name),
                },
            };

            let status = Command::new(executable_path)
                .args(&run_command.args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .expect("failed to execute process");

            match status.code() {
                Some(code) => std::process::exit(code),
                None => panic!("Process terminated by signal."), // todo: what to do here?
            }
        }
        SubCommand::Install => {
            let config_file_path = match configuration::find_config_file()? {
                Some(file_path) => file_path,
                None => {
                    return err!(
                        "Could not find .gvmrc.json in the current directory or its ancestors."
                    )
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
                    setup_plugin(&mut plugin_manifest, &url, &bin_dir).await?;
                    plugins::write_manifest(&plugin_manifest)?; // write for every setup plugin in case a further one fails
                }
            }
        }
        SubCommand::InstallUrl(url) => {
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

            setup_plugin(&mut plugin_manifest, &url, &bin_dir).await?;
            plugins::write_manifest(&plugin_manifest)?;
        }
        SubCommand::Use(use_command) => {
            let mut plugin_manifest = plugins::read_manifest()?;
            // todo: handle multiple binaries with the same name and version
            let binary = match plugin_manifest
                .get_binary_by_name_and_version(&use_command.binary_name, &use_command.version)
            {
                Some(binary) => binary,
                None => {
                    return err!(
                        "Could not find binary '{}' with version '{}'",
                        use_command.binary_name,
                        use_command.version
                    )
                }
            };
            let binary_name = binary.name.clone(); // clone to prevent mutating while borrowing
            let identifier = binary.get_identifier();
            plugin_manifest.use_global_version(binary_name, identifier);

            plugins::write_manifest(&plugin_manifest)?;
        }
    }

    Ok(())
}

async fn setup_plugin(
    plugin_manifest: &mut plugins::PluginsManifest,
    url: &str,
    bin_dir: &Path,
) -> Result<String, ErrBox> {
    // download the plugin file
    let plugin_file_bytes = utils::download_file(&url).await?;
    let plugin_file = plugins::read_plugin_file(&plugin_file_bytes)?;
    let identifier = plugin_file.get_identifier();

    // associate the url to the identifier
    plugin_manifest.set_identifier_for_url(url.to_string(), identifier.clone());

    // if the identifier is already in the manifest, then return that
    if let Some(binary) = plugin_manifest.get_binary(&identifier) {
        return Ok(binary.file_name.clone());
    }

    // download the zip bytes
    let zip_file_bytes = utils::download_file(plugin_file.get_zip_file()?).await?;
    // create folder
    let cache_dir = utils::get_user_data_dir()?;
    let plugin_cache_dir_path = cache_dir
        .join("plugins")
        .join(&plugin_file.group)
        .join(&plugin_file.name)
        .join(&plugin_file.version);
    let _ignore = std::fs::remove_dir_all(&plugin_cache_dir_path);
    std::fs::create_dir_all(&plugin_cache_dir_path)?;
    utils::extract_zip(&zip_file_bytes, &plugin_cache_dir_path)?;

    // add the plugin information to the script
    let file_name = plugin_cache_dir_path
        .join(plugin_file.get_binary_path()?)
        .to_string_lossy()
        .to_string();
    plugin_manifest.add_binary(plugins::BinaryManifestItem {
        group: plugin_file.group.clone(),
        name: plugin_file.name.clone(),
        version: plugin_file.version,
        created_time: utils::get_time_secs(),
        file_name: file_name.clone(),
    });
    plugins::create_path_script(&plugin_file.name, &bin_dir)?;

    Ok(file_name)
}
