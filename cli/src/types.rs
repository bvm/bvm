use dprint_cli_core::types::ErrBox;
use regex::Regex;
use semver::{Version as SemVersion, VersionReq as SemVersionReq};
use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::{Ord, Ordering, PartialOrd};
use std::fmt;
use std::hash::Hash;

#[derive(Debug, PartialEq, Clone)]
pub struct NameSelector {
    pub owner: Option<String>,
    pub name: String,
}

impl NameSelector {
    pub fn is_match(&self, name: &BinaryName) -> bool {
        if name.name.as_str() == self.name {
            if let Some(owner_name) = &self.owner {
                owner_name == &name.owner
            } else {
                true
            }
        } else {
            false
        }
    }
}

impl fmt::Display for NameSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(owner) = &self.owner {
            write!(f, "{}/{}", owner, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

pub enum PathOrVersionSelector {
    Path,
    Version(VersionSelector),
}

impl PathOrVersionSelector {
    pub fn parse(text: &str) -> Result<PathOrVersionSelector, ErrBox> {
        Ok(if text.to_lowercase() == "path" {
            PathOrVersionSelector::Path
        } else {
            PathOrVersionSelector::Version(VersionSelector::parse(text)?)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Version {
    full_text: String,
    sem_ver: SemVersion,
}

impl Version {
    pub fn parse(text: &str) -> Result<Version, ErrBox> {
        let sem_ver = match SemVersion::parse(text) {
            Ok(version) => version,
            Err(err) => return err!("Error parsing version to format `x.x.x`. {}", err.to_string()),
        };

        Ok(Version {
            full_text: text.to_string(),
            sem_ver,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.full_text
    }

    pub fn is_prerelease(&self) -> bool {
        self.sem_ver.is_prerelease()
    }

    pub fn to_selector(&self) -> VersionSelector {
        VersionSelector::parse(&self.full_text).unwrap() // should always work
    }
}

/// For testing purposes.
#[cfg(test)]
impl From<&str> for Version {
    fn from(value: &str) -> Self {
        Version::parse(value).unwrap()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        self.sem_ver.partial_cmp(&other.sem_ver)
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        self.sem_ver.cmp(&other.sem_ver)
    }
}

// todo: there must be a shorter way to do this serialization and deserialization?
impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.full_text)
    }
}

struct VersionVisitor;

impl<'de> Visitor<'de> for VersionVisitor {
    type Value = Version;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a valid semantic version")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Version::parse(v).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Version, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(VersionVisitor)
    }
}

pub struct VersionSelector {
    full_text: String,
    version_req: SemVersionReq,
}

impl VersionSelector {
    pub fn parse(text: &str) -> Result<VersionSelector, ErrBox> {
        lazy_static! {
            static ref FULL_VERSION_RE: Regex = Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+$").unwrap();
            static ref MINOR_VERSION_RE: Regex = Regex::new(r"^[0-9]+\.[0-9]+$").unwrap();
        }
        let text = text.trim();
        // make full versions exact and minor versions only within the minor
        if FULL_VERSION_RE.is_match(text) {
            Ok(VersionSelector::inner_parse(&format!("={}", text), text)?)
        } else if MINOR_VERSION_RE.is_match(text) {
            Ok(VersionSelector::inner_parse(&format!("~{}.0", text), text)?)
        } else {
            VersionSelector::inner_parse(text, text)
        }
    }

    /// Parses where "1" is equivalent to "^1" and "1.1" is equivalent to "^1.1"
    pub fn parse_for_config(text: &str) -> Result<VersionSelector, ErrBox> {
        VersionSelector::inner_parse(text, text)
    }

    fn inner_parse<'a>(version_text: &'a str, full_text: &'a str) -> Result<VersionSelector, ErrBox> {
        let version_req = match SemVersionReq::parse(version_text) {
            Ok(result) => result,
            Err(err) => return err!("Error parsing {} to a version. {}", version_text, err.to_string()),
        };
        Ok(VersionSelector {
            full_text: full_text.to_string(),
            version_req,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.full_text
    }

    pub fn matches(&self, version: &Version) -> bool {
        self.version_req.matches(&version.sem_ver)
    }
}

impl fmt::Display for VersionSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BinaryName {
    pub owner: String,
    pub name: String,
}

impl Serialize for BinaryName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{}/{}", self.owner, self.name))
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
        BinaryName { owner, name }
    }

    pub fn display_toggled_owner(&self, display_owner: bool) -> String {
        if display_owner {
            format!("{}", self)
        } else {
            self.name.clone()
        }
    }

    pub fn to_selector(&self) -> NameSelector {
        NameSelector {
            owner: Some(self.owner.clone()),
            name: self.name.clone(),
        }
    }
}

impl fmt::Display for BinaryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

impl PartialOrd for BinaryName {
    fn partial_cmp(&self, other: &BinaryName) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BinaryName {
    fn cmp(&self, other: &BinaryName) -> Ordering {
        let ordering = self.owner.cmp(&other.owner);
        match ordering {
            Ordering::Equal => self.name.cmp(&other.name),
            _ => ordering,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct CommandName(String);

impl CommandName {
    pub fn from_string(value: String) -> CommandName {
        CommandName(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for CommandName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

struct CommandNameVisitor;

impl<'de> Visitor<'de> for CommandNameVisitor {
    type Value = CommandName;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a command name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(CommandName::from_string(v.to_string()))
    }
}

impl<'de> Deserialize<'de> for CommandName {
    fn deserialize<D>(deserializer: D) -> Result<CommandName, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(CommandNameVisitor)
    }
}
