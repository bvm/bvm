use dprint_cli_core::checksums::{verify_sha256_checksum, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;
use std::path::{Path, PathBuf};

use super::*;
use crate::environment::Environment;
use crate::types::BinaryName;
use crate::utils;

pub fn get_plugin_dir(
    environment: &impl Environment,
    binary_name: &BinaryName,
    version: &str,
) -> Result<PathBuf, ErrBox> {
    let data_dir = environment.get_bvm_home_dir()?;
    Ok(data_dir
        .join("binaries")
        .join(&binary_name.owner)
        .join(binary_name.name.as_str())
        .join(version))
}

pub async fn get_and_associate_plugin_file<'a, TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &'a mut PluginsManifest,
    checksum_url: &ChecksumPathOrUrl,
) -> Result<PluginFile, ErrBox> {
    let plugin_file_bytes = environment.download_file(&checksum_url.path_or_url).await?;

    if let Some(checksum) = &checksum_url.checksum {
        verify_sha256_checksum(&plugin_file_bytes, &checksum)?;
    }

    let plugin_file = read_plugin_file(&plugin_file_bytes)?;

    // ensure the plugin version can parse to a semver
    if let Err(err) = semver::Version::parse(&plugin_file.version) {
        return err!(
            "The version found in the binary manifest file was invalid. {}",
            err.to_string()
        );
    }

    // associate the url to the binary identifier
    let identifier = plugin_file.get_identifier();
    plugin_manifest.set_identifier_for_url(&checksum_url, identifier);

    Ok(plugin_file)
}

pub async fn setup_plugin<'a, TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &'a mut PluginsManifest,
    plugin_file: &PluginFile,
    bin_dir: &Path,
) -> Result<&'a BinaryManifestItem, ErrBox> {
    // download the url's bytes
    let url = plugin_file.get_url()?;
    let download_type = plugin_file.get_download_type()?;
    let url_file_bytes = environment.download_file(url).await?;
    verify_sha256_checksum(&url_file_bytes, plugin_file.get_url_checksum()?)?;

    // create folder
    let plugin_cache_dir_path = get_plugin_dir(environment, &plugin_file.get_binary_name(), &plugin_file.version)?;
    let _ignore = environment.remove_dir_all(&plugin_cache_dir_path);
    environment.create_dir_all(&plugin_cache_dir_path)?;

    // run the pre install script
    if let Some(pre_install_script) = plugin_file.get_pre_install_script()? {
        environment.run_shell_command(&plugin_cache_dir_path, pre_install_script)?;
    }

    // handle the setup based on the download type
    let commands = plugin_file.get_commands()?;
    verify_commands(commands)?;
    match download_type {
        DownloadType::Zip => {
            utils::extract_zip(
                &format!(
                    "Extracting archive for {}/{} {}...",
                    plugin_file.owner, plugin_file.name, plugin_file.version
                ),
                environment,
                &url_file_bytes,
                &plugin_cache_dir_path,
            )
            .await?
        }
        DownloadType::TarGz => {
            utils::extract_tar_gz(
                &format!(
                    "Extracting archive for {}/{} {}...",
                    plugin_file.owner, plugin_file.name, plugin_file.version
                ),
                environment,
                &url_file_bytes,
                &plugin_cache_dir_path,
            )
            .await?
        }
        DownloadType::Binary => {
            if commands.len() != 1 {
                return err!("The binary download type must have exactly one command specified.");
            }
            environment.write_file(&plugin_cache_dir_path.join(&commands[0].path), &url_file_bytes)?
        }
    }

    // run the post install script
    if let Some(post_install_script) = plugin_file.get_post_install_script()? {
        environment.run_shell_command(&plugin_cache_dir_path, post_install_script)?;
    }

    // create the shims
    for command in commands {
        create_shim(environment, &bin_dir, &command.get_command_name())?;
    }

    // add the plugin information to the manifest
    let item = BinaryManifestItem {
        name: plugin_file.get_binary_name(),
        version: plugin_file.version.clone(),
        created_time: environment.get_time_secs(),
        commands: commands
            .iter()
            .map(|c| BinaryManifestItemCommand {
                name: c.name.clone(),
                path: c.path.clone(),
            })
            .collect(),
    };
    let identifier = item.get_identifier();
    plugin_manifest.add_binary(item);

    Ok(plugin_manifest.get_binary(&identifier).unwrap())
}

fn verify_commands(commands: &Vec<PlatformInfoCommand>) -> Result<(), ErrBox> {
    if commands.is_empty() {
        return err!("One command must be specified.");
    }

    // prevent funny business
    for command in commands.iter() {
        if command.path.contains("../") || command.path.contains("..\\") {
            return err!("A command path cannot go down directories.");
        }
        if PathBuf::from(&command.path).is_absolute() {
            return err!("A command path cannot be absolute.");
        }
    }

    Ok(())
}
