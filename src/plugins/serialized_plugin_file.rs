use serde::{self, Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::Version;
use crate::CommandName;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SerializedPluginFile {
    pub schema_version: u32,
    pub name: String,
    pub owner: String,
    pub version: Version,
    pub description: String,
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
    pub on_pre_install: Option<String>,
    pub on_post_install: Option<String>,
    pub on_use: Option<String>,
    pub on_stop_use: Option<String>,
    pub environment: Option<BinaryEnvironment>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BinaryEnvironment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfoCommand {
    pub name: CommandName,
    pub path: String,
}
