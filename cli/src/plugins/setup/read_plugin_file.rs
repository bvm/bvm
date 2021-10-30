use dprint_cli_core::types::ErrBox;

use crate::plugins::SerializedPluginFile;

pub fn read_plugin_file(file_bytes: &[u8]) -> Result<SerializedPluginFile, ErrBox> {
  // todo: don't use serde because this should fail with a nice error message if the schema version is not equal
  match serde_json::from_slice::<SerializedPluginFile>(&file_bytes) {
    Ok(file) => {
      if file.schema_version != 1 {
        return err!(
                    "Expected schema version 1, but found {}. This may indicate you need to upgrade your CLI version to use this binary.",
                    file.schema_version
                );
      }
      // Validate the binary owner and name
      if file.name.starts_with(".") || file.name.starts_with("_") {
        return err!("The binary owner and name should not start with '.' or '_'");
      } else if file.name == "bvm" {
        return err!("'bvm' is not allowed to be used as a binary name");
      } else if file.name.contains("||")
        || file.name.contains("~")
        || file.name.contains("(")
        || file.name.contains(")")
        || file.name.contains("'")
        || file.name.contains("!")
        || file.name.contains("*")
        || file.name.contains("/")
      {
        return err!("The binary owner and name may not contain any of these characters(||,~,(,),',!,*,/)");
      } else if file.name.len() > 224 {
        return err!("The binary owner and name should not execced 224 characters");
      }

      Ok(file)
    }
    Err(err) => err!("Error deserializing binary manifest file. {}", err.to_string()),
  }
}
