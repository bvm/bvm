use dprint_cli_core::types::ErrBox;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::environment::Environment;
use crate::types::BinaryName;
use crate::types::Version;
use crate::utils::ChecksumUrl;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryFile {
  pub schema_version: u32,
  pub binaries: Vec<RegistryBinary>,
}

impl RegistryFile {
  pub fn take_binary_with_name(self, name: &BinaryName) -> Option<RegistryBinary> {
    self
      .binaries
      .into_iter()
      .filter(|b| b.owner == name.owner && b.name == name.name)
      .next()
  }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryBinary {
  name: String,
  owner: String,
  pub description: String,
  pub versions: Vec<RegistryVersionInfo>,
}

impl RegistryBinary {
  pub fn get_binary_name(&self) -> BinaryName {
    BinaryName::new(self.owner.clone(), self.name.clone())
  }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegistryVersionInfo {
  pub version: Version,
  pub path: String,
  pub checksum: String,
}

impl RegistryVersionInfo {
  pub fn get_url(&self) -> Result<ChecksumUrl, ErrBox> {
    Ok(ChecksumUrl {
      url: Url::parse(&self.path)?,
      unresolved_path: self.path.clone(),
      checksum: Some(self.checksum.clone()),
    })
  }
}

pub fn download_registry_file<'a, TEnvironment: Environment>(
  environment: &TEnvironment,
  url: &str,
) -> Result<RegistryFile, ErrBox> {
  let plugin_file_bytes = environment.download_file(&url)?;

  read_registry_file(&plugin_file_bytes)
}

fn read_registry_file(file_bytes: &[u8]) -> Result<RegistryFile, ErrBox> {
  // todo: don't use serde because this should transform up to the latest schema version
  match serde_json::from_slice::<RegistryFile>(&file_bytes) {
    Ok(file) => {
      if file.schema_version != 1 {
        return err!(
                    "Expected schema version 1, but found {}. This may indicate you need to upgrade your CLI version to use this registry file.",
                    file.schema_version
                );
      }

      for binary in file.binaries.iter() {
        verify_binary_name(&binary)?;
      }

      Ok(file)
    }
    Err(err) => err!("Error deserializing registry file. {}", err.to_string()),
  }
}

fn verify_binary_name(binary: &RegistryBinary) -> Result<(), ErrBox> {
  if binary.name.contains("/") || binary.owner.contains("/") {
    return err!("The binary owner and name may not contain a forward slash.");
  }

  Ok(())
}
