use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ValidationError;

/// A stable ASCII language-tag key, without a claim of registry/alias resolution.
pub fn normalize_locale(locale: Option<&str>) -> Result<Option<String>, ValidationError> {
    locale
        .map(|value| {
            if value.is_empty()
                || value.trim() != value
                || !value.split('-').all(|segment| {
                    !segment.is_empty()
                        && segment.len() <= 8
                        && segment.bytes().all(|b| b.is_ascii_alphanumeric())
                })
            {
                return Err(ValidationError::InvalidValue);
            }
            Ok(value.to_ascii_lowercase())
        })
        .transpose()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetadataValue {
    pub id: Uuid,
    pub media_item_id: Uuid,
    pub field_key: String,
    pub locale: Option<String>,
    pub value: serde_json::Value,
    pub provider_id: Option<String>,
    pub source_url: Option<String>,
    pub fetched_at: Option<DateTime<Utc>>,
    pub confidence: Option<f64>,
    pub is_user_modified: bool,
    pub is_locked: bool,
}

impl MetadataValue {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.field_key.trim().is_empty()
            || self.field_key.trim() != self.field_key
            || self.field_key.contains('\0')
            || self
                .confidence
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err(ValidationError::InvalidValue);
        }
        normalize_locale(self.locale.as_deref())?;
        Ok(())
    }

    /// Whether a future Provider merge must retain this value until user review.
    pub const fn is_protected(&self) -> bool {
        self.is_locked || self.is_user_modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value() -> MetadataValue {
        MetadataValue {
            id: Uuid::new_v4(),
            media_item_id: Uuid::new_v4(),
            field_key: "title".into(),
            locale: Some("zh-CN".into()),
            value: serde_json::json!("用户标题"),
            provider_id: None,
            source_url: None,
            fetched_at: None,
            confidence: Some(1.0),
            is_user_modified: true,
            is_locked: false,
        }
    }

    #[test]
    fn user_edits_are_protected_even_without_lock() {
        let value = value();
        assert!(value.is_protected());
        let decoded: MetadataValue =
            serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
        assert_eq!(decoded, value);
    }

    #[test]
    fn invalid_confidence_and_empty_locale_are_rejected() {
        for confidence in [f64::NAN, f64::INFINITY, -0.01, 1.01] {
            let mut value = value();
            value.confidence = Some(confidence);
            assert!(value.validate().is_err());
        }
        let mut value = value();
        value.locale = Some(String::new());
        assert!(value.validate().is_err());
    }
}
