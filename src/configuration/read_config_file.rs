use std::collections::HashMap;
use jsonc_parser::{parse_to_value, JsonValue};
use crate::types::ErrBox;

pub struct ConfigFile {
    pub dependencies: HashMap<String, ConfigFileBinary>,
}

pub struct ConfigFileBinary {
    pub name: String,
    pub url: String,
}

pub fn read_config_file(file_text: &str) -> Result<ConfigFile, ErrBox> {
    let value = match parse_to_value(file_text) {
        Ok(value) => value,
        Err(err) => return err!("Error parsing configuration file. {}", err.get_message_with_range(file_text)),
    };

    let mut root_object_node = match value {
        Some(JsonValue::Object(obj)) => obj,
        _ => return err!("Expected a root object in the json"),
    };

    let json_dependencies = match root_object_node.take_object("dependencies") {
        Some(json_dependencies) => json_dependencies,
        None => return err!("Expected to find a 'dependencies' array."),
    };

    let mut dependencies = HashMap::new();
    for (key, value) in json_dependencies.into_iter() {
        let url = match value {
            JsonValue::String(url) => url,
            _ => return err!("Expected a string for key '{}'.", key),
        };
        dependencies.insert(key.clone(), ConfigFileBinary {
            name: key,
            url,
        });
    }

    Ok(ConfigFile {
        dependencies
    })
}