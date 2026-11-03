//! Adult video identities and fields never extend the ordinary movie model.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogNumber {
    pub raw: String,
    pub normalized: String,
    pub namespace: String,
    pub is_primary: bool,
    pub aliases: Vec<String>,
    #[serde(default)]
    pub part: Option<CatalogPart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogPartKind {
    Cd,
    Part,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogPart {
    pub kind: CatalogPartKind,
    pub number: u16,
}

/// Catalog identities preserve issuer namespaces and zero padding. Only ASCII
/// presentation separators are normalized; incompatible namespaces never merge.
pub fn normalize_catalog_number(
    raw: &str,
    namespace: &str,
) -> Result<CatalogNumber, crate::ProviderFailure> {
    use crate::ProviderFailure;
    if raw.trim().is_empty()
        || raw.len() > 128
        || namespace.is_empty()
        || namespace.len() > 128
        || raw.chars().any(char::is_control)
        || !namespace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
    {
        return Err(ProviderFailure::InvalidConfiguration);
    }
    let mut cleaned = raw.trim().to_ascii_uppercase().replace('_', "-");
    let mut part = None;
    for (marker, kind) in [("PART", CatalogPartKind::Part), ("CD", CatalogPartKind::Cd)] {
        if let Some(index) = cleaned.rfind(marker) {
            if index == 0
                || !cleaned.as_bytes()[index - 1].is_ascii_digit()
                    && !matches!(cleaned.as_bytes()[index - 1], b'-' | b' ')
            {
                continue;
            }
            let suffix = cleaned[index + marker.len()..].trim_start_matches(['-', ' ']);
            if !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()) {
                let number = suffix
                    .parse::<u16>()
                    .ok()
                    .filter(|v| *v > 0)
                    .ok_or(ProviderFailure::InvalidConfiguration)?;
                part = Some(CatalogPart { kind, number });
                cleaned.truncate(index);
                cleaned = cleaned.trim_end_matches(['-', ' ']).into();
                break;
            }
        }
    }
    if !cleaned
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(ProviderFailure::InvalidConfiguration);
    }
    let compressed = cleaned.replace('-', "");
    let split = compressed
        .bytes()
        .rposition(|b| !b.is_ascii_digit())
        .map(|i| i + 1)
        .ok_or(ProviderFailure::InvalidConfiguration)?;
    if split == 0
        || split == compressed.len()
        || !compressed[..split].bytes().any(|b| b.is_ascii_alphabetic())
        || !compressed[..split]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric())
    {
        return Err(ProviderFailure::InvalidConfiguration);
    }
    let prefix = if &compressed[..split] == "FC2PPV" {
        "FC2-PPV"
    } else {
        &compressed[..split]
    };
    let normalized = format!("{prefix}-{}", &compressed[split..]);
    let aliases = if normalized == cleaned {
        vec![compressed]
    } else {
        vec![cleaned, compressed]
    };
    Ok(CatalogNumber {
        raw: raw.into(),
        normalized,
        namespace: namespace.into(),
        is_primary: true,
        aliases,
        part,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdultVideoMetadata {
    pub media_item_id: Uuid,
    pub catalog_numbers: Vec<CatalogNumber>,
    pub studio: Option<Uuid>,
    pub label: Option<Uuid>,
    pub series: Option<String>,
    pub release_date: Option<NaiveDate>,
    pub runtime_minutes: Option<u32>,
    pub title: String,
    pub original_title: Option<String>,
    pub description: Option<String>,
    pub performers: Vec<Uuid>,
    pub directors: Vec<Uuid>,
    pub genres: Vec<String>,
    pub cover: Option<Uuid>,
    pub back_cover: Option<Uuid>,
    pub samples: Vec<Uuid>,
    pub trailer: Option<String>,
    pub source_provider: Option<String>,
    pub source_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_numbers_retain_raw_values_and_issuer_namespaces() {
        let number = CatalogNumber {
            raw: "Abc_001".into(),
            normalized: "ABC-001".into(),
            namespace: "issuer-a".into(),
            is_primary: true,
            aliases: vec!["ABC001".into()],
            part: None,
        };
        let decoded: CatalogNumber =
            serde_json::from_value(serde_json::to_value(&number).unwrap()).unwrap();
        assert_eq!(decoded, number);
        let mut reissue = number.clone();
        reissue.namespace = "issuer-b".into();
        assert_ne!(number, reissue);
        let metadata = AdultVideoMetadata {
            media_item_id: Uuid::new_v4(),
            catalog_numbers: vec![number, reissue],
            studio: Some(Uuid::new_v4()),
            label: None,
            series: None,
            release_date: NaiveDate::from_ymd_opt(2024, 4, 1),
            runtime_minutes: Some(120),
            title: "Fixture title".into(),
            original_title: None,
            description: None,
            performers: vec![Uuid::new_v4()],
            directors: vec![],
            genres: vec![],
            cover: Some(Uuid::new_v4()),
            back_cover: None,
            samples: vec![Uuid::new_v4()],
            trailer: None,
            source_provider: Some("fixture-provider".into()),
            source_url: None,
        };
        let decoded: AdultVideoMetadata =
            serde_json::from_value(serde_json::to_value(&metadata).unwrap()).unwrap();
        assert_eq!(decoded, metadata);
        assert_eq!(decoded.catalog_numbers.len(), 2);
    }
    #[test]
    fn catalog_normalization_preserves_namespace_padding_and_parts() {
        for (raw, expected) in [
            ("abc_001", "ABC-001"),
            ("ABC001", "ABC-001"),
            ("  Abc-001  ", "ABC-001"),
            ("abcd001", "ABCD-001"),
            ("1abc001", "1ABC-001"),
            ("FC2-PPV-001", "FC2-PPV-001"),
        ] {
            let number = normalize_catalog_number(raw, "issuer-a").unwrap();
            assert_eq!(number.raw, raw);
            assert_eq!(number.normalized, expected);
            assert_eq!(number.namespace, "issuer-a");
            assert_eq!(number.part, None);
            assert!(number.is_primary);
        }
        let cd = normalize_catalog_number("abc_001-CD2", "issuer-a").unwrap();
        let part = normalize_catalog_number("abc001-Part-2", "issuer-a").unwrap();
        assert_eq!(cd.normalized, "ABC-001");
        assert_eq!(
            cd.part,
            Some(CatalogPart {
                kind: CatalogPartKind::Cd,
                number: 2
            })
        );
        assert_eq!(
            part.part,
            Some(CatalogPart {
                kind: CatalogPartKind::Part,
                number: 2
            })
        );
        assert_ne!(cd.part, part.part);
        assert_ne!(
            normalize_catalog_number("ABC-1", "issuer-a")
                .unwrap()
                .normalized,
            cd.normalized
        );
        assert_ne!(
            normalize_catalog_number("ABC001", "issuer-b")
                .unwrap()
                .namespace,
            cd.namespace
        );
        for raw in ["", "ABC", "001", "ABC001/../../x", "ABC001\n", "ABC001-CD0"] {
            assert!(
                normalize_catalog_number(raw, "issuer-a").is_err(),
                "{raw:?}"
            );
        }
        let legacy = r#"{"raw":"ABC001","normalized":"ABC-001","namespace":"issuer","is_primary":true,"aliases":[]}"#;
        assert_eq!(
            serde_json::from_str::<CatalogNumber>(legacy).unwrap().part,
            None
        );
    }
}
