use std::collections::hash_map::Values;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;
use crate::types::ErrBox;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CacheManifest(HashMap<String, CacheItem>);

impl CacheManifest {
    pub(super) fn new() -> CacheManifest {
        CacheManifest(HashMap::new())
    }

    pub fn add_item(&mut self, key: String, item: CacheItem) {
        self.0.insert(key, item);
    }

    pub fn get_item(&self, key: &str) -> Option<&CacheItem> {
        self.0.get(key)
    }

    pub fn remove_item(&mut self, key: &str) -> Option<CacheItem> {
        self.0.remove(key)
    }

    pub fn items(&self) -> Values<'_, String, CacheItem> {
        self.0.values()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CacheItem {
    pub file_name: String,
    /// Created time in *seconds* since epoch.
    pub created_time: u64,
}

pub fn read_manifest() -> Result<CacheManifest, ErrBox> {
    let file_path = get_manifest_file_path()?;
    match std::fs::read_to_string(&file_path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(manifest) => Ok(manifest),
            Err(err) => {
                eprintln!("Error deserializing cache manifest, but ignoring: {}", err);
                Ok(CacheManifest::new())
            }
        },
        Err(_) => Ok(CacheManifest::new()),
    }
}

pub fn write_manifest(manifest: &CacheManifest) -> Result<(), ErrBox> {
    let file_path = get_manifest_file_path()?;
    let serialized_manifest = serde_json::to_string(&manifest)?;
    std::fs::write(&file_path, &serialized_manifest)?;
    Ok(())
}

fn get_manifest_file_path() -> Result<PathBuf, ErrBox> {
    let cache_dir = crate::utils::get_cache_dir()?;
    Ok(cache_dir.join("cache-manifest.json"))
}
