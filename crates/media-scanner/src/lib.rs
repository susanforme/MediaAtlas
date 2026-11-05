//! Streaming, read-only local media scanning. No network or database dependency.

mod local;
mod models;
mod parser;
mod scanner;

pub use local::{parse_nfo, read_audio_tags, read_nfo};
pub use models::*;
pub use parser::parse_filename;
pub use scanner::{run_scan, run_scan_selected, sidecar_fingerprint};
