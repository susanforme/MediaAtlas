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
}
