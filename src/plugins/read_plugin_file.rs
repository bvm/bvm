use dprint_cli_core::types::ErrBox;
use serde::{self, Deserialize, Serialize};

use crate::types::BinaryName;
use crate::CommandName;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginFile {
    pub schema_version: u32,
    pub name: String,
    pub owner: String,
    pub version: String,
    #[serde(rename = "linux-x86_64")]
    linux: Option<PlatformInfo>,
    #[serde(rename = "darwin-x86_64")]
    mac: Option<PlatformInfo>,
    #[serde(rename = "windows-x86_64")]
    windows: Option<PlatformInfo>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    url: String,
    checksum: String,
    #[serde(rename = "type")]
    download_type: String,
    commands: Vec<PlatformInfoCommand>,
    pre_install: Option<String>,
    post_install: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfoCommand {
    pub name: String,
    pub path: String,
}

impl PlatformInfoCommand {
    pub fn get_command_name(&self) -> CommandName {
        CommandName::from_string(self.name.clone())
    }
}

pub enum DownloadType {
    Zip,
    Binary,
    TarGz,
}

impl PluginFile {
    pub fn get_binary_name(&self) -> BinaryName {
        BinaryName::new(self.owner.clone(), self.name.clone())
    }

    pub fn get_url(&self) -> Result<&String, ErrBox> {
        Ok(&self.get_platform_info()?.url)
    }

    pub fn get_url_checksum(&self) -> Result<&String, ErrBox> {
        Ok(&self.get_platform_info()?.checksum)
    }

    pub fn get_commands(&self) -> Result<&Vec<PlatformInfoCommand>, ErrBox> {
        Ok(&self.get_platform_info()?.commands)
    }

    pub fn get_download_type(&self) -> Result<DownloadType, ErrBox> {
        let download_type = self.get_platform_info()?.download_type.to_lowercase();
        Ok(match download_type.as_str() {
            "zip" => DownloadType::Zip,
            "binary" => DownloadType::Binary,
            "tar.gz" => DownloadType::TarGz,
            _ => return err!("Unknown download type: {}", download_type),
        })
    }

    pub fn get_pre_install_script(&self) -> Result<&Option<String>, ErrBox> {
        Ok(&self.get_platform_info()?.pre_install)
    }

    pub fn get_post_install_script(&self) -> Result<&Option<String>, ErrBox> {
        Ok(&self.get_platform_info()?.post_install)
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
        let binary_name = BinaryName::new(self.owner.clone(), self.name.clone());
        super::BinaryIdentifier::new(&binary_name, &self.version)
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
        Ok(file) => {
            if file.schema_version != 1 {
                return err!(
                    "Expected schema version 1, but found {}. This may indicate you need to upgrade your CLI version to use this binary.",
                    file.schema_version
                );
            }

            if file.name.contains("/") || file.owner.contains("/") {
                return err!("The binary owner and name may not contain a forward slash.");
            }

            Ok(file)
        }
        Err(err) => err!("Error deserializing binary manifest file. {}", err.to_string()),
    }
}
