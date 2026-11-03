//! Media identities and type-specific models, independent of storage and desktop I/O.

pub mod adult;
pub mod adult_scraper;
pub mod library;
pub mod metadata;
pub mod music;
pub mod music_workflow;
pub mod paths;
pub mod provider;
pub mod scraper;
pub mod tasks;
pub mod video;

pub use adult::*;
pub use adult_scraper::*;
pub use library::*;
pub use metadata::*;
pub use music::*;
pub use music_workflow::*;
pub use paths::*;
pub use provider::*;
pub use scraper::*;
pub use tasks::*;
pub use video::*;

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
