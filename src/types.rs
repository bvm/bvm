use dprint_cli_core::types::ErrBox;
use semver::Version as SemVersion;
use semver_parser;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::{Ord, Ordering, PartialOrd};
use std::fmt;
use std::hash::Hash;

#[derive(Debug, PartialEq, Clone)]
pub struct BinarySelector {
    pub owner: Option<String>,
    pub name: String,
}

impl BinarySelector {
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

impl fmt::Display for BinarySelector {
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
        let sem_ver = SemVersion::parse(text)?;
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
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

impl VersionSelector {
    pub fn parse(text: &str) -> Result<VersionSelector, ErrBox> {
        // todo: unit tests
        match VersionSelector::inner_parse(text.trim()) {
            Ok(result) => Ok(result),
            Err(err) => err!("Error parsing {} to a version. {}", text, err.to_string()),
        }
    }

    fn inner_parse<'a>(text: &'a str) -> Result<VersionSelector, semver_parser::parser::Error<'a>> {
        let mut p = semver_parser::parser::Parser::new(text)?;
        let major = p.numeric()?;
        let mut minor = None;
        let mut patch = None;

        if !p.is_eof() {
            minor = Some(p.dot_numeric()?);
            if !p.is_eof() {
                // Patch is good enough for our purposes
                // do not worry about pre and build as they are
                // in the full text.
                patch = Some(p.dot_numeric()?);
            }
        }
        Ok(VersionSelector {
            full_text: text.to_string(),
            major,
            minor,
            patch,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.full_text
    }

    pub fn to_version(&self) -> Result<Version, ErrBox> {
        if self.minor.is_some() && self.patch.is_some() {
            return Version::parse(self.as_str());
        }
        return err!(
            "Could not parse '{}' as semantic version with three parts (ex. 1.0.0).",
            self.as_str()
        );
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
