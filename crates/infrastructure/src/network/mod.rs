//! Bounded HTTP engines. Never serializes credentials, request bodies or raw errors.
mod music;
mod parsers;
mod policy;
mod transport;
pub use parsers::{parse_html, parse_json, HtmlDocument};
pub use policy::{is_public_ip, safe_origin};
pub use tokio_util::sync::CancellationToken;
pub use transport::*;

#[cfg(test)]
mod tests;
