use dprint_cli_core::types::ErrBox;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::{BinaryName, NameSelector};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
  name_to_urls: HashMap<BinaryName, Vec<String>>,
}

pub struct RegistryItem {
  name: BinaryName,
  url: String,
}

impl RegistryItem {
  pub fn compare(&self, other: &RegistryItem) -> Ordering {
    let ordering = self.name.cmp(&other.name);
    match ordering {
      Ordering::Equal => self.url.partial_cmp(&other.url).unwrap(),
      _ => ordering,
    }
  }

  pub fn display(&self) -> String {
    format!("{} - {}", self.name, self.url)
  }
}

pub struct UrlResult {
  pub owner: String,
  pub url: String,
}

impl Registry {
  fn new() -> Registry {
    Registry {
      name_to_urls: HashMap::new(),
    }
  }

  pub fn load(environment: &impl Environment) -> Registry {
    let file_path = get_registry_file_path(environment);
    match environment.read_file_text(&file_path) {
      Ok(text) => match serde_json::from_str(&text) {
        Ok(manifest) => manifest,
        Err(err) => {
          environment.log_stderr(&format!("Error deserializing registry: {}", err));
          Registry::new()
        }
      },
      Err(_) => Registry::new(),
    }
  }

  pub fn save(&self, environment: &impl Environment) -> Result<(), ErrBox> {
    let file_path = get_registry_file_path(environment);
    let serialized_manifest = serde_json::to_string(&self)?;
    environment.write_file_text(&file_path, &serialized_manifest)?;
    Ok(())
  }

  pub fn get_urls(&self, name_selector: &NameSelector) -> Vec<UrlResult> {
    let mut result = Vec::new();

    for (url_name, urls) in self.name_to_urls.iter() {
      if name_selector.is_match(url_name) {
        for url in urls.iter() {
          result.push(UrlResult {
            owner: url_name.owner.clone(),
            url: url.clone(),
          });
        }
      }
    }

    result
  }

  pub fn add_url(&mut self, name: BinaryName, url: String) {
    let mut items = self.name_to_urls.remove(&name).unwrap_or(Vec::new());
    if !items.contains(&url) {
      items.push(url);
    }
    self.name_to_urls.insert(name, items);
  }

  pub fn remove_url(&mut self, url: &str) {
    let mut keys_to_remove = Vec::new();
    for (name, urls) in self.name_to_urls.iter_mut() {
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
        keys_to_remove.push(name.clone());
      }
    }

    for key in keys_to_remove {
      self.name_to_urls.remove(&key);
    }
  }

  pub fn items(&self) -> Vec<RegistryItem> {
    let mut results = Vec::new();
    for (name, urls) in self.name_to_urls.iter() {
      for url in urls.iter() {
        results.push(RegistryItem {
          name: name.clone(),
          url: url.clone(),
        });
      }
    }
    results
  }
}

fn get_registry_file_path(environment: &impl Environment) -> PathBuf {
  let user_data_dir = environment.get_user_data_dir(); // share across domains
  user_data_dir.join("registry.json")
}
