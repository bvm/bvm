use crate::types::ErrBox;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Values;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginsManifest {
    // Key is url.
    urls_to_identifier: HashMap<String, BinaryIdentifier>,
    /// Key is binary name.
    global_versions: HashMap<String, BinaryIdentifier>,
    binaries: HashMap<BinaryIdentifier, BinaryManifestItem>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BinaryManifestItem {
    pub group: String,
    pub name: String,
    pub version: String,
    pub file_name: String,
    /// Created time in *seconds* since epoch.
    pub created_time: u64,
}

impl BinaryManifestItem {
    pub fn get_identifier(&self) -> BinaryIdentifier {
        BinaryIdentifier::new(&self.group, &self.name, &self.version)
    }

    pub fn get_sem_ver(&self) -> Version {
        // at this point, expect this to be ok since we validated it on setup
        Version::parse(&self.version).unwrap()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct BinaryIdentifier(String);

impl BinaryIdentifier {
    pub fn new(group: &str, name: &str, version: &str) -> Self {
        BinaryIdentifier(format!("{}||{}||{}", group, name, version))
    }
}

impl PluginsManifest {
    pub(super) fn new() -> PluginsManifest {
        PluginsManifest {
            global_versions: HashMap::new(),
            binaries: HashMap::new(),
            urls_to_identifier: HashMap::new(),
        }
    }

    // url to identifier

    pub fn get_identifier_from_url(&self, url: &str) -> Option<&BinaryIdentifier> {
        self.urls_to_identifier.get(url)
    }

    pub fn set_identifier_for_url(&mut self, url: String, identifier: BinaryIdentifier) {
        self.urls_to_identifier.insert(url, identifier);
    }

    pub fn remove_url(&mut self, url: &str) {
        self.urls_to_identifier.remove(url);
    }

    // binary

    pub fn add_binary(&mut self, item: BinaryManifestItem) {
        let identifier = item.get_identifier();
        // add to the global versions if nothing is in there
        if !self.global_versions.contains_key(&item.name) {
            self.global_versions
                .insert(item.name.clone(), identifier.clone());
        }
        self.binaries.insert(identifier, item);
    }

    pub fn get_binary(&self, identifier: &BinaryIdentifier) -> Option<&BinaryManifestItem> {
        self.binaries.get(identifier)
    }

    pub fn remove_binary(&mut self, identifier: &BinaryIdentifier) {
        let name = if let Some(item) = self.binaries.get(identifier) {
            Some(item.name.clone())
        } else {
            None
        };

        self.binaries.remove(identifier);

        if let Some(name) = name {
            self.remove_if_global_binary(&name, identifier);
        }
    }

    pub fn get_binary_by_name_and_version(
        &self,
        name: &str,
        version: &str,
    ) -> Option<&BinaryManifestItem> {
        for binary in self.binaries() {
            if binary.name == name && binary.version == version {
                return Some(binary);
            }
        }

        None
    }

    pub fn binaries(&self) -> Values<'_, BinaryIdentifier, BinaryManifestItem> {
        self.binaries.values()
    }

    pub fn get_binaries_with_name(&self, name: &str) -> Vec<&BinaryManifestItem> {
        self.binaries().filter(|b| b.name == name).collect()
    }

    pub fn get_latest_binary_with_name(&self, name: &str) -> Option<&BinaryManifestItem> {
        let mut binaries = self.get_binaries_with_name(name);
        binaries.sort_by(|a, b| a.get_sem_ver().partial_cmp(&b.get_sem_ver()).unwrap());
        binaries.pop()
    }

    pub fn get_global_binary(&self, binary_name: &str) -> Option<&BinaryManifestItem> {
        match self.global_versions.get(binary_name) {
            Some(key) => self.binaries.get(key),
            None => None,
        }
    }

    pub fn use_global_version(&mut self, binary_name: String, identifier: BinaryIdentifier) {
        self.global_versions.insert(binary_name, identifier);
    }

    fn remove_if_global_binary(
        &mut self,
        removed_binary_name: &str,
        removed_binary_identifier: &BinaryIdentifier,
    ) {
        if let Some(global_binary_identifier) = self.global_versions.get(removed_binary_name) {
            if global_binary_identifier == removed_binary_identifier {
                // set the latest binary as the global binary
                if let Some(latest_binary) = self.get_latest_binary_with_name(&removed_binary_name)
                {
                    let latest_identifier = latest_binary.get_identifier();
                    self.use_global_version(removed_binary_name.to_string(), latest_identifier);
                } else {
                    self.remove_global_binary(&removed_binary_name);
                }
            }
        }
    }

    fn remove_global_binary(&mut self, binary_name: &str) {
        self.global_versions.remove(binary_name);
    }

    pub fn is_global_version(&mut self, identifier: &BinaryIdentifier) -> bool {
        if let Some(item) = self.binaries.get(identifier) {
            if let Some(global_version_identifier) = self.global_versions.get(&item.name) {
                global_version_identifier == identifier
            } else {
                false
            }
        } else {
            false
        }
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
