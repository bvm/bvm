use std::error::Error as StdError;

#[derive(Debug, PartialEq, Clone)]
pub struct BinaryName {
    pub owner: Option<String>,
    pub name: String,
}

impl BinaryName {
    pub fn new(owner: Option<String>, name: String) -> BinaryName {
        BinaryName { owner, name }
    }

    pub fn is_match(&self, owner: &str, name: &str) -> bool {
        if name == self.name {
            if let Some(owner_name) = &self.owner {
                owner_name == owner
            } else {
                true
            }
        } else {
            false
        }
    }

    pub fn display(&self) -> String {
        if let Some(owner) = &self.owner {
            format!("{}/{}", owner, self.name)
        } else {
            self.name.clone()
        }
    }

    pub fn display_toggled_owner(&self, display_owner: bool) -> String {
        if display_owner {
            self.display()
        } else {
            self.name.clone()
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

pub type ErrBox = Box<dyn StdError + Send + Sync>;

#[derive(std::fmt::Debug)]
pub struct Error(String);

impl Error {
    pub fn new(text: String) -> Box<Self> {
        Box::new(Error(text))
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl StdError for Error {}

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {
        Err($crate::types::Error::new(format!($($arg)*)));
    }
}
