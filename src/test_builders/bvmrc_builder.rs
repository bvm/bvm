use crate::environment::{Environment, TestEnvironment};

use std::path::PathBuf;

enum BinaryItem {
    String(String),
    Object(BinaryItemObject),
}

struct BinaryItemObject {
    path: String,
    checksum: Option<String>,
    version: Option<String>,
}

pub struct BvmrcBuilder {
    environment: TestEnvironment,
    path: Option<String>,
    on_pre_install: Option<String>,
    on_post_install: Option<String>,
    binaries: Vec<BinaryItem>,
}

impl BvmrcBuilder {
    pub fn new(environment: &TestEnvironment) -> Self {
        BvmrcBuilder {
            environment: environment.clone(),
            path: None,
            on_pre_install: None,
            on_post_install: None,
            binaries: Vec::new(),
        }
    }

    pub fn path<'a>(&'a mut self, path: &str) -> &'a mut BvmrcBuilder {
        self.path = Some(path.to_string());
        self
    }

    pub fn on_pre_install<'a>(&'a mut self, script: &str) -> &'a mut BvmrcBuilder {
        self.on_pre_install = Some(script.to_string());
        self
    }

    pub fn on_post_install<'a>(&'a mut self, script: &str) -> &'a mut BvmrcBuilder {
        self.on_post_install = Some(script.to_string());
        self
    }

    pub fn add_binary_path<'a>(&'a mut self, path: &str) -> &'a mut BvmrcBuilder {
        self.binaries.push(BinaryItem::String(path.to_string()));
        self
    }

    pub fn add_binary_object<'a>(
        &'a mut self,
        path: &str,
        checksum: Option<&str>,
        version: Option<&str>,
    ) -> &'a mut BvmrcBuilder {
        self.binaries.push(BinaryItem::Object(BinaryItemObject {
            path: path.to_string(),
            checksum: checksum.map(|p| p.to_string()),
            version: version.map(|v| v.to_string()),
        }));
        self
    }

    pub fn build(&mut self) {
        let mut writer = String::new();
        writer.push_str("{");
        if let Some(text) = &self.on_pre_install {
            writer.push_str(&format!(r#""onPreInstall": "{}","#, escape_quotes(text)));
        }
        if let Some(text) = &self.on_post_install {
            writer.push_str(&format!(r#""onPostInstall": "{}","#, escape_quotes(text)));
        }
        writer.push_str(r#""binaries": ["#);
        for (i, binary_item) in self.binaries.iter().enumerate() {
            if i > 0 {
                writer.push_str(",");
            }
            match binary_item {
                BinaryItem::String(text) => writer.push_str(&format!(r#""{}""#, escape_quotes(text))),
                BinaryItem::Object(obj) => {
                    writer.push_str("{");
                    writer.push_str(&format!(r#""path": "{}""#, escape_quotes(&obj.path)));
                    if let Some(text) = &obj.checksum {
                        writer.push_str(&format!(r#","checksum": "{}""#, escape_quotes(text)));
                    }
                    if let Some(text) = &obj.version {
                        writer.push_str(&format!(r#","version": "{}""#, escape_quotes(text)));
                    }
                    writer.push_str("}");
                }
            }
        }
        writer.push_str("]");
        writer.push_str("}");
        let file_path = PathBuf::from(self.path.clone().unwrap_or("/project/.bvmrc.json".to_string()));
        self.environment.write_file_text(&file_path, &writer).unwrap();
    }
}

fn escape_quotes(text: &str) -> String {
    text.replace("\"", "\\\"")
}
