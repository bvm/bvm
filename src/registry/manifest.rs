use dprint_cli_core::types::ErrBox;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::BinaryFullName;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    /// Long name (ex. "denoland/deno") to urls.
    name_to_urls: HashMap<String, Vec<String>>,
}

pub struct RegistryItem {
    name: BinaryFullName,
    url: String,
}

impl RegistryItem {
    pub fn compare(&self, other: &RegistryItem) -> Ordering {
        let ordering = self.name.compare(&other.name);
        match ordering {
            Ordering::Equal => self.url.partial_cmp(&other.url).unwrap(),
            _ => ordering,
        }
    }

    pub fn display(&self) -> String {
        format!("{} - {}", self.name.display(), self.url)
    }
}

impl Registry {
    fn new() -> Registry {
        Registry {
            name_to_urls: HashMap::new(),
        }
    }

    pub fn load(environment: &impl Environment) -> Result<Registry, ErrBox> {
        let file_path = get_registry_file_path(environment)?;
        match environment.read_file_text(&file_path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(manifest) => Ok(manifest),
                Err(err) => {
                    environment.log_error(&format!("Error deserializing registry: {}", err));
                    Ok(Registry::new())
                }
            },
            Err(_) => Ok(Registry::new()),
        }
    }

    pub fn save(&self, environment: &impl Environment) -> Result<(), ErrBox> {
        let file_path = get_registry_file_path(environment)?;
        let serialized_manifest = serde_json::to_string(&self)?;
        environment.write_file_text(&file_path, &serialized_manifest)?;
        Ok(())
    }

    pub fn add_url(&mut self, name: &BinaryFullName, url: String) {
        let key = get_full_name_key(name);
        let mut items = self.name_to_urls.remove(&key).unwrap_or(Vec::new());
        if !items.contains(&url) {
            items.push(url);
        }
        self.name_to_urls.insert(key, items);
    }

    pub fn remove_url(&mut self, url: &str) {
        let mut keys_to_remove = Vec::new();
        for (key, urls) in self.name_to_urls.iter_mut() {
            let mut indexes_to_remove = Vec::new();
            for (i, item) in urls.iter().enumerate() {
                if item == url {
                    indexes_to_remove.push(i);
                }
            }
            indexes_to_remove.reverse();
            for index in indexes_to_remove {
                urls.remove(index);
            }

            if urls.is_empty() {
                keys_to_remove.push(key.clone());
            }
        }

        for key in keys_to_remove {
            self.name_to_urls.remove(&key);
        }
    }

    pub fn items(&self) -> Vec<RegistryItem> {
        let mut results = Vec::new();
        for (key, urls) in self.name_to_urls.iter() {
            for url in urls.iter() {
                results.push(RegistryItem {
                    name: get_full_name_from_key(key),
                    url: url.clone(),
                });
            }
        }
        results
    }
}

fn get_full_name_key(name: &BinaryFullName) -> String {
    format!("{}/{}", name.owner, name.name)
}

fn get_full_name_from_key(key: &str) -> BinaryFullName {
    let items = key.split("/").collect::<Vec<_>>();
    BinaryFullName::new(items[0].to_string(), items[1].to_string())
}

fn get_registry_file_path(environment: &impl Environment) -> Result<PathBuf, ErrBox> {
    let user_data_dir = environment.get_user_data_dir()?;
    Ok(user_data_dir.join("registry.json"))
}
