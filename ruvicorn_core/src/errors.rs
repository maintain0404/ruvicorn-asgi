use std::fmt;
use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub struct AsgiSpecError {}

impl Display for AsgiSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AsgiSpecError")
    }
}

impl Error for AsgiSpecError {}
