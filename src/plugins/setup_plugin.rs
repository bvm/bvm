use std::path::{Path, PathBuf};

use super::*;
use crate::types::ErrBox;
use crate::utils;

pub fn get_plugin_dir(owner: &str, name: &str, version: &str) -> Result<PathBuf, ErrBox> {
    let data_dir = utils::get_user_data_dir()?;
    Ok(data_dir.join("binaries").join(owner).join(name).join(version))
}

pub async fn setup_plugin<'a>(
    plugin_manifest: &'a mut PluginsManifest,
    checksum_url: &utils::ChecksumUrl,
    bin_dir: &Path,
) -> Result<&'a BinaryManifestItem, ErrBox> {
    // download the plugin file
    let plugin_file_bytes = utils::download_file(&checksum_url.url).await?;

    if let Some(checksum) = &checksum_url.checksum {
        utils::verify_sha256_checksum(&plugin_file_bytes, &checksum)?;
    }

    let plugin_file = read_plugin_file(&plugin_file_bytes)?;
    let identifier = plugin_file.get_identifier();

    println!(
        "Installing {}/{} {}...",
        plugin_file.owner, plugin_file.name, plugin_file.version
    );

    // ensure the version can parse to a semver
    if let Err(err) = semver::Version::parse(&plugin_file.version) {
        return err!(
            "The version found in the binary manifest file was invalid. {}",
            err.to_string()
        );
    }

    // associate the url to the identifier
    plugin_manifest.set_identifier_for_url(checksum_url.url.clone(), identifier.clone());

    // if the identifier is already in the manifest, then return that
    if plugin_manifest.get_binary(&identifier).is_some() {
        // the is_some() and unwrap() is done because the borrow checker couldn't figure out doing if let Some(item)...
        return Ok(plugin_manifest.get_binary(&identifier).unwrap());
    }

    // download the zip bytes
    let zip_file_bytes = utils::download_file(plugin_file.get_zip_file()?).await?;
    utils::verify_sha256_checksum(&zip_file_bytes, plugin_file.get_zip_checksum()?)?;
    // create folder
    let plugin_cache_dir_path = get_plugin_dir(&plugin_file.owner, &plugin_file.name, &plugin_file.version)?;
    let _ignore = std::fs::remove_dir_all(&plugin_cache_dir_path);
    std::fs::create_dir_all(&plugin_cache_dir_path)?;
    utils::extract_zip(&zip_file_bytes, &plugin_cache_dir_path)?;

    // run the post extract script
    if let Some(post_extract_script) = plugin_file.get_post_extract_script()? {
        utils::run_shell_command(&plugin_cache_dir_path, post_extract_script)?;
    }

    // add the plugin information to the manifestscript
    let file_name = plugin_cache_dir_path
        .join(plugin_file.get_binary_path()?)
        .to_string_lossy()
        .to_string();
    let item = BinaryManifestItem {
        owner: plugin_file.owner.clone(),
        name: plugin_file.name.clone(),
        version: plugin_file.version,
        created_time: utils::get_time_secs(),
        file_name: file_name.clone(),
    };
    let identifier = item.get_identifier();
    let command_name = item.get_command_name();
    plugin_manifest.add_binary(item);

    // create the script that runs on the path
    create_path_script(&bin_dir, &command_name)?;

    Ok(plugin_manifest.get_binary(&identifier).unwrap())
}
