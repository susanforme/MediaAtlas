//! Versioned wire types; this crate intentionally has no Tauri dependency.

use mediaatlas_application::AppInfo;
use mediaatlas_domain::AppError;
use serde::Serialize;
use uuid::Uuid;

pub const SCHEMA_VERSION: u16 = 1;
pub const APP_READY_EVENT: &str = "app-ready";

#[derive(Debug, Clone, Serialize)]
pub struct AppInfoDto {
    pub schema_version: u16,
    pub app_name: &'static str,
    pub app_version: String,
}

impl From<AppInfo> for AppInfoDto {
    fn from(value: AppInfo) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            app_name: value.app_name,
            app_version: value.app_version,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDto {
    pub schema_version: u16,
    pub code: &'static str,
    pub message: &'static str,
    pub correlation_id: String,
}

impl From<AppError> for ErrorDto {
    fn from(value: AppError) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            code: value.code(),
            message: value.public_message(),
            correlation_id: Uuid::new_v4().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn success_payload_matches_the_frozen_frontend_contract() {
        let dto = AppInfoDto::from(mediaatlas_application::get_app_info("0.1.0").unwrap());
        assert_eq!(
            serde_json::to_value(dto).unwrap(),
            json!({
                "schema_version": 1,
                "app_name": "MediaAtlas",
                "app_version": "0.1.0"
            })
        );
    }

    #[test]
    fn errors_expose_only_allowlisted_text_and_unique_correlation_ids() {
        let first = ErrorDto::from(AppError::Internal);
        let second = ErrorDto::from(AppError::Internal);
        assert_ne!(first.correlation_id, second.correlation_id);
        assert!(Uuid::parse_str(&first.correlation_id).is_ok());
        let payload = serde_json::to_value(first).unwrap();
        assert_eq!(payload.as_object().unwrap().len(), 4);
        assert_eq!(payload["code"], "internal_error");
        assert_eq!(
            payload["message"],
            "An internal application error occurred."
        );
        assert_eq!(payload["schema_version"], 1);
    }
}
