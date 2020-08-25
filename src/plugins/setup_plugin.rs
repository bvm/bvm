use std::path::Path;

use super::*;
use crate::types::ErrBox;
use crate::utils;

pub async fn setup_plugin(
    plugin_manifest: &mut PluginsManifest,
    url: &str,
    bin_dir: &Path,
) -> Result<String, ErrBox> {
    // download the plugin file
    let plugin_file_bytes = utils::download_file(&url).await?;
    let plugin_file = read_plugin_file(&plugin_file_bytes)?;
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

    // run the post extract script
    if let Some(post_extract_script) = plugin_file.get_post_extract_script()? {
        utils::run_shell_command(&plugin_cache_dir_path, post_extract_script)?;
    }

    // add the plugin information to the script
    let file_name = plugin_cache_dir_path
        .join(plugin_file.get_binary_path()?)
        .to_string_lossy()
        .to_string();
    plugin_manifest.add_binary(BinaryManifestItem {
        group: plugin_file.group.clone(),
        name: plugin_file.name.clone(),
        version: plugin_file.version,
        created_time: utils::get_time_secs(),
        file_name: file_name.clone(),
    });
    create_path_script(&plugin_file.name, &bin_dir)?;

    Ok(file_name)
}
