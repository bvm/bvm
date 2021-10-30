use dprint_cli_core::checksums::get_sha256_checksum;
use dprint_cli_core::checksums::verify_sha256_checksum;
use dprint_cli_core::types::ErrBox;

use super::read_plugin_file;
use super::PluginFile;
use crate::environment::Environment;
use crate::utils::ChecksumUrl;

pub fn get_plugin_file<TEnvironment: Environment>(
    environment: &TEnvironment,
    checksum_url: &ChecksumUrl,
) -> Result<PluginFile, ErrBox> {
    let plugin_file_bytes = environment.fetch_url(&checksum_url.url)?;
    let checksum = if let Some(checksum) = &checksum_url.checksum {
        verify_sha256_checksum(&plugin_file_bytes, &checksum)?;
        checksum.clone()
    } else {
        get_sha256_checksum(&plugin_file_bytes)
    };

    let serialized_plugin_file = read_plugin_file(&plugin_file_bytes)?;
    Ok(PluginFile {
        url: checksum_url.url.clone(),
        checksum,
        file: serialized_plugin_file,
    })
}
