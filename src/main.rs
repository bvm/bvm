use std::process::{Command, Stdio};

#[macro_use]
mod types;
mod arg_parser;
mod configuration;
mod plugins;
mod utils;

use types::ErrBox;
use arg_parser::*;

#[tokio::main]
async fn main() -> Result<(), ErrBox> {
    match run().await {
        Ok(_) => {},
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
            let mut plugin_manifest = plugins::read_manifest()?;
            let executable_path = if let Some(config_file_path) = config_file_path {
                let config_file_text = std::fs::read_to_string(&config_file_path)?;
                let config_file = configuration::read_config_file(&config_file_text)?;
                if let Some(config_file_binary) = config_file.binaries.get(&run_command.binary_name) {
                    if let Some(cache_item) = plugin_manifest.get_binary(&config_file_binary.url) {
                        Some(cache_item.file_name.clone())
                    } else {
                        let result = setup_plugin(&mut plugin_manifest, &config_file_binary.url).await?;
                        plugins::write_manifest(&plugin_manifest)?;
                        Some(result)
                    }
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
                }
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
        },
        SubCommand::Install => {
            let config_file_path = match configuration::find_config_file()? {
                Some(file_path) => file_path,
                None => return err!("Could not find .gvmrc.json in the current directory or its ancestors."),
            };
            let config_file_text = std::fs::read_to_string(&config_file_path)?;
            let config_file = configuration::read_config_file(&config_file_text)?;
            let cache_dir = utils::get_user_data_dir()?;
            let binaries_cache_dir = cache_dir.join("bin");
            std::fs::create_dir_all(&binaries_cache_dir)?;
            let mut plugin_manifest = plugins::read_manifest()?;

            for (key, value) in config_file.binaries.iter() {
                if plugin_manifest.get_binary(&value.url).is_none() {
                    setup_plugin(&mut plugin_manifest, &value.url).await?;
                    plugins::create_path_script(key, &binaries_cache_dir)?;
                    plugins::write_manifest(&plugin_manifest)?; // write for every setup plugin in case a further one fails
                }
            }
        },
        SubCommand::Use(use_command) => {
            let mut plugin_manifest = plugins::read_manifest()?;
            let binary = match plugin_manifest.get_binary_by_name_and_version(&use_command.binary_name, &use_command.version) {
                Some(binary) => binary,
                None => return err!("Could not find binary '{}' with version '{}'", use_command.binary_name, use_command.version),
            };
            let binary_name = binary.binary_name.clone(); // clone to prevent mutating while borrowing
            let binary_url = binary.url.clone();
            plugin_manifest.use_global_version(&binary_name, &binary_url);

            plugins::write_manifest(&plugin_manifest)?;
        },
    }

    Ok(())
}

async fn setup_plugin(plugin_manifest: &mut plugins::PluginsManifest, url: &str) -> Result<String, ErrBox> {
    // download the url
    let plugin_file_bytes = utils::download_file(&url).await?;
    let plugin_file = plugins::read_plugin_file(&plugin_file_bytes)?;
    let zip_file_bytes = utils::download_file(plugin_file.get_zip_file()?).await?;
    // create folder
    let cache_dir = utils::get_user_data_dir()?;
    let plugin_cache_dir_path = cache_dir.join("plugins").join(&plugin_file.name).join(&plugin_file.version);
    let _ignore = std::fs::remove_dir_all(&plugin_cache_dir_path);
    std::fs::create_dir_all(&plugin_cache_dir_path)?;
    utils::extract_zip(&zip_file_bytes, &plugin_cache_dir_path)?;

    let file_name = plugin_cache_dir_path.join(plugin_file.get_binary_path()?).to_string_lossy().to_string();
    plugin_manifest.add_binary(url.to_string(), plugins::BinaryManifestItem {
        url: url.to_string(),
        binary_name: plugin_file.name,
        version: plugin_file.version,
        created_time: utils::get_time_secs(),
        file_name: file_name.clone(),
    });

    Ok(file_name)
}
