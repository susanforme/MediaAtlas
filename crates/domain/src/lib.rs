//! Domain errors have stable codes and safe public messages, with no I/O dependencies.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppError {
    InvalidConfiguration,
    Internal,
}

impl AppError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "invalid_configuration",
            Self::Internal => "internal_error",
        }
    }

    pub const fn public_message(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "Application configuration is invalid.",
            Self::Internal => "An internal application error occurred.",
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.public_message())
    }
}

impl std::error::Error for AppError {}
