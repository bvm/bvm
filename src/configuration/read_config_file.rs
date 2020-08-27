use jsonc_parser::{parse_to_value, JsonValue};

use crate::types::ErrBox;
use crate::utils;

pub struct ConfigFile {
    pub binaries: Vec<utils::ChecksumUrl>,
}

pub fn read_config_file(file_text: &str) -> Result<ConfigFile, ErrBox> {
    let value = match parse_to_value(file_text) {
        Ok(value) => value,
        Err(err) => {
            return err!(
                "Error parsing configuration file. {}",
                err.get_message_with_range(file_text)
            )
        }
    };

    let mut root_object_node = match value {
        Some(JsonValue::Object(obj)) => obj,
        _ => return err!("Expected a root object in the json"),
    };

    let json_binaries = match root_object_node.take_array("binaries") {
        Some(json_binaries) => json_binaries,
        None => return err!("Expected to find a 'binaries' array."),
    };

    let mut binaries = Vec::new();
    for value in json_binaries.into_iter() {
        let url = match value {
            JsonValue::String(text) => utils::parse_checksum_url(&text),
            _ => return err!("Expected a string for all items in 'binaries' array."),
        };
        binaries.push(url);
    }

    Ok(ConfigFile { binaries })
}
