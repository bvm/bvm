use dprint_cli_core::types::ErrBox;
use serde::{self, Deserialize, Serialize};

use crate::CommandName;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SerializedPluginFile {
    pub schema_version: u32,
    pub name: String,
    pub owner: String,
    pub version: String,
    #[serde(rename = "linux-x86_64")]
    pub linux: Option<PlatformInfo>,
    #[serde(rename = "darwin-x86_64")]
    pub mac: Option<PlatformInfo>,
    #[serde(rename = "windows-x86_64")]
    pub windows: Option<PlatformInfo>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub path: String,
    pub checksum: String,
    #[serde(rename = "type")]
    pub download_type: String,
    pub commands: Vec<PlatformInfoCommand>,
    pub pre_install: Option<String>,
    pub post_install: Option<String>,
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

pub fn read_plugin_file(file_bytes: &[u8]) -> Result<SerializedPluginFile, ErrBox> {
    // todo: don't use serde because this should fail with a nice error message if the schema version is not equal
    match serde_json::from_slice::<SerializedPluginFile>(&file_bytes) {
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
