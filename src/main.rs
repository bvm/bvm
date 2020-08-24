use std::process::{Command, Stdio};

#[macro_use]
mod types;
mod cache;
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
            let executable_path = if let Some(config_file_path) = config_file_path {
                let config_file_text = std::fs::read_to_string(&config_file_path)?;
                let config_file = configuration::read_config_file(&config_file_text)?;
                if let Some(config_file_binary) = config_file.dependencies.get(&run_command.binary_name) {
                    let mut cache_manifest = cache::read_manifest()?;
                    if let Some(cache_item) = cache_manifest.get_item(&config_file_binary.url) {
                        cache_item.file_name.clone()
                    } else {
                        let result = setup_plugin(&mut cache_manifest, &config_file_binary.url).await?;
                        cache::write_manifest(&cache_manifest)?;
                        result
                    }
                } else {
                    run_command.binary_name
                }
            } else {
                run_command.binary_name
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
            let cache_dir = utils::get_cache_dir()?;
            let binaries_cache_dir = cache_dir.join("binaries");
            std::fs::create_dir_all(&binaries_cache_dir)?;
            let mut cache_manifest = cache::read_manifest()?;

            for (key, value) in config_file.dependencies.iter() {
                if cache_manifest.get_item(&value.url).is_none() {
                    setup_plugin(&mut cache_manifest, &value.url).await?;
                    plugins::create_path_script(key, &binaries_cache_dir)?;
                    cache::write_manifest(&cache_manifest)?; // write for every setup plugin in case a further one fails
                }
            }
        }
    }

    Ok(())
}

async fn setup_plugin(cache_manifest: &mut cache::CacheManifest, url: &str) -> Result<String, ErrBox> {
    // download the url
    let plugin_file_bytes = utils::download_file(&url).await?;
    let plugin_file = plugins::read_plugin_file(&plugin_file_bytes)?;
    let zip_file_bytes = utils::download_file(plugin_file.get_zip_file()?).await?;
    // create folder
    let cache_dir = utils::get_cache_dir()?;
    let plugin_cache_dir_path = cache_dir.join("plugins").join(&plugin_file.name).join(&plugin_file.version);
    std::fs::create_dir_all(&plugin_cache_dir_path)?;
    utils::extract_zip(&zip_file_bytes, &plugin_cache_dir_path)?;

    let file_name = plugin_cache_dir_path.join(plugin_file.get_binary_path()?).to_string_lossy().to_string();
    cache_manifest.add_item(url.to_string(), cache::CacheItem {
        created_time: utils::get_time_secs(),
        file_name: file_name.clone(),
    });

    Ok(file_name)
}
