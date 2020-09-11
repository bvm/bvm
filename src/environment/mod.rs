#[macro_use]
mod environment;

mod common;

#[cfg(unix)]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

mod real_environment;
#[cfg(test)]
mod test_environment;

pub use environment::*;
pub use real_environment::*;

#[cfg(test)]
pub use test_environment::*;
