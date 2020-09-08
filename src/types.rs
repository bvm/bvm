use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

#[derive(Debug, PartialEq, Clone)]
pub struct BinarySelector {
    pub owner: Option<String>,
    pub name: CommandName,
}

impl BinarySelector {
    pub fn is_match(&self, name: &BinaryName) -> bool {
        if name.name == self.name {
            if let Some(owner_name) = &self.owner {
                owner_name == &name.owner
            } else {
                true
            }
        } else {
            false
        }
    }

    pub fn display(&self) -> String {
        if let Some(owner) = &self.owner {
            format!("{}/{}", owner, self.name.display())
        } else {
            self.name.display().to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct BinaryName {
    serialized_value: String,
    pub owner: String,
    pub name: CommandName,
}

impl PartialEq for BinaryName {
    fn eq(&self, other: &Self) -> bool {
        self.serialized_value == other.serialized_value
    }
}

impl Eq for BinaryName {}

impl Hash for BinaryName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.serialized_value.hash(state);
    }
}

impl Serialize for BinaryName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.serialized_value)
    }
}

struct BinaryNameVisitor;

impl<'de> Visitor<'de> for BinaryNameVisitor {
    type Value = BinaryName;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a binary full name in the format owner/name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let items = v.split("/").collect::<Vec<_>>();
        Ok(BinaryName::new(items[0].to_string(), items[1].to_string()))
    }
}

impl<'de> Deserialize<'de> for BinaryName {
    fn deserialize<D>(deserializer: D) -> Result<BinaryName, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BinaryNameVisitor)
    }
}

impl BinaryName {
    pub fn new(owner: String, name: String) -> BinaryName {
        BinaryName {
            serialized_value: format!("{}/{}", owner, name),
            owner,
            name: CommandName::from_string(name),
        }
    }

    pub fn compare(&self, other: &BinaryName) -> Ordering {
        self.serialized_value.partial_cmp(&other.serialized_value).unwrap()
    }

    pub fn display(&self) -> String {
        format!("{}/{}", self.owner, self.name.display())
    }

    pub fn display_toggled_owner(&self, display_owner: bool) -> String {
        if display_owner {
            self.display()
        } else {
            self.name.display().to_string()
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct CommandName(String);

impl CommandName {
    pub fn from_string(value: String) -> CommandName {
        CommandName(value)
    }

    pub fn display(&self) -> &str {
        self.as_str()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}
