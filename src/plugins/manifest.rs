use core::cmp::Ordering;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Values;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::types::{BinaryName, CommandName, ErrBox};

const PATH_GLOBAL_VERSION_VALUE: &'static str = "path";
const IDENTIFIER_GLOBAL_PREFIX: &'static str = "identifier:";

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginsManifest {
    // Key is url.
    urls_to_identifier: HashMap<String, BinaryIdentifier>,
    global_versions: GlobalVersionsMap,
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

    pub fn has_name(&self, name: &BinaryName) -> bool {
        name.is_match(&self.group, &self.name)
    }

    pub fn get_command_name(&self) -> CommandName {
        CommandName::from_string(self.name.clone())
    }

    pub fn get_binary_name(&self) -> BinaryName {
        BinaryName::new(Some(self.group.clone()), self.name.clone())
    }

    pub fn compare(&self, other: &BinaryManifestItem) -> Ordering {
        self.get_sem_ver().partial_cmp(&other.get_sem_ver()).unwrap()
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

pub enum GlobalBinaryLocation {
    /// Use a bvm binary.
    Bvm(BinaryIdentifier),
    /// Use the binary on the path.
    Path,
}

impl From<BinaryIdentifier> for GlobalBinaryLocation {
    fn from(identifier: BinaryIdentifier) -> Self {
        GlobalBinaryLocation::Bvm(identifier)
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
struct GlobalVersionsMap(HashMap<String, String>);

impl GlobalVersionsMap {
    fn replace(&mut self, command_name: CommandName, location: GlobalBinaryLocation) {
        self.0.insert(
            command_name.into_string(),
            match location {
                GlobalBinaryLocation::Path => PATH_GLOBAL_VERSION_VALUE.to_string(),
                GlobalBinaryLocation::Bvm(identifier) => format!("{}{}", IDENTIFIER_GLOBAL_PREFIX, identifier.0),
            },
        );
    }

    fn get(&self, command_name: &CommandName) -> Option<GlobalBinaryLocation> {
        self.0.get(command_name.as_str()).map(|key| {
            if key == PATH_GLOBAL_VERSION_VALUE {
                GlobalBinaryLocation::Path
            } else if key.starts_with(IDENTIFIER_GLOBAL_PREFIX) {
                GlobalBinaryLocation::Bvm(BinaryIdentifier(key[IDENTIFIER_GLOBAL_PREFIX.len()..].to_string()))
            } else {
                // todo: don't panic and improve this
                panic!("Unknown key: {}", key);
            }
        })
    }

    fn remove(&mut self, command_name: &CommandName) {
        self.0.remove(command_name.as_str());
    }
}

impl PluginsManifest {
    pub(super) fn new() -> PluginsManifest {
        PluginsManifest {
            global_versions: GlobalVersionsMap(HashMap::new()),
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
        self.binaries.insert(item.get_identifier(), item);
    }

    pub fn get_binary(&self, identifier: &BinaryIdentifier) -> Option<&BinaryManifestItem> {
        self.binaries.get(identifier)
    }

    pub fn remove_binary(&mut self, identifier: &BinaryIdentifier) {
        let binary_name = if let Some(item) = self.binaries.get(identifier) {
            Some(item.get_binary_name())
        } else {
            None
        };

        self.binaries.remove(identifier);

        if let Some(binary_name) = binary_name {
            // update the selected global binary
            let command_name = binary_name.get_command_name();
            if !self.has_binary_with_command(&command_name) {
                self.remove_global_binary(&command_name); // could be removing the path entry
            } else {
                self.remove_if_global_binary(&binary_name, identifier);
            }
        }
    }

    pub fn get_binaries_by_name_and_version(&self, name: &BinaryName, version: &str) -> Vec<&BinaryManifestItem> {
        self.binaries()
            .filter(|b| b.has_name(name) && b.version == version)
            .collect()
    }

    pub fn binaries(&self) -> Values<'_, BinaryIdentifier, BinaryManifestItem> {
        self.binaries.values()
    }

    pub fn has_binary_with_name(&self, name: &BinaryName) -> bool {
        self.binaries().any(|b| b.has_name(name))
    }

    pub fn has_binary_with_command(&self, name: &CommandName) -> bool {
        self.binaries().any(|b| b.name == name.as_str())
    }

    pub fn get_binaries_with_name(&self, name: &BinaryName) -> Vec<&BinaryManifestItem> {
        self.binaries().filter(|b| b.has_name(name)).collect()
    }

    pub fn get_binaries_with_command(&self, name: &CommandName) -> Vec<&BinaryManifestItem> {
        self.binaries().filter(|b| b.name == name.as_str()).collect()
    }

    pub fn get_latest_binary_with_name(&self, name: &BinaryName) -> Option<&BinaryManifestItem> {
        let mut binaries = self.get_binaries_with_name(name);
        binaries.sort_by(|a, b| a.compare(b));
        binaries.pop()
    }

    pub fn get_latest_binary_with_command(&self, name: &CommandName) -> Option<&BinaryManifestItem> {
        let mut binaries = self.get_binaries_with_command(name);
        binaries.sort_by(|a, b| a.compare(b));
        binaries.pop()
    }

    pub fn get_global_binary_location(&self, command_name: &CommandName) -> Option<GlobalBinaryLocation> {
        self.global_versions.get(command_name)
    }

    pub fn use_global_version(&mut self, command_name: CommandName, location: GlobalBinaryLocation) {
        self.global_versions.replace(command_name, location)
    }

    fn remove_if_global_binary(
        &mut self,
        removed_binary_name: &BinaryName,
        removed_binary_identifier: &BinaryIdentifier,
    ) {
        let command_name = removed_binary_name.get_command_name();
        if let Some(GlobalBinaryLocation::Bvm(current_identifier)) = self.global_versions.get(&command_name) {
            if &current_identifier == removed_binary_identifier {
                // set the latest binary as the global binary
                let latest_binary = self
                    .get_latest_binary_with_name(&removed_binary_name)
                    .or_else(|| self.get_latest_binary_with_command(&removed_binary_name.get_command_name()));
                if let Some(latest_binary) = latest_binary {
                    let latest_identifier = latest_binary.get_identifier();
                    self.use_global_version(command_name, latest_identifier.into());
                } else {
                    self.remove_global_binary(&command_name);
                }
            }
        }
    }

    fn remove_global_binary(&mut self, command_name: &CommandName) {
        self.global_versions.remove(command_name);
    }

    pub fn is_global_version(&mut self, identifier: &BinaryIdentifier) -> bool {
        if let Some(item) = self.binaries.get(identifier) {
            if let Some(GlobalBinaryLocation::Bvm(global_version_identifier)) =
                self.global_versions.get(&item.get_command_name())
            {
                &global_version_identifier == identifier
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
