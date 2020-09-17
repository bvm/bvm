use dprint_cli_core::checksums::{get_sha256_checksum, verify_sha256_checksum, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;

use super::{read_plugin_file, PluginFile};
use crate::environment::Environment;

pub async fn get_plugin_file<TEnvironment: Environment>(
    environment: &TEnvironment,
    checksum_url: &ChecksumPathOrUrl,
) -> Result<PluginFile, ErrBox> {
    let plugin_file_bytes = environment.download_file(&checksum_url.path_or_url).await?;

    let checksum = if let Some(checksum) = &checksum_url.checksum {
        verify_sha256_checksum(&plugin_file_bytes, &checksum)?;
        checksum.clone()
    } else {
        get_sha256_checksum(&plugin_file_bytes)
    };

    let serialized_plugin_file = read_plugin_file(&plugin_file_bytes)?;
    Ok(PluginFile {
        url: checksum_url.path_or_url.clone(),
        checksum,
        file: serialized_plugin_file,
    })
}
