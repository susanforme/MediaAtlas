//! Video extensions keep season/episode and animation relationships explicit.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovieMetadata {
    pub media_item_id: Uuid,
    pub runtime_minutes: Option<u32>,
    pub budget: Option<u64>,
    pub revenue: Option<u64>,
    pub original_language: Option<String>,
    pub production_companies: Vec<Uuid>,
    pub collection: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EpisodeOrder {
    Aired,
    Dvd,
    Absolute,
    Anime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TvSeriesMetadata {
    pub media_item_id: Uuid,
    pub status: Option<String>,
    pub networks: Vec<Uuid>,
    pub first_air_date: Option<NaiveDate>,
    pub last_air_date: Option<NaiveDate>,
    pub season_order: EpisodeOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonMetadata {
    pub media_item_id: Uuid,
    pub series_id: Uuid,
    pub season_number: u32,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeNumberMapping {
    pub order: EpisodeOrder,
    pub season_number: Option<u32>,
    pub episode_number: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpisodeMetadata {
    pub media_item_id: Uuid,
    pub series_id: Uuid,
    pub season_id: Option<Uuid>,
    pub season_number: Option<u32>,
    pub episode_number: Option<u32>,
    pub absolute_number: Option<u32>,
    pub air_date: Option<NaiveDate>,
    pub runtime_minutes: Option<u32>,
    pub still: Option<Uuid>,
    pub number_mappings: Vec<EpisodeNumberMapping>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnimeMetadata {
    pub media_item_id: Uuid,
    /// Link to a Movie or TV entity rather than creating an isolated animation island.
    pub base_media_item_id: Option<Uuid>,
    pub format: Option<String>,
    pub season: Option<String>,
    pub cour: Option<u32>,
    pub studios: Vec<Uuid>,
    pub source: Option<String>,
    pub aliases: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_extensions_roundtrip_without_losing_series_and_season_identity() {
        let movie = MovieMetadata {
            media_item_id: Uuid::new_v4(),
            runtime_minutes: Some(125),
            budget: Some(10_000),
            revenue: None,
            original_language: Some("ja".into()),
            production_companies: vec![Uuid::new_v4()],
            collection: None,
        };
        let series = TvSeriesMetadata {
            media_item_id: Uuid::new_v4(),
            status: Some("continuing".into()),
            networks: vec![Uuid::new_v4()],
            first_air_date: NaiveDate::from_ymd_opt(2024, 4, 1),
            last_air_date: None,
            season_order: EpisodeOrder::Aired,
        };
        let season = SeasonMetadata {
            media_item_id: Uuid::new_v4(),
            series_id: series.media_item_id,
            season_number: 1,
            name: None,
            overview: None,
            poster: None,
        };
        let episode = EpisodeMetadata {
            media_item_id: Uuid::new_v4(),
            series_id: series.media_item_id,
            season_id: Some(season.media_item_id),
            season_number: Some(1),
            episode_number: Some(1),
            absolute_number: Some(1),
            air_date: None,
            runtime_minutes: Some(24),
            still: None,
            number_mappings: vec![],
        };
        let items = (movie, series, season, episode);
        let decoded: (
            MovieMetadata,
            TvSeriesMetadata,
            SeasonMetadata,
            EpisodeMetadata,
        ) = serde_json::from_value(serde_json::to_value(&items).unwrap()).unwrap();
        assert_eq!(decoded, items);
        assert_eq!(decoded.3.series_id, decoded.1.media_item_id);
        assert_eq!(decoded.3.season_id, Some(decoded.2.media_item_id));
    }

    #[test]
    fn alternate_episode_orders_survive_serialization() {
        let mappings = vec![
            EpisodeNumberMapping {
                order: EpisodeOrder::Aired,
                season_number: Some(2),
                episode_number: 1,
            },
            EpisodeNumberMapping {
                order: EpisodeOrder::Absolute,
                season_number: None,
                episode_number: 13,
            },
        ];
        let roundtrip: Vec<EpisodeNumberMapping> =
            serde_json::from_value(serde_json::to_value(&mappings).unwrap()).unwrap();
        assert_eq!(roundtrip, mappings);
        assert_eq!(roundtrip[0].episode_number, 1);
        assert_eq!(roundtrip[1].episode_number, 13);
    }

    #[test]
    fn anime_retains_the_underlying_video_link() {
        let anime = AnimeMetadata {
            media_item_id: Uuid::new_v4(),
            base_media_item_id: Some(Uuid::new_v4()),
            format: Some("tv".into()),
            season: Some("spring".into()),
            cour: Some(1),
            studios: vec![],
            source: None,
            aliases: vec!["別名".into()],
        };
        let decoded: AnimeMetadata =
            serde_json::from_value(serde_json::to_value(&anime).unwrap()).unwrap();
        assert_eq!(decoded, anime);
        assert_ne!(decoded.media_item_id, decoded.base_media_item_id.unwrap());
    }
}
