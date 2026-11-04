use super::*;
use std::collections::{BTreeMap, HashSet};

pub(super) fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: "tvmaze".into(),
        name: "TVmaze".into(),
        description: "Public television metadata, aired seasons and episodes.".into(),
        website: Some("https://www.tvmaze.com".into()),
        terms_url: Some("https://www.tvmaze.com/api#licensing".into()),
        privacy_url: None,
        license: "CC BY-SA 4.0; retain TVmaze attribution and source URL".into(),
        media_kinds: vec![MediaKind::TvSeries],
        languages: vec!["en-US".into()],
        capabilities: vec!["search".into(), "details".into(), "images".into(), "external_ids".into()],
        fixture: false,
        documentation_url: Some("https://www.tvmaze.com/api".into()),
        engine_types: vec!["api".into()],
        requires_api_key: false,
        requires_login: false,
        rate_limit_policy: "Public API; enforced >= 1 s between requests, concurrency 1; honor 429 and Retry-After.".into(),
        attribution: "Metadata supplied by TVmaze, https://www.tvmaze.com; CC BY-SA 4.0.".into(),
        data_usage_notes: "Default disabled. English source text is retained without claiming translation; attribution and license accompany exported metadata. Unnumbered specials are not mapped to invented aired episode numbers.".into(),
    }
}

impl VideoProviderHub {
    async fn tvmaze_json(
        &self,
        path: &str,
        parameters: &[(&str, &str)],
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Value, ProviderFailure> {
        let mut url = Url::parse(&format!("{}{path}", self.tvmaze_base))
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        for (key, value) in parameters {
            url.query_pairs_mut().append_pair(key, value);
        }
        let mut effective = job.clone();
        effective.config.interval_ms = effective.config.interval_ms.max(1000);
        effective.config.concurrency = 1;
        self.engine.execute(&effective, RequestSpec::get(url.as_str(), EngineKind::Api), cancel).await?.json()
    }

    pub(super) async fn tvmaze_search(
        &self,
        query: &VideoQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        let (title, _, ids, _) = query.parts();
        if let Some(id) = ids.iter().find(|id| id.namespace == "tvmaze") {
            numeric_id(&id.value)?;
            let value = self.tvmaze_json(&format!("/shows/{}", id.value), &[], job, cancel).await?;
            let candidate = candidate(&value)?;
            if candidate.data().remote_id != id.value {
                return Err(ProviderFailure::IdentityMismatch);
            }
            return Ok(vec![candidate]);
        }
        if let Some(id) = ids.iter().find(|id| matches!(id.namespace.as_str(), "imdb" | "tvdb")) {
            let key = if id.namespace == "imdb" {
                if !id.value.starts_with("tt") || id.value.len() > 20 {
                    return Err(ProviderFailure::InvalidConfiguration);
                }
                numeric_id(&id.value[2..])?;
                "imdb"
            } else {
                numeric_id(&id.value)?;
                "thetvdb"
            };
            let value = self.tvmaze_json("/lookup/shows", &[(key, &id.value)], job, cancel).await?;
            let candidate = candidate(&value)?;
            if !candidate.data().external_ids.iter().any(|actual| actual == id) {
                return Err(ProviderFailure::IdentityMismatch);
            }
            return Ok(vec![candidate]);
        }
        let value = self.tvmaze_json("/search/shows", &[("q", title)], job, cancel).await?;
        let rows = value.as_array().ok_or(ProviderFailure::ParseError)?;
        if rows.len() > MAX_RESULTS {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        rows.iter().map(|row| candidate(&row["show"])).collect()
    }

    pub(super) async fn tvmaze_detail(
        &self,
        candidate: &VideoCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        let id = &candidate.data().remote_id;
        numeric_id(id)?;
        let value = self.tvmaze_json(&format!("/shows/{id}"), &[("embed", "episodes")], job, cancel).await?;
        let seasons = self.tvmaze_json(&format!("/shows/{id}/seasons"), &[], job, cancel).await?;
        metadata(&value, &seasons, id)
    }
}

fn candidate(value: &Value) -> Result<VideoCandidate, ProviderFailure> {
    let id = required_id(&value["id"])?;
    numeric_id(&id)?;
    let mut external_ids = vec![RemoteId { namespace: "tvmaze".into(), value: id.clone() }];
    for (key, namespace) in [("imdb", "imdb"), ("thetvdb", "tvdb")] {
        if let Some(value) = scalar_id(&value["externals"][key]) {
            external_ids.push(RemoteId { namespace: namespace.into(), value });
        }
    }
    let year = value["premiered"].as_str().filter(|date| !date.is_empty()).map(|date| {
        date.get(..4).ok_or(ProviderFailure::ParseError)?.parse::<i32>().map_err(|_| ProviderFailure::ParseError)
    }).transpose()?;
    if year.is_some_and(|year| !(1800..=2200).contains(&year)) {
        return Err(ProviderFailure::ParseError);
    }
    Ok(VideoCandidate::Tv(CandidateData {
        provider_id: "tvmaze".into(), remote_id: id.clone(),
        title: required_str(value, "name")?.into(), original_title: None, year,
        aliases: vec![], external_ids, source_url: format!("https://www.tvmaze.com/shows/{id}"),
        language: "en-US".into(),
    }))
}

fn summary(value: &Value) -> Result<Option<String>, ProviderFailure> {
    value.as_str().map(|html| parse_html(html).css_text("body").map(|text| text.join("\n"))).transpose()
}

fn metadata(value: &Value, seasons: &Value, expected: &str) -> Result<VideoMetadata, ProviderFailure> {
    let identity = candidate(value)?.data().clone();
    if identity.remote_id != expected {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let overview = summary(&value["summary"])?;
    let mut fields = standard_fields(&identity, overview.as_deref());
    let genres = array(value, "genres")?;
    if genres.len() > 100 {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    fields.push(field("genres", json!(genres.iter().filter_map(Value::as_str).collect::<Vec<_>>()), Some("en-US")));
    if let Some(rating) = value["rating"]["average"].as_f64().filter(|rating| (0.0..=10.0).contains(rating)) {
        fields.push(field("rating", json!(rating), None));
    }
    fields.push(field("attribution", json!(format!("TVmaze; {}; CC BY-SA 4.0", identity.source_url)), None));
    fields.push(field("source_license", json!("CC BY-SA 4.0; https://creativecommons.org/licenses/by-sa/4.0/"), None));
    let mut artwork = Vec::new();
    if let Some(url) = value["image"]["original"].as_str().or_else(|| value["image"]["medium"].as_str()) {
        validate_image_url(url, &["static.tvmaze.com"])?;
        artwork.push(ArtworkCandidate { url: url.into(), source_provider: "tvmaze".into(), kind: "poster".into(), language: None });
    }
    let rows = seasons.as_array().ok_or(ProviderFailure::ParseError)?;
    if rows.len() > MAX_SEASONS {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    let mut seasons = BTreeMap::new();
    for row in rows {
        let number = u32_value(&row["number"])?.ok_or(ProviderFailure::ParseError)?;
        let id = required_id(&row["id"])?;
        numeric_id(&id)?;
        if seasons.insert(number, SeasonDetail { remote_id: id, season_number: number, title: row["name"].as_str().unwrap_or_default().into(), episodes: Vec::new() }).is_some() {
            return Err(ProviderFailure::IdentityMismatch);
        }
    }
    let episodes = array(&value["_embedded"], "episodes")?;
    if episodes.len() > MAX_EPISODES {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    let mut seen = HashSet::new();
    for row in episodes {
        // shortcut: map unnumbered specials only after the episode contract can represent them.
        let Some(number) = u32_value(&row["number"])? else { continue; };
        let season_number = u32_value(&row["season"])?.ok_or(ProviderFailure::ParseError)?;
        let season = seasons.get_mut(&season_number).ok_or(ProviderFailure::IdentityMismatch)?;
        if !seen.insert((season_number, number)) {
            return Err(ProviderFailure::IdentityMismatch);
        }
        let id = required_id(&row["id"])?;
        numeric_id(&id)?;
        season.episodes.push(EpisodeDetail { remote_id: id, season_number, episode_number: number, title: row["name"].as_str().unwrap_or_default().into(), overview: summary(&row["summary"])?, order: EpisodeOrder::Aired });
    }
    for season in seasons.values_mut() {
        season.episodes.sort_by_key(|episode| episode.episode_number);
    }
    Ok(VideoMetadata::Tv(TvDetail { identity, fields, artwork, seasons: seasons.into_values().collect(), order: EpisodeOrder::Aired }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show() -> Value {
        json!({"id":1,"name":"Fixture TV","premiered":"2024-01-02","externals":{"imdb":"tt12345","thetvdb":42},"genres":["Drama"],"summary":"<p>Fixture <b>summary</b>.</p>","image":{"original":"https://static.tvmaze.com/uploads/images/original_untouched/1/1.jpg"},"_embedded":{"episodes":[{"id":10,"season":1,"number":2,"name":"Second"},{"id":11,"season":1,"number":1,"name":"First"},{"id":12,"season":1,"number":null,"name":"Unnumbered"}]}})
    }

    #[test]
    fn tvmaze_maps_real_airs_identity_and_preserves_license_without_html() {
        let VideoMetadata::Tv(detail) = metadata(&show(), &json!([{"id":5,"number":1,"name":""}]), "1").unwrap() else { panic!("expected TV"); };
        assert_eq!(detail.identity.language, "en-US");
        assert_eq!(detail.seasons[0].remote_id, "5");
        assert_eq!(detail.seasons[0].episodes.iter().map(|episode| episode.episode_number).collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(detail.fields.iter().find(|field| field.key == "overview").unwrap().value, json!("Fixture summary."));
        assert!(detail.fields.iter().any(|field| field.key == "source_license"));
        assert_eq!(detail.artwork[0].source_provider, "tvmaze");
        assert!(metadata(&show(), &json!([]), "2").is_err());
        let mut unsafe_image = show();
        unsafe_image["image"]["original"] = json!("https://example.com/arbitrary.jpg");
        assert_eq!(metadata(&unsafe_image, &json!([]), "1"), Err(ProviderFailure::UnsafeTarget));
    }
}
