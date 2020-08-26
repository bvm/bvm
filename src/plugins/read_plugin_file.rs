use crate::types::ErrBox;
use serde::{self, Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginFile {
    pub schema_version: u32,
    pub name: String,
    pub group: String,
    pub version: String,
    linux: Option<PlatformInfo>,
    mac: Option<PlatformInfo>,
    windows: Option<PlatformInfo>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    archive: String,
    binary_path: String,
    post_extract: Option<String>,
}

impl PluginFile {
    pub fn get_zip_file(&self) -> Result<&String, ErrBox> {
        Ok(&self.get_platform_info()?.archive)
    }

    pub fn get_binary_path(&self) -> Result<&String, ErrBox> {
        Ok(&self.get_platform_info()?.binary_path)
    }

    pub fn get_post_extract_script(&self) -> Result<&Option<String>, ErrBox> {
        Ok(&self.get_platform_info()?.post_extract)
    }

    fn get_platform_info(&self) -> Result<&PlatformInfo, ErrBox> {
        // todo: how to throw a nice compile error here for an unsupported OS?
        #[cfg(target_os = "linux")]
        return get_plugin_platform_info(&self.linux);

        #[cfg(target_os = "macos")]
        return get_plugin_platform_info(&self.mac);

        #[cfg(target_os = "windows")]
        return get_plugin_platform_info(&self.windows);
    }

    pub fn get_identifier(&self) -> super::BinaryIdentifier {
        super::BinaryIdentifier::new(&self.group, &self.name, &self.version)
    }
}

fn get_plugin_platform_info<'a>(platform_info: &'a Option<PlatformInfo>) -> Result<&'a PlatformInfo, ErrBox> {
    if let Some(platform_info) = &platform_info {
        Ok(platform_info)
    } else {
        return err!("Unsupported operating system.");
    }
}

pub fn read_plugin_file(file_bytes: &[u8]) -> Result<PluginFile, ErrBox> {
    // todo: don't use serde because this should fail with a nice error message if the schema version is not equal
    match serde_json::from_slice::<PluginFile>(&file_bytes) {
        Ok(plugin_file) => {
            if plugin_file.schema_version != 1 {
                return err!(
                    "Expected schema version 1, but found {}. This may indicate you need to upgrade your CLI version to use this binary.",
                    plugin_file.schema_version
                );
            }

            Ok(plugin_file)
        }
        Err(err) => err!("Error deserializing binary manifest file. {}", err.to_string()),
    }
}
