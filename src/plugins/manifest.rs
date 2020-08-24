use crate::types::ErrBox;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Values;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginsManifest {
    /// Key is binary name. Value is key in the `binaries` map.
    global_versions: HashMap<String, String>,
    binaries: HashMap<String, BinaryManifestItem>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BinaryManifestItem {
    pub url: String,
    pub binary_name: String,
    pub version: String,
    pub file_name: String,
    /// Created time in *seconds* since epoch.
    pub created_time: u64,
}

impl PluginsManifest {
    pub(super) fn new() -> PluginsManifest {
        PluginsManifest {
            global_versions: HashMap::new(),
            binaries: HashMap::new(),
        }
    }

    pub fn add_binary(&mut self, key: String, item: BinaryManifestItem) {
        // add to the global versions if nothing is in there
        if !self.global_versions.contains_key(&item.binary_name) {
            self.global_versions
                .insert(item.binary_name.clone(), key.clone());
        }
        self.binaries.insert(key, item);
    }

    pub fn get_binary(&self, url: &str) -> Option<&BinaryManifestItem> {
        self.binaries.get(url)
    }

    pub fn get_binary_by_name_and_version(
        &self,
        name: &str,
        version: &str,
    ) -> Option<&BinaryManifestItem> {
        for binary in self.binaries() {
            if binary.binary_name == name && binary.version == version {
                return Some(binary);
            }
        }

        None
    }

    pub fn binaries(&self) -> Values<'_, String, BinaryManifestItem> {
        self.binaries.values()
    }

    pub fn get_global_binary(&self, binary_name: &str) -> Option<&BinaryManifestItem> {
        match self.global_versions.get(binary_name) {
            Some(key) => self.binaries.get(key),
            None => None,
        }
    }

    pub fn use_global_version(&mut self, binary_name: &str, url: &str) {
        self.global_versions
            .insert(binary_name.to_string(), url.to_string());
    }
}

pub fn read_manifest() -> Result<PluginsManifest, ErrBox> {
    let file_path = get_manifest_file_path()?;
    match std::fs::read_to_string(&file_path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(manifest) => Ok(manifest),
            Err(err) => {
                eprintln!("Error deserializing cache manifest, but ignoring: {}", err);
                Ok(PluginsManifest::new())
            }
        },
        Err(_) => Ok(PluginsManifest::new()),
    }
}

pub fn write_manifest(manifest: &PluginsManifest) -> Result<(), ErrBox> {
    let file_path = get_manifest_file_path()?;
    let serialized_manifest = serde_json::to_string(&manifest)?;
    std::fs::write(&file_path, &serialized_manifest)?;
    Ok(())
}

fn get_manifest_file_path() -> Result<PathBuf, ErrBox> {
    let user_data_dir = crate::utils::get_user_data_dir()?;
    Ok(user_data_dir.join("plugins-manifest.json"))
}
