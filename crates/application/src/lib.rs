//! Application use cases are independent of the desktop framework and storage.

pub mod library;
pub mod music;

use mediaatlas_domain::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInfo {
    pub app_name: &'static str,
    pub app_version: String,
}

pub fn get_app_info(build_version: &str) -> Result<AppInfo, AppError> {
    if build_version.trim().is_empty() {
        return Err(AppError::InvalidConfiguration);
    }
    Ok(AppInfo {
        app_name: "MediaAtlas",
        app_version: build_version.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_desktop_build_version() {
        assert_eq!(get_app_info("1.2.3").unwrap().app_version, "1.2.3");
    }

    #[test]
    fn rejects_missing_build_metadata() {
        assert_eq!(get_app_info(" \n"), Err(AppError::InvalidConfiguration));
    }
}
