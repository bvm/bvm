use dprint_cli_core::types::ErrBox;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::environment::Environment;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentManifest {
    // Use a deterministic order (so no HashSet).
    binary_paths: Vec<String>,
}

impl EnvironmentManifest {
    fn new() -> EnvironmentManifest {
        EnvironmentManifest {
            binary_paths: Vec::new(),
        }
    }

    pub fn load(environment: &impl Environment) -> Result<EnvironmentManifest, ErrBox> {
        let file_path = get_environment_manifest_file(environment)?;
        match environment.read_file_text(&file_path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(manifest) => Ok(manifest),
                Err(err) => {
                    environment.log_error(&format!("Error deserializing environment manifest file: {}", err));
                    Ok(EnvironmentManifest::new())
                }
            },
            Err(_) => Ok(EnvironmentManifest::new()),
        }
    }

    pub fn save(&self, environment: &impl Environment) -> Result<(), ErrBox> {
        let file_path = get_environment_manifest_file(environment)?;
        let serialized_manifest = serde_json::to_string(&self)?;
        environment.write_file_text(&file_path, &serialized_manifest)?;
        Ok(())
    }

    pub fn get_paths(&self) -> &Vec<String> {
        &self.binary_paths
    }

    pub fn add_paths(&mut self, paths: Vec<String>) {
        for path in paths {
            if !self.binary_paths.contains(&path) {
                self.binary_paths.push(path);
            }
        }
    }

    pub fn remove_paths(&mut self, paths: &Vec<String>) {
        for path in paths.iter() {
            if let Some(pos) = self.binary_paths.iter().position(|p| p == path) {
                self.binary_paths.remove(pos);
            }
        }
    }
}

fn get_environment_manifest_file(environment: &impl Environment) -> Result<PathBuf, ErrBox> {
    let user_data_dir = environment.get_user_data_dir()?; // share across domains
    Ok(user_data_dir.join("environment.json"))
}
