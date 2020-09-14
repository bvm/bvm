#[macro_use]
mod environment;

mod common;

mod real_environment;
#[cfg(test)]
mod test_environment;

pub use environment::*;
pub use real_environment::*;

#[cfg(test)]
pub use test_environment::*;

mod variable_names;

pub use variable_names::*;
