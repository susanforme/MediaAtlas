//! Built-in ordinary video adapters. Every HTTP operation uses the P3 engine.
//! Fixtures exercise these adapters with injected test-only loopback endpoints.
use crate::{
    credentials::{CredentialMode, CredentialStore},
    network::{parse_html, CancellationToken, EngineKind, NetworkEngine, RequestSpec},
};
use mediaatlas_domain::*;
use reqwest::Url;
use serde_json::{json, Value};
use std::sync::Arc;

pub const TMDB_CREDENTIAL_REF: &str = "tmdb.access_token";
const MAX_RESULTS: usize = 50;
const MAX_SEASONS: usize = 40;
const MAX_EPISODES: usize = 1500;
const ANILIST_FIELDS: &str = "id idMal type isAdult title { romaji english native } synonyms startDate { year } description(asHtml:false) format season episodes duration siteUrl coverImage { large } bannerImage studios(isMain:true) { nodes { name } }";

#[derive(Clone)]
pub struct VideoProviderHub {
    engine: Arc<NetworkEngine>,
    credentials: Arc<CredentialStore>,
    credential_mode: CredentialMode,
    auth: Option<crate::provider_auth::ProviderAuthService>,
    tmdb_base: String,
    anilist_endpoint: String,
    wikipedia_base: String,
}
impl VideoProviderHub {
    pub fn new(engine: Arc<NetworkEngine>, credentials: Arc<CredentialStore>) -> Self {
        Self {
            engine,
            credentials,
            credential_mode: CredentialMode::System,
            auth: None,
            tmdb_base: "https://api.themoviedb.org/3".into(),
            anilist_endpoint: "https://graphql.anilist.co".into(),
            wikipedia_base: "https://en.wikipedia.org/wiki".into(),
        }
    }
    /// Trusted composition only. Session mode never falls back to disk or OS storage.
    pub fn with_auth_service(mut self, auth: crate::provider_auth::ProviderAuthService) -> Self {
        self.auth = Some(auth);
        self
    }
    pub fn with_credential_mode(mut self, mode: CredentialMode) -> Self {
        self.credential_mode = mode;
        self
    }
    #[cfg(test)]
    pub(crate) fn with_mock_endpoints(
        mut self,
        tmdb_base: &str,
        anilist_endpoint: &str,
        wikipedia_base: &str,
    ) -> Self {
        self.tmdb_base = tmdb_base.into();
        self.anilist_endpoint = anilist_endpoint.into();
        self.wikipedia_base = wikipedia_base.into();
        self
    }
    pub fn descriptors() -> Vec<ProviderDescriptor> {
        vec![
            descriptor(
                "tmdb",
                "TMDB",
                vec![MediaKind::Movie, MediaKind::TvSeries],
                "https://www.themoviedb.org",
                "https://www.themoviedb.org/terms-of-use",
                true,
                false,
            ),
            descriptor(
                "anilist",
                "AniList",
                vec![MediaKind::Anime],
                "https://anilist.co",
                "https://docs.anilist.co/terms-of-use",
                false,
                false,
            ),
            descriptor(
                "wikipedia.html",
                "Wikipedia article (HTML supplement)",
                vec![MediaKind::Movie, MediaKind::TvSeries, MediaKind::Anime],
                "https://en.wikipedia.org",
                "https://foundation.wikimedia.org/wiki/Policy:Terms_of_Use",
                false,
                true,
            ),
        ]
    }
    pub async fn search(
        &self,
        provider: &str,
        query: &VideoQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        validate_query(query)?;
        if provider != job.provider_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        match provider {
            "tmdb" => self.tmdb_search(query, job, cancel).await,
            "anilist" if matches!(query, VideoQuery::Anime { .. }) => {
                self.anilist_search(query, job, cancel).await
            }
            "wikipedia.html" => self.wikipedia_search(query, job, cancel).await,
            _ => Err(ProviderFailure::InvalidConfiguration),
        }
    }
    pub async fn detail(
        &self,
        candidate: &VideoCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        if candidate.data().provider_id != job.provider_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        match candidate.data().provider_id.as_str() {
            "tmdb" => self.tmdb_detail(candidate, job, cancel).await,
            "anilist" if matches!(candidate, VideoCandidate::Anime(_)) => {
                self.anilist_detail(candidate, job, cancel).await
            }
            "wikipedia.html" => self.wikipedia_detail(candidate, job, cancel).await,
            _ => Err(ProviderFailure::InvalidConfiguration),
        }
    }
    async fn tmdb_json(
        &self,
        path: &str,
        language: &str,
        parameters: &[(&str, &str)],
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Value, ProviderFailure> {
        let mut url = Url::parse(&format!("{}{path}", self.tmdb_base))
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.query_pairs_mut().append_pair("language", language);
        for (key, value) in parameters {
            url.query_pairs_mut().append_pair(key, value);
        }
        if let Some(auth) = &self.auth {
            let resolved = auth.resolve(job.config.credential_ref.as_deref()).await?;
            let mut spec = RequestSpec::get(url.as_str(), EngineKind::Api);
            match resolved.method {
                "bearer" => spec.bearer = Some(resolved.secret),
                "api_key" => spec.secret_query = Some(("api_key".into(), resolved.secret)),
                _ => return Err(ProviderFailure::InvalidConfiguration),
            }
            return self.engine.execute(job, spec, cancel).await?.json();
        }
        if job
            .config
            .credential_ref
            .as_deref()
            .is_some_and(|r| r != TMDB_CREDENTIAL_REF)
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let mut credential_job = job.clone();
        credential_job.config.credential_ref = Some(TMDB_CREDENTIAL_REF.into());
        self.engine
            .execute_with_store(
                &credential_job,
                RequestSpec::get(url.as_str(), EngineKind::Api),
                cancel,
                &self.credentials,
                self.credential_mode,
            )
            .await?
            .json()
    }
    async fn tmdb_search(
        &self,
        query: &VideoQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        let (title, _, ids, language) = query.parts();
        let movie = matches!(query, VideoQuery::Movie { .. });
        if matches!(query, VideoQuery::Anime { .. }) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if let Some(id) = ids.iter().find(|i| i.namespace == "tmdb") {
            numeric_id(&id.value)?;
            let data = self
                .tmdb_json(
                    &format!("/{}/{}", if movie { "movie" } else { "tv" }, id.value),
                    language,
                    &[],
                    job,
                    cancel,
                )
                .await?;
            let candidate = tmdb_candidate(&data, movie, language)?;
            if candidate.data().remote_id != id.value {
                return Err(ProviderFailure::IdentityMismatch);
            }
            return Ok(vec![candidate]);
        }
        let data = if let Some(id) = ids
            .iter()
            .find(|i| matches!(i.namespace.as_str(), "imdb" | "tvdb"))
        {
            if !id.value.bytes().all(|b| b.is_ascii_alphanumeric()) || id.value.len() > 40 {
                return Err(ProviderFailure::InvalidConfiguration);
            }
            let source = if id.namespace == "imdb" {
                "imdb_id"
            } else {
                "tvdb_id"
            };
            self.tmdb_json(
                &format!("/find/{}", id.value),
                language,
                &[("external_source", source)],
                job,
                cancel,
            )
            .await?
        } else {
            self.tmdb_json(
                &format!("/search/{}", if movie { "movie" } else { "tv" }),
                language,
                &[("query", title), ("include_adult", "false"), ("page", "1")],
                job,
                cancel,
            )
            .await?
        };
        let key = if data.get("results").is_some() {
            "results"
        } else if movie {
            "movie_results"
        } else {
            "tv_results"
        };
        let rows = array(&data, key)?;
        if rows.len() > MAX_RESULTS {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        rows.iter()
            .filter(|v| v["adult"].as_bool() != Some(true))
            .map(|v| tmdb_candidate(v, movie, language))
            .collect()
    }
    async fn tmdb_detail(
        &self,
        candidate: &VideoCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        let movie = matches!(candidate, VideoCandidate::Movie(_));
        if matches!(candidate, VideoCandidate::Anime(_)) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let input = candidate.data();
        numeric_id(&input.remote_id)?;
        let kind = if movie { "movie" } else { "tv" };
        let data = self
            .tmdb_json(
                &format!("/{kind}/{}", input.remote_id),
                &input.language,
                &[
                    ("append_to_response", "images,external_ids,credits"),
                    ("include_image_language", "null,en,zh"),
                ],
                job,
                cancel,
            )
            .await?;
        let mut identity = tmdb_candidate(&data, movie, &input.language)?
            .data()
            .clone();
        if identity.remote_id != input.remote_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        for (key, namespace) in [
            ("imdb_id", "imdb"),
            ("tvdb_id", "tvdb"),
            ("wikidata_id", "wikidata"),
        ] {
            if let Some(value) = scalar_id(&data["external_ids"][key]) {
                identity.external_ids.push(RemoteId {
                    namespace: namespace.into(),
                    value,
                });
            }
        }
        let mut fields = standard_fields(&identity, data["overview"].as_str());
        fields.push(field(
            "genres",
            json!(array(&data, "genres")?
                .iter()
                .filter_map(|g| g["name"].as_str())
                .collect::<Vec<_>>()),
            Some(&input.language),
        ));
        if let Some(rating) = data["vote_average"].as_f64() {
            fields.push(field("rating", json!(rating), None));
        }
        let companies = data["production_companies"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v["name"].as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        fields.push(field("production_companies", json!(companies), None));
        if let Some(credits) = data["credits"]["cast"].as_array() {
            if credits.len() > 1000 {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            fields.push(field(
                "cast",
                json!(credits
                    .iter()
                    .filter_map(|p| p["name"].as_str())
                    .collect::<Vec<_>>()),
                None,
            ));
        }
        let artwork = tmdb_artwork(&data["images"])?;
        if movie {
            let runtime = u32_value(&data["runtime"])?;
            if let Some(runtime) = runtime {
                fields.push(field("runtime_minutes", json!(runtime), None));
            }
            Ok(VideoMetadata::Movie(MovieDetail {
                identity,
                fields,
                artwork,
                runtime_minutes: runtime,
            }))
        } else {
            let source_seasons = array(&data, "seasons")?;
            if source_seasons.len() > MAX_SEASONS {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            let mut seasons = Vec::new();
            let mut count = 0;
            for season in source_seasons {
                let number =
                    u32_value(&season["season_number"])?.ok_or(ProviderFailure::ParseError)?;
                let value = self
                    .tmdb_json(
                        &format!("/tv/{}/season/{number}", input.remote_id),
                        &input.language,
                        &[],
                        job,
                        cancel,
                    )
                    .await?;
                if value["season_number"].as_u64() != Some(u64::from(number))
                    || required_id(&season["id"])? != required_id(&value["id"])?
                {
                    return Err(ProviderFailure::IdentityMismatch);
                }
                let episode_values = array(&value, "episodes")?;
                count += episode_values.len();
                if count > MAX_EPISODES {
                    return Err(ProviderFailure::ResponseTooLarge);
                }
                let episodes = episode_values
                    .iter()
                    .map(|v| parse_episode(v, number, None))
                    .collect::<Result<Vec<_>, _>>()?;
                seasons.push(SeasonDetail {
                    remote_id: required_id(&value["id"])?,
                    season_number: number,
                    title: required_str(&value, "name")?.into(),
                    episodes,
                });
            }
            Ok(VideoMetadata::Tv(TvDetail {
                identity,
                fields,
                artwork,
                seasons,
                order: EpisodeOrder::Aired,
            }))
        }
    }
    /// Explicit per-episode endpoint. Aired identity is checked; no DVD/absolute guess.
    pub async fn tv_episode(
        &self,
        series_id: &str,
        season: u32,
        episode: u32,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<EpisodeDetail, ProviderFailure> {
        numeric_id(series_id)?;
        if job.provider_id != "tmdb" || season > 10000 || episode > 10000 {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let value = self
            .tmdb_json(
                &format!("/tv/{series_id}/season/{season}/episode/{episode}"),
                &job.language,
                &[],
                job,
                cancel,
            )
            .await?;
        parse_episode(&value, season, Some(episode))
    }
    async fn anilist_request(
        &self,
        query: &str,
        variables: Value,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Value, ProviderFailure> {
        let spec = RequestSpec::graphql(&self.anilist_endpoint, query, variables)?;
        let result = self.engine.execute(job, spec, cancel).await?.json()?;
        if result
            .get("errors")
            .is_some_and(|e| e.as_array().is_none_or(|a| !a.is_empty()))
        {
            return Err(ProviderFailure::ParseError);
        }
        result
            .get("data")
            .filter(|v| v.is_object())
            .cloned()
            .ok_or(ProviderFailure::ParseError)
    }
    async fn anilist_search(
        &self,
        query: &VideoQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        let (title, year, ids, language) = query.parts();
        if let Some(id) = ids
            .iter()
            .find(|i| matches!(i.namespace.as_str(), "anilist" | "mal"))
        {
            numeric_id(&id.value)?;
            let parameter = if id.namespace == "mal" { "idMal" } else { "id" };
            let query=format!("query ($id:Int!) {{ Media({parameter}:$id,type:ANIME,isAdult:false) {{ {ANILIST_FIELDS} }} }}");
            let value=self.anilist_request(&query,json!({"id":id.value.parse::<u32>().map_err(|_|ProviderFailure::InvalidConfiguration)?}),job,cancel).await?;
            let identity = anilist_identity(&value["Media"], language)?;
            let returned = if id.namespace == "mal" {
                scalar_id(&value["Media"]["idMal"])
            } else {
                Some(identity.remote_id.clone())
            };
            if returned.as_deref() != Some(id.value.as_str()) {
                return Err(ProviderFailure::IdentityMismatch);
            }
            return Ok(vec![VideoCandidate::Anime(identity)]);
        }
        // Year remains a matching signal instead of suppressing remakes from results.
        let _ = year;
        let query=format!("query ($search:String!) {{ Page(page:1,perPage:25) {{ media(search:$search,type:ANIME,isAdult:false) {{ {ANILIST_FIELDS} }} }} }}");
        let value = self
            .anilist_request(&query, json!({"search":title}), job, cancel)
            .await?;
        let rows = array(&value["Page"], "media")?;
        if rows.len() > MAX_RESULTS {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        rows.iter()
            .map(|v| Ok(VideoCandidate::Anime(anilist_identity(v, language)?)))
            .collect()
    }
    async fn anilist_detail(
        &self,
        candidate: &VideoCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        let input = candidate.data();
        numeric_id(&input.remote_id)?;
        let query = format!(
            "query ($id:Int!) {{ Media(id:$id,type:ANIME,isAdult:false) {{ {ANILIST_FIELDS} }} }}"
        );
        let result=self.anilist_request(&query,json!({"id":input.remote_id.parse::<u32>().map_err(|_|ProviderFailure::InvalidConfiguration)?}),job,cancel).await?;
        let value = &result["Media"];
        let identity = anilist_identity(value, &input.language)?;
        if identity.remote_id != input.remote_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        let mut fields = standard_fields(&identity, None);
        if let Some(description) = value["description"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
        {
            fields.push(field("overview", json!(description), Some("en")));
        }
        fields.push(field("aliases", json!(identity.aliases), None));
        for key in ["format", "season", "episodes", "duration"] {
            if !value[key].is_null() {
                fields.push(field(key, value[key].clone(), None));
            }
        }
        if let Some(studios) = value["studios"]["nodes"].as_array() {
            fields.push(field(
                "studios",
                json!(studios
                    .iter()
                    .filter_map(|s| s["name"].as_str())
                    .collect::<Vec<_>>()),
                None,
            ));
        }
        let mut artwork = Vec::new();
        for (url, kind) in [
            (value["coverImage"]["large"].as_str(), "poster"),
            (value["bannerImage"].as_str(), "backdrop"),
        ] {
            if let Some(url) = url {
                validate_image_url(url, &["s4.anilist.co"])?;
                artwork.push(ArtworkCandidate {
                    url: url.into(),
                    source_provider: "anilist".into(),
                    kind: kind.into(),
                    language: None,
                });
            }
        }
        Ok(VideoMetadata::Anime(AnimeDetail {
            aliases: identity.aliases.clone(),
            identity,
            fields,
            artwork,
            format: value["format"].as_str().map(str::to_owned),
            season: value["season"].as_str().map(str::to_owned),
            episodes: u32_value(&value["episodes"])?,
        }))
    }
    async fn wikipedia_fetch(
        &self,
        title: &str,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<(String, String, String), ProviderFailure> {
        if title.is_empty() || title.len() > 256 || title.contains(['\0', '\n', '\r']) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let mut url =
            Url::parse(&self.wikipedia_base).map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.path_segments_mut()
            .map_err(|_| ProviderFailure::InvalidConfiguration)?
            .push(&title.replace(' ', "_"));
        let mut spec = RequestSpec::get(url.as_str(), EngineKind::Html);
        spec.user_agent=Some("MediaAtlas/0.1 (desktop metadata supplement; https://github.com/susanforme/MediaAtlas; manual article lookup)".into());
        // The access policy is a backend floor, independent of UI configuration.
        let mut effective_job = job.clone();
        effective_job.config.interval_ms = effective_job.config.interval_ms.max(1000);
        effective_job.config.concurrency = 1;
        let response = self.engine.execute(&effective_job, spec, cancel).await?;
        let html = parse_html(response.text()?);
        let heading = html
            .css_text("#firstHeading")?
            .into_iter()
            .next()
            .filter(|s| !s.is_empty())
            .ok_or(ProviderFailure::ParseError)?;
        // Disambiguation/search/login pages are not metadata.
        if !html.css_text(".mw-disambig")?.is_empty() || !html.css_text("#disambigbox")?.is_empty()
        {
            return Err(ProviderFailure::IdentityMismatch);
        }
        let description = html
            .css_text(".mw-parser-output > p")?
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .or_else(|| html.open_graph().get("og:description").cloned())
            .unwrap_or_default();
        let source = format!(
            "https://en.wikipedia.org/wiki/{}",
            Url::parse(url.as_str())
                .map_err(|_| ProviderFailure::ParseError)?
                .path_segments()
                .and_then(|mut p| p.next_back())
                .ok_or(ProviderFailure::ParseError)?
        );
        Ok((heading, description, source))
    }
    async fn wikipedia_search(
        &self,
        query: &VideoQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        let (title, _, ids, _language) = query.parts();
        let language = "en-US";
        let article = ids
            .iter()
            .find(|i| i.namespace == "wikipedia.en")
            .map(|i| i.value.as_str())
            .unwrap_or(title);
        let (heading, _, source) = self.wikipedia_fetch(article, job, cancel).await?;
        let identity = CandidateData {
            provider_id: "wikipedia.html".into(),
            remote_id: article.into(),
            title: heading,
            original_title: None,
            year: None,
            aliases: vec![],
            external_ids: vec![RemoteId {
                namespace: "wikipedia.en".into(),
                value: article.into(),
            }],
            source_url: source,
            language: language.into(),
        };
        Ok(vec![match query {
            VideoQuery::Movie { .. } => VideoCandidate::Movie(identity),
            VideoQuery::Tv { .. } => VideoCandidate::Tv(identity),
            VideoQuery::Anime { .. } => VideoCandidate::Anime(identity),
        }])
    }
    async fn wikipedia_detail(
        &self,
        candidate: &VideoCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        let input = candidate.data();
        let (heading, description, source) =
            self.wikipedia_fetch(&input.remote_id, job, cancel).await?;
        if heading != input.title {
            return Err(ProviderFailure::IdentityMismatch);
        }
        let mut identity = input.clone();
        identity.source_url = source;
        let mut fields = standard_fields(&identity, Some(&description));
        fields.push(field(
            "attribution",
            json!(format!("Wikipedia contributors; {}", identity.source_url)),
            None,
        ));
        fields.push(field(
            "source_license",
            json!("CC BY-SA 4.0; https://creativecommons.org/licenses/by-sa/4.0/"),
            None,
        ));
        Ok(match candidate {
            VideoCandidate::Movie(_) => VideoMetadata::Movie(MovieDetail {
                identity,
                fields,
                artwork: vec![],
                runtime_minutes: None,
            }),
            VideoCandidate::Tv(_) => VideoMetadata::Tv(TvDetail {
                identity,
                fields,
                artwork: vec![],
                seasons: vec![],
                order: EpisodeOrder::Aired,
            }),
            VideoCandidate::Anime(_) => VideoMetadata::Anime(AnimeDetail {
                identity,
                fields,
                artwork: vec![],
                format: None,
                season: None,
                episodes: None,
                aliases: vec![],
            }),
        })
    }
}
fn validate_query(query: &VideoQuery) -> Result<(), ProviderFailure> {
    let (title, year, ids, language) = query.parts();
    if title.len() > 256
        || title.chars().any(char::is_control)
        || title.trim().is_empty() && ids.is_empty()
        || year.is_some_and(|y| !(1800..=2200).contains(&y))
        || ids.len() > 12
        || normalize_locale(Some(language)).is_err()
        || ids.iter().any(|i| {
            i.value.is_empty() || i.value.len() > 256 || i.value.chars().any(char::is_control)
        })
    {
        Err(ProviderFailure::InvalidConfiguration)
    } else {
        Ok(())
    }
}
fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, ProviderFailure> {
    value[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or(ProviderFailure::ParseError)
}
fn scalar_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .or_else(|| value.as_u64().map(|v| v.to_string()))
}
fn required_id(value: &Value) -> Result<String, ProviderFailure> {
    scalar_id(value).ok_or(ProviderFailure::ParseError)
}
fn numeric_id(value: &str) -> Result<(), ProviderFailure> {
    if value.is_empty()
        || value.len() > 12
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.parse::<u64>().ok().filter(|n| *n > 0).is_none()
    {
        Err(ProviderFailure::InvalidConfiguration)
    } else {
        Ok(())
    }
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, ProviderFailure> {
    value[key].as_array().ok_or(ProviderFailure::ParseError)
}
fn u32_value(value: &Value) -> Result<Option<u32>, ProviderFailure> {
    if value.is_null() {
        Ok(None)
    } else {
        value
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or(ProviderFailure::ParseError)
    }
}
fn tmdb_candidate(
    value: &Value,
    movie: bool,
    language: &str,
) -> Result<VideoCandidate, ProviderFailure> {
    let id = required_id(&value["id"])?;
    numeric_id(&id)?;
    if value["adult"].as_bool() == Some(true) {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let date = value[if movie {
        "release_date"
    } else {
        "first_air_date"
    }]
    .as_str()
    .unwrap_or_default();
    let year = if date.is_empty() {
        None
    } else {
        Some(
            date.get(..4)
                .ok_or(ProviderFailure::ParseError)?
                .parse()
                .map_err(|_| ProviderFailure::ParseError)?,
        )
    };
    let data = CandidateData {
        provider_id: "tmdb".into(),
        remote_id: id.clone(),
        title: required_str(value, if movie { "title" } else { "name" })?.into(),
        original_title: value[if movie {
            "original_title"
        } else {
            "original_name"
        }]
        .as_str()
        .map(str::to_owned),
        year,
        aliases: vec![],
        external_ids: vec![RemoteId {
            namespace: "tmdb".into(),
            value: id.clone(),
        }],
        source_url: format!(
            "https://www.themoviedb.org/{}/{id}",
            if movie { "movie" } else { "tv" }
        ),
        language: language.into(),
    };
    Ok(if movie {
        VideoCandidate::Movie(data)
    } else {
        VideoCandidate::Tv(data)
    })
}
fn standard_fields(identity: &CandidateData, overview: Option<&str>) -> Vec<RemoteField> {
    let mut fields = vec![field(
        "title",
        json!(identity.title),
        Some(&identity.language),
    )];
    if let Some(title) = &identity.original_title {
        fields.push(field("original_title", json!(title), None));
    }
    if let Some(year) = identity.year {
        fields.push(field("year", json!(year), None));
    }
    if let Some(overview) = overview.filter(|s| !s.trim().is_empty()) {
        fields.push(field("overview", json!(overview), Some(&identity.language)));
    }
    fields
}
fn field(key: &str, value: Value, language: Option<&str>) -> RemoteField {
    RemoteField {
        key: key.into(),
        value,
        language: language.map(str::to_owned),
    }
}
fn tmdb_artwork(images: &Value) -> Result<Vec<ArtworkCandidate>, ProviderFailure> {
    let mut result = Vec::new();
    for (key, kind) in [("posters", "poster"), ("backdrops", "backdrop")] {
        if let Some(values) = images[key].as_array() {
            if values.len() > 500 {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            for value in values {
                let path = required_str(value, "file_path")?;
                if !path.starts_with('/') || path.contains("..") || path.contains(['?', '#', '\\'])
                {
                    return Err(ProviderFailure::UnsafeTarget);
                }
                let url = format!("https://image.tmdb.org/t/p/original{path}");
                validate_image_url(&url, &["image.tmdb.org"])?;
                result.push(ArtworkCandidate {
                    url,
                    source_provider: "tmdb".into(),
                    kind: kind.into(),
                    language: value["iso_639_1"].as_str().map(str::to_owned),
                });
            }
        }
    }
    Ok(result)
}
fn validate_image_url(url: &str, hosts: &[&str]) -> Result<(), ProviderFailure> {
    let url = Url::parse(url).map_err(|_| ProviderFailure::UnsafeTarget)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !hosts.contains(&url.host_str().unwrap_or_default())
        || url.port().is_some_and(|p| p != 443)
    {
        Err(ProviderFailure::UnsafeTarget)
    } else {
        Ok(())
    }
}
fn parse_episode(
    value: &Value,
    season: u32,
    expected: Option<u32>,
) -> Result<EpisodeDetail, ProviderFailure> {
    let number = u32_value(&value["episode_number"])?.ok_or(ProviderFailure::ParseError)?;
    if value["season_number"].as_u64() != Some(u64::from(season))
        || expected.is_some_and(|n| n != number)
    {
        return Err(ProviderFailure::IdentityMismatch);
    }
    Ok(EpisodeDetail {
        remote_id: required_id(&value["id"])?,
        season_number: season,
        episode_number: number,
        title: required_str(value, "name")?.into(),
        overview: value["overview"].as_str().map(str::to_owned),
        order: EpisodeOrder::Aired,
    })
}
fn anilist_identity(value: &Value, language: &str) -> Result<CandidateData, ProviderFailure> {
    let id = required_id(&value["id"])?;
    numeric_id(&id)?;
    if value["isAdult"].as_bool() != Some(false) || value["type"].as_str() != Some("ANIME") {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let actual_language =
        if language.starts_with("en") && value["title"]["english"].as_str().is_some() {
            "en-US"
        } else if language.starts_with("ja") && value["title"]["native"].as_str().is_some() {
            "ja"
        } else if value["title"]["romaji"].as_str().is_some() {
            "ja-Latn"
        } else {
            "ja"
        };
    let title = if language.starts_with("en") {
        value["title"]["english"].as_str()
    } else if language.starts_with("ja") {
        value["title"]["native"].as_str()
    } else {
        None
    }
    .or_else(|| value["title"]["romaji"].as_str())
    .or_else(|| value["title"]["native"].as_str())
    .filter(|s| !s.trim().is_empty())
    .ok_or(ProviderFailure::ParseError)?;
    let mut aliases = Vec::new();
    for key in ["english", "native", "romaji"] {
        if let Some(alias) = value["title"][key].as_str().filter(|s| !s.is_empty()) {
            if !aliases.iter().any(|s| s == alias) {
                aliases.push(alias.to_owned());
            }
        }
    }
    if let Some(names) = value["synonyms"].as_array() {
        if names.len() > 200 {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        for alias in names.iter().filter_map(Value::as_str) {
            if !aliases.iter().any(|s| s == alias) {
                aliases.push(alias.to_owned());
            }
        }
    }
    let mut ids = vec![RemoteId {
        namespace: "anilist".into(),
        value: id.clone(),
    }];
    if let Some(mal) = scalar_id(&value["idMal"]) {
        ids.push(RemoteId {
            namespace: "mal".into(),
            value: mal,
        });
    }
    Ok(CandidateData {
        provider_id: "anilist".into(),
        remote_id: id.clone(),
        title: title.into(),
        original_title: value["title"]["native"].as_str().map(str::to_owned),
        year: value["startDate"]["year"]
            .as_i64()
            .and_then(|v| i32::try_from(v).ok()),
        aliases,
        external_ids: ids,
        source_url: format!("https://anilist.co/anime/{id}"),
        language: actual_language.into(),
    })
}
fn descriptor(
    id: &str,
    name: &str,
    kinds: Vec<MediaKind>,
    website: &str,
    terms: &str,
    key: bool,
    html: bool,
) -> ProviderDescriptor {
    ProviderDescriptor{id:id.into(),name:name.into(),description:if html{"Explicit English article lookup; supplemental text only, manual selection required."}else{"Built-in live API adapter; live availability requires separate manual acceptance."}.into(),website:Some(website.into()),terms_url:Some(terms.into()),privacy_url:Some(if id=="tmdb"{"https://www.themoviedb.org/privacy-policy"}else if id=="anilist"{"https://anilist.co/terms"}else{"https://foundation.wikimedia.org/wiki/Policy:Privacy_policy"}.into()),license:if html{"CC BY-SA 4.0 text; preserve attribution and license; no image rights inferred"}else{"Provider terms apply; no blanket downstream redistribution license"}.into(),media_kinds:kinds,languages:if html{vec!["en-US".into()]}else{vec!["zh-CN".into(),"en-US".into()]},capabilities:if html{vec!["article_lookup".into(),"details".into()]}else{vec!["search".into(),"details".into(),"images".into(),"external_ids".into()]},fixture:false,documentation_url:Some("https://github.com/susanforme/MediaAtlas/blob/main/docs/provider-access.md".into()),engine_types:vec![if html{"html"}else{"api"}.into()],requires_api_key:key,requires_login:false,rate_limit_policy:if html{"Manual lookup only; enforced >= 1 s between requests and concurrency 1; honor 403/429/CAPTCHA; no bulk crawler"}else if id=="anilist"{"Honor API rate limit and Retry-After; conservative 1 s request interval"}else{"Honor TMDB rate limits and Retry-After; no concurrent unbounded season crawl"}.into(),attribution:if id=="tmdb"{"This product uses the TMDB API but is not endorsed or certified by TMDB."}else if html{"Wikipedia contributors; source article URL and CC BY-SA attribution must accompany exported text."}else{"Metadata supplied by AniList; https://anilist.co"}.into(),data_usage_notes:"Default disabled. Fixture success does not verify live connectivity, account authorization or downstream use rights.".into()}
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::Mutex;
    use uuid::Uuid;

    type Requests = Arc<Mutex<Vec<String>>>;
    async fn server() -> (String, Requests, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let saved = requests.clone();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let saved = saved.clone();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    loop {
                        let mut chunk = [0; 4096];
                        let length = stream.read(&mut chunk).await.unwrap();
                        if length == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&chunk[..length]);
                        assert!(bytes.len() <= 16384);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let headers = String::from_utf8_lossy(&bytes[..end]);
                            let body_length = headers
                                .lines()
                                .find_map(|line| {
                                    line.split_once(':')
                                        .filter(|(key, _)| {
                                            key.eq_ignore_ascii_case("content-length")
                                        })
                                        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if bytes.len() >= end + 4 + body_length {
                                break;
                            }
                        }
                    }
                    let request = String::from_utf8_lossy(&bytes).into_owned();
                    let line = request.lines().next().unwrap_or_default().to_owned();
                    let path = line.split_whitespace().nth(1).unwrap_or_default();
                    saved.lock().await.push(request.clone());
                    let (mime, body) = if path.starts_with("/wiki/") {
                        ("text/html", r#"<html><h1 id="firstHeading">Fixture Film</h1><div class="mw-parser-output"><p>Synthetic film summary.</p></div></html>"#.to_owned())
                    } else if path == "/graphql" {
                        let media = json!({"id":7,"idMal":70,"isAdult":false,"type":"ANIME","title":{"romaji":"Fixture Anime","english":"Fixture Animation","native":"架空"},"synonyms":["Alias"],"startDate":{"year":2024},"description":"Synthetic anime summary","format":"TV","season":"SPRING","episodes":12,"duration":24,"coverImage":{"large":"https://s4.anilist.co/file/fixture.jpg"},"bannerImage":null,"studios":{"nodes":[{"name":"Fixture Studio"}]}});
                        let value = if request.contains("Page(") {
                            json!({"data":{"Page":{"media":[media]}}})
                        } else {
                            json!({"data":{"Media":media}})
                        };
                        ("application/json", value.to_string())
                    } else if path.starts_with("/3/search/movie") {
                        ("application/json",json!({"results":[{"id":1,"title":"Fixture Film","original_title":"Fixture Original","release_date":"2024-01-01","adult":false},{"id":2,"title":"Fixture Film","release_date":"1990-01-01","adult":false}]}).to_string())
                    } else if path.starts_with("/3/find/") {
                        ("application/json",json!({"movie_results":[{"id":1,"title":"Fixture Film","release_date":"2024-01-01"}]}).to_string())
                    } else if path.starts_with("/3/search/tv") {
                        ("application/json",json!({"results":[{"id":10,"name":"Fixture Series","first_air_date":"2020-02-01"}]}).to_string())
                    } else if path.starts_with("/3/movie/") {
                        ("application/json",json!({"id":1,"title":"Fixture Film","original_title":"Fixture Original","release_date":"2024-01-01","overview":"Synthetic overview","runtime":110,"genres":[{"name":"Drama"}],"production_companies":[{"name":"Studio"}],"credits":{"cast":[{"name":"Actor"}]},"images":{"posters":[{"file_path":"/fixture.jpg","iso_639_1":"en"}],"backdrops":[]},"external_ids":{"imdb_id":"tt123","wikidata_id":"Q123"}}).to_string())
                    } else if path.starts_with("/3/tv/10/season/1/episode/1") {
                        ("application/json",json!({"id":101,"season_number":1,"episode_number":1,"name":"Pilot","overview":"Episode summary"}).to_string())
                    } else if path.starts_with("/3/tv/10/season/1") {
                        ("application/json",json!({"id":if path.contains("xx-XX"){999}else{100},"season_number":1,"name":"Season 1","episodes":[{"id":101,"season_number":1,"episode_number":1,"name":"Pilot","overview":"Episode summary"}]}).to_string())
                    } else if path.starts_with("/3/tv/10") {
                        ("application/json",json!({"id":10,"name":"Fixture Series","first_air_date":"2020-02-01","overview":"Series summary","genres":[],"images":{"posters":[],"backdrops":[]},"external_ids":{"tvdb_id":1000},"seasons":[{"id":100,"season_number":1}]}).to_string())
                    } else {
                        ("application/json", "{}".to_owned())
                    };
                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        (format!("http://{address}"), requests, handle)
    }
    fn job(provider: &str) -> ProviderJob {
        ProviderJob {
            id: Uuid::new_v4(),
            provider_id: provider.into(),
            media_item_id: None,
            media_kind: None,
            file_identity: None,
            region: None,
            correlation_id: Uuid::new_v4(),
            operation: "search".into(),
            query: "Fixture".into(),
            language: "en-US".into(),
            config: ProviderConfig {
                enabled: true,
                interval_ms: 0,
                max_retries: 0,
                ..Default::default()
            },
            created_at: Utc::now(),
        }
    }
    async fn setup() -> (VideoProviderHub, Requests, tokio::task::JoinHandle<()>) {
        let (base, requests, handle) = server().await;
        let credentials = Arc::new(CredentialStore::new());
        credentials
            .put(
                TMDB_CREDENTIAL_REF,
                crate::network::SecretString::new("mock-session-token".into()),
                CredentialMode::SessionOnly,
            )
            .await
            .unwrap();
        let mut hub = VideoProviderHub::new(Arc::new(NetworkEngine::for_loopback(4)), credentials)
            .with_credential_mode(CredentialMode::SessionOnly);
        hub = hub.with_mock_endpoints(
            &format!("{base}/3"),
            &format!("{base}/graphql"),
            &format!("{base}/wiki"),
        );
        (hub, requests, handle)
    }
    #[tokio::test]
    async fn mock_tmdb_movie_search_find_details_images_ids_use_bounded_transport() {
        let (hub, requests, handle) = setup().await;
        let job = job("tmdb");
        let cancel = CancellationToken::new();
        let query = VideoQuery::Movie {
            title: "Fixture Film".into(),
            year: Some(2024),
            external_ids: vec![],
            language: "en-US".into(),
        };
        let candidates = hub.search("tmdb", &query, &job, &cancel).await.unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[1].data().year, Some(1990));
        let metadata = hub.detail(&candidates[0], &job, &cancel).await.unwrap();
        let VideoMetadata::Movie(movie) = metadata else {
            panic!("movie expected")
        };
        assert_eq!(movie.runtime_minutes, Some(110));
        assert_eq!(
            movie.artwork[0].url,
            "https://image.tmdb.org/t/p/original/fixture.jpg"
        );
        assert!(movie.identity.external_ids.contains(&RemoteId {
            namespace: "imdb".into(),
            value: "tt123".into()
        }));
        let query = VideoQuery::Movie {
            title: String::new(),
            year: None,
            external_ids: vec![RemoteId {
                namespace: "imdb".into(),
                value: "tt123".into(),
            }],
            language: "en-US".into(),
        };
        assert_eq!(
            hub.search("tmdb", &query, &job, &cancel).await.unwrap()[0]
                .data()
                .remote_id,
            "1"
        );
        let query = VideoQuery::Movie {
            title: String::new(),
            year: None,
            external_ids: vec![RemoteId {
                namespace: "tmdb".into(),
                value: "2".into(),
            }],
            language: "en-US".into(),
        };
        assert_eq!(
            hub.search("tmdb", &query, &job, &cancel).await,
            Err(ProviderFailure::IdentityMismatch)
        );
        let raw = requests.lock().await.join("\n");
        assert!(raw.contains("/3/find/tt123"));
        assert!(raw
            .to_ascii_lowercase()
            .contains("authorization: bearer mock-session-token"));
        let logs = serde_json::to_string(&hub.engine.records().await).unwrap();
        assert!(!logs.contains("mock-session-token"));
        assert!(!logs.contains("Fixture Film"));
        handle.abort();
    }
    #[tokio::test]
    async fn mock_tmdb_tv_series_season_episode_keep_aired_identity() {
        let (hub, _, handle) = setup().await;
        let job = job("tmdb");
        let cancel = CancellationToken::new();
        let query = VideoQuery::Tv {
            title: "Fixture Series".into(),
            year: None,
            external_ids: vec![],
            language: "en-US".into(),
        };
        let candidates = hub.search("tmdb", &query, &job, &cancel).await.unwrap();
        let VideoMetadata::Tv(tv) = hub.detail(&candidates[0], &job, &cancel).await.unwrap() else {
            panic!("tv expected")
        };
        assert_eq!(tv.order, EpisodeOrder::Aired);
        assert_eq!(tv.seasons[0].episodes[0].episode_number, 1);
        assert_eq!(tv.identity.external_ids[1].namespace, "tvdb");
        let episode = hub.tv_episode("10", 1, 1, &job, &cancel).await.unwrap();
        assert_eq!(episode.remote_id, "101");
        let mut wrong = candidates[0].clone();
        if let VideoCandidate::Tv(identity) = &mut wrong {
            identity.language = "xx-XX".into();
        }
        assert_eq!(
            hub.detail(&wrong, &job, &cancel).await,
            Err(ProviderFailure::IdentityMismatch)
        );
        assert_eq!(
            hub.tv_episode("../10", 1, 1, &job, &cancel).await,
            Err(ProviderFailure::InvalidConfiguration)
        );
        handle.abort();
    }
    #[tokio::test]
    async fn mock_anilist_graphql_aliases_ids_actual_locale_and_wikipedia_html_supplement() {
        let (hub, requests, handle) = setup().await;
        let cancel = CancellationToken::new();
        let query = VideoQuery::Anime {
            title: "Fixture Anime".into(),
            year: Some(2024),
            external_ids: vec![],
            language: "zh-CN".into(),
        };
        let candidates = hub
            .search("anilist", &query, &job("anilist"), &cancel)
            .await
            .unwrap();
        assert_eq!(candidates[0].data().language, "ja-Latn");
        let VideoMetadata::Anime(anime) = hub
            .detail(&candidates[0], &job("anilist"), &cancel)
            .await
            .unwrap()
        else {
            panic!("anime expected")
        };
        assert_eq!(anime.episodes, Some(12));
        assert!(anime.aliases.contains(&"Alias".into()));
        assert_eq!(anime.identity.external_ids[1].namespace, "mal");
        assert_eq!(
            anime
                .fields
                .iter()
                .find(|f| f.key == "overview")
                .unwrap()
                .language
                .as_deref(),
            Some("en")
        );
        assert_eq!(anime.artwork[0].source_provider, "anilist");
        for (namespace, value) in [("anilist", "8"), ("mal", "71")] {
            let wrong = VideoQuery::Anime {
                title: String::new(),
                year: None,
                external_ids: vec![RemoteId {
                    namespace: namespace.into(),
                    value: value.into(),
                }],
                language: "en-US".into(),
            };
            assert_eq!(
                hub.search("anilist", &wrong, &job("anilist"), &cancel)
                    .await,
                Err(ProviderFailure::IdentityMismatch)
            );
        }
        assert_eq!(
            anilist_identity(&json!({"id":7,"type":"MANGA","isAdult":false}), "en-US"),
            Err(ProviderFailure::IdentityMismatch)
        );
        let query = VideoQuery::Movie {
            title: "Fixture Film".into(),
            year: None,
            external_ids: vec![],
            language: "zh-CN".into(),
        };
        let candidates = hub
            .search("wikipedia.html", &query, &job("wikipedia.html"), &cancel)
            .await
            .unwrap();
        assert_eq!(candidates[0].data().language, "en-US");
        let detail = hub
            .detail(&candidates[0], &job("wikipedia.html"), &cancel)
            .await
            .unwrap();
        assert!(detail.artwork().is_empty());
        assert!(detail.fields().iter().any(|f| f.key == "source_license"));
        let raw = requests.lock().await.join("\n");
        assert!(raw.contains("POST /graphql"));
        assert!(raw.contains("GET /wiki/Fixture_Film"));
        handle.abort();
    }
    #[tokio::test]
    async fn credentials_unavailable_and_identity_injection_fail_closed() {
        let (mut hub, requests, handle) = setup().await;
        hub.credentials = Arc::new(CredentialStore::new());
        let query = VideoQuery::Movie {
            title: "Fixture Film".into(),
            year: None,
            external_ids: vec![],
            language: "en-US".into(),
        };
        assert_eq!(
            hub.search("tmdb", &query, &job("tmdb"), &CancellationToken::new())
                .await,
            Err(ProviderFailure::CredentialsUnavailable)
        );
        assert!(requests.lock().await.is_empty());
        assert_eq!(
            hub.search("anilist", &query, &job("tmdb"), &CancellationToken::new())
                .await,
            Err(ProviderFailure::IdentityMismatch)
        );
        assert_eq!(
            tmdb_artwork(&json!({"posters":[{"file_path":"/../../secret"}]})),
            Err(ProviderFailure::UnsafeTarget)
        );
        assert_eq!(
            validate_image_url("http://127.0.0.1/x", &["s4.anilist.co"]),
            Err(ProviderFailure::UnsafeTarget)
        );
        assert_eq!(
            parse_episode(
                &json!({"id":1,"season_number":2,"episode_number":1,"name":"Bad"}),
                1,
                None
            ),
            Err(ProviderFailure::IdentityMismatch)
        );
        assert_eq!(
            anilist_identity(&json!({"id":1,"isAdult":true}), "en-US"),
            Err(ProviderFailure::IdentityMismatch)
        );
        handle.abort();
    }
    #[tokio::test]
    async fn real_mock_requests_follow_saved_auth_method_and_old_jobs_never_switch() {
        use crate::provider_auth::{AuthSaveInput, ProviderAuthService};
        let (hub, requests, handle) = setup().await;
        let dir = tempfile::tempdir().unwrap();
        let repo = crate::library::LibraryRepository::open(&dir.path().join("auth.sqlite"))
            .await
            .unwrap();
        let auth = ProviderAuthService::new(repo, hub.credentials.clone());
        auth.save(AuthSaveInput {
            provider_id: "tmdb".into(),
            method: "bearer".into(),
            storage: "session_only".into(),
            secret: "synthetic-new-bearer".into(),
        })
        .await
        .unwrap();
        let hub = hub.with_auth_service(auth.clone());
        let mut first_job = job("tmdb");
        first_job.config.credential_ref = auth.snapshot_ref("tmdb").await.unwrap();
        let query = VideoQuery::Movie {
            title: "Fixture Film".into(),
            year: None,
            external_ids: vec![],
            language: "en-US".into(),
        };
        let cancel = CancellationToken::new();
        hub.search("tmdb", &query, &first_job, &cancel)
            .await
            .unwrap();
        auth.save(AuthSaveInput {
            provider_id: "tmdb".into(),
            method: "api_key".into(),
            storage: "session_only".into(),
            secret: "synthetic-api-key".into(),
        })
        .await
        .unwrap();
        let mut second_job = job("tmdb");
        second_job.config.credential_ref = auth.snapshot_ref("tmdb").await.unwrap();
        hub.search("tmdb", &query, &second_job, &cancel)
            .await
            .unwrap();
        assert_eq!(
            hub.search("tmdb", &query, &first_job, &cancel).await,
            Err(ProviderFailure::CredentialsUnavailable)
        );
        let captured = requests.lock().await;
        assert_eq!(captured.len(), 2);
        assert!(captured[0]
            .to_ascii_lowercase()
            .contains("authorization: bearer synthetic-new-bearer"));
        assert!(!captured[0].contains("api_key="));
        assert!(captured[1]
            .lines()
            .next()
            .unwrap()
            .contains("api_key=synthetic-api-key"));
        assert!(!captured[1].to_ascii_lowercase().contains("authorization:"));
        let logs = serde_json::to_string(&hub.engine.records().await).unwrap();
        assert!(!logs.contains("synthetic-new-bearer"));
        assert!(!logs.contains("synthetic-api-key"));
        handle.abort();
    }
    #[test]
    fn real_descriptors_and_resource_caps_are_explicit() {
        let descriptors = VideoProviderHub::descriptors();
        assert_eq!(descriptors.len(), 3);
        assert!(descriptors
            .iter()
            .all(|d| !d.fixture && d.terms_url.is_some()));
        let large = Value::Array(vec![json!({"file_path":"/a.jpg"}); 501]);
        assert_eq!(
            tmdb_artwork(&json!({"posters":large})),
            Err(ProviderFailure::ResponseTooLarge)
        );
        assert_eq!(numeric_id("0"), Err(ProviderFailure::InvalidConfiguration));
    }
}
