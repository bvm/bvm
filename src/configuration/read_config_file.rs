use crate::types::VersionSelector;
use dprint_cli_core::checksums::{parse_checksum_path_or_url, ChecksumPathOrUrl};
use dprint_cli_core::types::ErrBox;
use jsonc_parser::{parse_to_value, JsonValue};

pub struct ConfigFile {
    pub on_pre_install: Option<String>,
    pub on_post_install: Option<String>,
    pub binaries: Vec<ConfigFileBinary>,
}

pub struct ConfigFileBinary {
    pub path: ChecksumPathOrUrl,
    pub version: Option<VersionSelector>,
}

pub fn read_config_file(file_text: &str) -> Result<ConfigFile, ErrBox> {
    let value = parse_to_value(file_text)?;
    let mut root_object = match value {
        Some(JsonValue::Object(obj)) => obj,
        _ => return err!("Expected a root object in the json file."),
    };

    let json_binaries = root_object
        .take_array("binaries")
        .ok_or_else(|| err_obj!("Expected to find a 'binaries' array."))?;

    let mut binaries = Vec::new();
    for value in json_binaries.into_iter() {
        binaries.push(match value {
            JsonValue::String(text) => ConfigFileBinary {
                path: parse_checksum_path_or_url(&text),
                version: None,
            },
            JsonValue::Object(mut obj) => {
                let path = obj
                    .take_string("path")
                    .ok_or_else(|| err_obj!("Expected to find a 'path' string in binary object."))?;
                let version = obj.take_string("version");
                let checksum = obj.take_string("checksum");

                ConfigFileBinary {
                    path: if let Some(checksum) = checksum {
                        ChecksumPathOrUrl {
                            path_or_url: path,
                            checksum: Some(checksum),
                        }
                    } else {
                        parse_checksum_path_or_url(&path)
                    },
                    version: if let Some(version) = version {
                        Some(VersionSelector::parse_for_config(&version)?)
                    } else {
                        None
                    },
                }
            }
            _ => return err!("Expected a string or object for items in 'binaries' array."),
        });
    }

    let on_pre_install = root_object.take_string("onPreInstall");
    let on_post_install = root_object.take_string("onPostInstall");

    for (key, _) in root_object.into_iter() {
        return err!("Unknown key '{}'", key);
    }

    Ok(ConfigFile {
        binaries,
        on_pre_install,
        on_post_install,
    })
}
