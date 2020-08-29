use std::path::{Path, PathBuf};

use super::*;
use crate::environment::Environment;
use crate::types::ErrBox;
use crate::utils;

pub fn get_plugin_dir(
    enviroment: &impl Environment,
    owner: &str,
    name: &str,
    version: &str,
) -> Result<PathBuf, ErrBox> {
    let data_dir = enviroment.get_user_data_dir()?;
    Ok(data_dir.join("binaries").join(owner).join(name).join(version))
}

pub async fn setup_plugin<'a, TEnvironment: Environment>(
    environment: &TEnvironment,
    plugin_manifest: &'a mut PluginsManifest,
    checksum_url: &utils::ChecksumUrl,
    bin_dir: &Path,
) -> Result<&'a BinaryManifestItem, ErrBox> {
    let plugin_file = get_and_associate_plugin_file(environment, plugin_manifest, checksum_url).await?;

    environment.log_error(&format!(
        "Installing {}/{} {}...",
        plugin_file.owner, plugin_file.name, plugin_file.version
    ));

    // download the url's bytes
    let url = plugin_file.get_url()?;
    let download_type = plugin_file.get_download_type()?;
    let url_file_bytes = environment.download_file(url).await?;
    utils::verify_sha256_checksum(&url_file_bytes, plugin_file.get_url_checksum()?)?;

    // create folder
    let plugin_cache_dir_path =
        get_plugin_dir(environment, &plugin_file.owner, &plugin_file.name, &plugin_file.version)?;
    let _ignore = environment.remove_dir_all(&plugin_cache_dir_path);
    environment.create_dir_all(&plugin_cache_dir_path)?;

    // handle the setup based on the download type
    let binary_path = plugin_cache_dir_path.join(plugin_file.get_binary_path()?);
    match download_type {
        DownloadType::Zip => utils::extract_zip(environment, &url_file_bytes, &plugin_cache_dir_path)?,
        DownloadType::Binary => environment.write_file(&binary_path, &url_file_bytes)?,
    }

    // run the post install script
    if let Some(post_install_script) = plugin_file.get_post_install_script()? {
        environment.run_shell_command(&plugin_cache_dir_path, post_install_script)?;
    }

    // add the plugin information to the manifestscript
    let file_name = binary_path.to_string_lossy().to_string();
    let item = BinaryManifestItem {
        owner: plugin_file.owner.clone(),
        name: plugin_file.name.clone(),
        version: plugin_file.version.clone(),
        created_time: environment.get_time_secs(),
        file_name: file_name.clone(),
    };
    let identifier = item.get_identifier();
    let command_name = item.get_command_name();
    plugin_manifest.add_binary(item);

    // create the script that runs on the path
    create_path_script(environment, &bin_dir, &command_name)?;

    Ok(plugin_manifest.get_binary(&identifier).unwrap())
}

async fn get_and_associate_plugin_file(
    environment: &impl Environment,
    plugin_manifest: &mut PluginsManifest,
    checksum_url: &utils::ChecksumUrl,
) -> Result<PluginFile, ErrBox> {
    let plugin_file_bytes = environment.download_file(&checksum_url.url).await?;

    if let Some(checksum) = &checksum_url.checksum {
        utils::verify_sha256_checksum(&plugin_file_bytes, &checksum)?;
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
    plugin_manifest.set_identifier_for_url(checksum_url.url.clone(), identifier);

    Ok(plugin_file)
}
