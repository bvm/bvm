use core::cmp::Ordering;
use dprint_cli_core::checksums::ChecksumPathOrUrl;
use dprint_cli_core::types::ErrBox;
use semver::Version as SemVersion;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::Values;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::{BinaryName, BinarySelector, CommandName, Version};

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
pub struct BinaryManifestItemSource {
    pub path: String,
    pub checksum: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BinaryManifestItem {
    pub name: BinaryName,
    pub version: String,
    /// Created time in *seconds* since epoch.
    pub created_time: u64,
    pub commands: Vec<BinaryManifestItemCommand>,
    // Source for reinstalling.
    pub source: BinaryManifestItemSource,
}

impl BinaryManifestItem {
    pub fn get_identifier(&self) -> BinaryIdentifier {
        BinaryIdentifier::new(&self.name, &self.version)
    }

    pub fn get_sem_ver(&self) -> SemVersion {
        // at this point, expect this to be ok since we validated it on setup
        SemVersion::parse(&self.version).unwrap()
    }

    pub fn matches(&self, selector: &BinarySelector) -> bool {
        selector.is_match(&self.name)
    }

    pub fn get_command_names(&self) -> Vec<CommandName> {
        self.commands.iter().map(|c| c.get_command_name()).collect()
    }

    pub fn compare(&self, other: &BinaryManifestItem) -> Ordering {
        let name_ordering = self.name.compare(&other.name);
        match name_ordering {
            Ordering::Equal => self.get_sem_ver().partial_cmp(&other.get_sem_ver()).unwrap(),
            _ => name_ordering,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BinaryManifestItemCommand {
    pub name: String,
    /// The relative path to the file name.
    pub path: String,
}

impl BinaryManifestItemCommand {
    pub fn get_command_name(&self) -> CommandName {
        CommandName::from_string(self.name.clone())
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct BinaryIdentifier(String);

impl BinaryIdentifier {
    pub fn new(name: &BinaryName, version: &str) -> Self {
        BinaryIdentifier(format!("{}||{}||{}", name.owner, name.name.as_str(), version))
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
    fn new() -> PluginsManifest {
        PluginsManifest {
            global_versions: GlobalVersionsMap(HashMap::new()),
            binaries: HashMap::new(),
            urls_to_identifier: HashMap::new(),
        }
    }

    pub fn load(environment: &impl Environment) -> Result<PluginsManifest, ErrBox> {
        let file_path = get_manifest_file_path(environment)?;
        match environment.read_file_text(&file_path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(manifest) => Ok(manifest),
                Err(err) => {
                    environment.log_error(&format!("Error deserializing plugins manifest: {}", err));
                    Ok(PluginsManifest::new())
                }
            },
            Err(_) => Ok(PluginsManifest::new()),
        }
    }

    pub fn save(&self, environment: &impl Environment) -> Result<(), ErrBox> {
        let file_path = get_manifest_file_path(environment)?;
        let serialized_manifest = serde_json::to_string(&self)?;
        environment.write_file_text(&file_path, &serialized_manifest)?;
        Ok(())
    }

    // url to identifier

    pub fn get_identifier_from_url(&self, url: &ChecksumPathOrUrl) -> Option<&BinaryIdentifier> {
        self.urls_to_identifier.get(&url.path_or_url)
    }

    pub fn set_identifier_for_url(&mut self, url: &ChecksumPathOrUrl, identifier: BinaryIdentifier) {
        self.urls_to_identifier.insert(url.path_or_url.clone(), identifier);
    }

    pub fn clear_cached_urls(&mut self) {
        self.urls_to_identifier.clear();
    }

    // binary

    pub fn add_binary(&mut self, item: BinaryManifestItem) {
        self.binaries.insert(item.get_identifier(), item);
    }

    pub fn get_binary(&self, identifier: &BinaryIdentifier) -> Option<&BinaryManifestItem> {
        self.binaries.get(identifier)
    }

    pub fn has_binary(&self, identifier: &BinaryIdentifier) -> bool {
        self.get_binary(identifier).is_some()
    }

    pub fn remove_binary(&mut self, identifier: &BinaryIdentifier) {
        let binary_info = if let Some(item) = self.binaries.get(identifier) {
            Some((item.name.clone(), item.get_command_names()))
        } else {
            None
        };

        self.binaries.remove(identifier);

        if let Some((binary_name, command_names)) = binary_info {
            // update the selected global binary
            for command_name in command_names {
                if !self.has_binary_with_command(&command_name) {
                    self.remove_global_binary(&command_name); // could be removing the path entry
                } else {
                    self.remove_if_global_binary(&binary_name, &command_name, identifier);
                }
            }
        }
    }

    pub fn get_binaries_by_selector_and_version(
        &self,
        selector: &BinarySelector,
        version: &Version,
    ) -> Vec<&BinaryManifestItem> {
        self.binaries()
            .filter(|b| b.matches(selector) && b.version == version.as_str())
            .collect()
    }

    pub fn binaries(&self) -> Values<'_, BinaryIdentifier, BinaryManifestItem> {
        self.binaries.values()
    }

    pub fn has_binary_with_selector(&self, selector: &BinarySelector) -> bool {
        self.binaries().any(|b| b.matches(selector))
    }

    pub fn has_binary_with_command(&self, name: &CommandName) -> bool {
        self.binaries().any(|b| &b.name.name == name)
    }

    pub fn command_name_has_same_owner(&self, command_name: &CommandName) -> bool {
        let binaries = self
            .binaries()
            .filter(|b| &b.name.name == command_name)
            .collect::<Vec<_>>();
        if let Some(first_binary) = binaries.get(0) {
            let first_owner = &first_binary.name.owner;
            binaries.iter().all(|b| &b.name.owner == first_owner)
        } else {
            true
        }
    }

    pub fn get_binaries_matching(&self, selector: &BinarySelector) -> Vec<&BinaryManifestItem> {
        self.binaries().filter(|b| b.matches(selector)).collect()
    }

    pub fn get_binaries_with_command(&self, name: &CommandName) -> Vec<&BinaryManifestItem> {
        self.binaries().filter(|b| &b.name.name == name).collect()
    }

    pub fn get_latest_binary_with_name(&self, name: &BinaryName) -> Option<&BinaryManifestItem> {
        let mut binaries = self.binaries().filter(|b| &b.name == name).collect::<Vec<_>>();
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
        removed_command_name: &CommandName,
        removed_binary_identifier: &BinaryIdentifier,
    ) {
        if let Some(GlobalBinaryLocation::Bvm(current_identifier)) = self.global_versions.get(removed_command_name) {
            if &current_identifier == removed_binary_identifier {
                // set the latest binary as the global binary
                let latest_binary = self
                    .get_latest_binary_with_name(&removed_binary_name)
                    .or_else(|| self.get_latest_binary_with_command(removed_command_name));
                if let Some(latest_binary) = latest_binary {
                    let latest_identifier = latest_binary.get_identifier();
                    self.use_global_version(removed_command_name.clone(), latest_identifier.into());
                } else {
                    self.remove_global_binary(removed_command_name);
                }
            }
        }
    }

    fn remove_global_binary(&mut self, command_name: &CommandName) {
        self.global_versions.remove(command_name);
    }

    pub fn is_global_version(&self, identifier: &BinaryIdentifier, command_name: &CommandName) -> bool {
        if let Some(GlobalBinaryLocation::Bvm(global_version_identifier)) = self.global_versions.get(command_name) {
            &global_version_identifier == identifier
        } else {
            false
        }
    }

    pub fn get_global_command_names(&self, identifier: &BinaryIdentifier) -> Vec<CommandName> {
        let mut result = Vec::new();
        if let Some(item) = self.binaries.get(identifier) {
            for command_name in item.get_command_names() {
                if self.is_global_version(identifier, &command_name) {
                    result.push(command_name);
                }
            }
        }
        result
    }
}

fn get_manifest_file_path(environment: &impl Environment) -> Result<PathBuf, ErrBox> {
    let user_data_dir = environment.get_user_data_dir()?; // share across domains
    Ok(user_data_dir.join("binaries-manifest.json"))
}
