//! Lexical identity rules. These functions do not resolve symlinks or access disk.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathStyle {
    Windows,
    Posix,
}

impl PathStyle {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Windows => "windows",
            Self::Posix => "posix",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    InvalidPath,
    InvalidValue,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPath => "Path is invalid for the selected platform.",
            Self::InvalidValue => "Domain value is invalid.",
        })
    }
}
impl std::error::Error for ValidationError {}

fn windows_component(value: &str) -> Result<String, ValidationError> {
    if value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|c| c.is_control() || "<>:\"|?*".contains(c))
    {
        return Err(ValidationError::InvalidPath);
    }
    let normalized = value.to_ascii_lowercase();
    let base = normalized.split('.').next().unwrap_or_default();
    if matches!(base, "con" | "prn" | "aux" | "nul" | "clock$")
        || base
            .strip_prefix("com")
            .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        || base
            .strip_prefix("lpt")
            .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
    {
        return Err(ValidationError::InvalidPath);
    }
    Ok(normalized)
}

fn components(path: &str, style: PathStyle) -> Result<Vec<String>, ValidationError> {
    path.split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(|part| {
            if part == ".." {
                return Err(ValidationError::InvalidPath);
            }
            match style {
                PathStyle::Windows => windows_component(part),
                PathStyle::Posix => Ok(part.to_owned()),
            }
        })
        .collect()
}

pub fn normalize_relative_path(path: &str, style: PathStyle) -> Result<String, ValidationError> {
    if path.is_empty() || path.contains('\0') {
        return Err(ValidationError::InvalidPath);
    }
    let path = match style {
        PathStyle::Windows => path.replace('\\', "/"),
        PathStyle::Posix => path.to_owned(),
    };
    if path.starts_with('/') {
        return Err(ValidationError::InvalidPath);
    }
    let parts = components(&path, style)?;
    if parts.is_empty() {
        return Err(ValidationError::InvalidPath);
    }
    Ok(parts.join("/"))
}

pub fn normalize_root_path(path: &str, style: PathStyle) -> Result<String, ValidationError> {
    if path.is_empty() || path.contains('\0') {
        return Err(ValidationError::InvalidPath);
    }
    match style {
        PathStyle::Posix => {
            if !path.starts_with('/') {
                return Err(ValidationError::InvalidPath);
            }
            let prefix = if path.starts_with("//") && !path.starts_with("///") {
                "//"
            } else {
                "/"
            };
            Ok(format!("{prefix}{}", components(path, style)?.join("/")))
        }
        PathStyle::Windows => {
            let path = path.replace('\\', "/");
            if let Some(tail) = path.strip_prefix("//") {
                if tail.starts_with('/') || matches!(tail.split('/').next(), Some("." | "?")) {
                    return Err(ValidationError::InvalidPath);
                }
                let parts = components(tail, style)?;
                if parts.len() < 2 {
                    return Err(ValidationError::InvalidPath);
                }
                return Ok(format!("//{}", parts.join("/")));
            }
            let bytes = path.as_bytes();
            if bytes.len() < 3
                || !bytes[0].is_ascii_alphabetic()
                || bytes[1] != b':'
                || bytes[2] != b'/'
            {
                return Err(ValidationError::InvalidPath);
            }
            Ok(format!(
                "{}:/{}",
                (bytes[0] as char).to_ascii_lowercase(),
                components(&path[3..], style)?.join("/")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_use_explicit_platform_rules_on_any_host() {
        assert_eq!(
            normalize_relative_path(r"Movie\Part1.MKV", PathStyle::Windows).unwrap(),
            "movie/part1.mkv"
        );
        assert_eq!(
            normalize_relative_path("Movie//./Part1.MKV", PathStyle::Posix).unwrap(),
            "Movie/Part1.MKV"
        );
        assert_eq!(
            normalize_relative_path(r"Movie\Part1.MKV", PathStyle::Posix).unwrap(),
            r"Movie\Part1.MKV"
        );
        assert_eq!(
            normalize_root_path(r"C:\Media\Movies\", PathStyle::Windows).unwrap(),
            "c:/media/movies"
        );
        assert_eq!(
            normalize_root_path(r"\\Server\Share\Movies", PathStyle::Windows).unwrap(),
            "//server/share/movies"
        );
        assert_eq!(
            normalize_root_path("/Media/Movies/", PathStyle::Posix).unwrap(),
            "/Media/Movies"
        );
    }

    #[test]
    fn traversal_and_windows_ambiguities_never_form_file_identities() {
        for path in ["../x", "movie/../x", "/absolute", "\0", ".", ""] {
            assert!(
                normalize_relative_path(path, PathStyle::Posix).is_err(),
                "{path:?}"
            );
        }
        for path in [
            r"C:\Media",
            "C:relative",
            r"\\server\share",
            "a:stream",
            "movie. /x",
            "CON.mkv",
            "Aux",
            "LPT1.txt",
            "a/../b",
            "name.",
            "name ",
            "a?b",
            "a\nb",
        ] {
            assert!(
                normalize_relative_path(path, PathStyle::Windows).is_err(),
                "{path:?}"
            );
        }
        for path in [
            "relative",
            "C:relative",
            r"\rooted",
            r"\\?\C:\Media",
            r"\\.\pipe\name",
            "///server/share",
            r"\\server",
            "C:/../Media",
        ] {
            assert!(
                normalize_root_path(path, PathStyle::Windows).is_err(),
                "{path:?}"
            );
        }
    }
}
