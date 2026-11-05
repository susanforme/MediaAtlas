//! OMDb's documented API; IMDb identifiers do not authorize requests to arbitrary URLs.
use super::*;

const CREDENTIAL_REF: &str = "omdb.api_key";

pub(super) fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: "omdb".into(), name: "OMDb".into(),
        description: "Movie and television metadata with precise IMDb identity lookup.".into(),
        website: Some("https://www.omdbapi.com".into()),
        terms_url: Some("https://www.omdbapi.com/legal.htm".into()), privacy_url: None,
        license: "CC BY-NC 4.0; retain OMDb attribution and source URL".into(),
        media_kinds: vec![MediaKind::Movie, MediaKind::TvSeries], languages: vec!["en-US".into()],
        capabilities: vec!["search".into(), "details".into(), "images".into(), "external_ids".into()],
        fixture: false, documentation_url: Some("https://www.omdbapi.com".into()),
        engine_types: vec!["api".into()], requires_api_key: true, requires_login: false,
        rate_limit_policy: "API key and account quota required; >= 1 s request interval, concurrency 1; honor 429 and Retry-After.".into(),
        attribution: "Metadata supplied by OMDb, https://www.omdbapi.com; CC BY-NC 4.0.".into(),
        data_usage_notes: "Default disabled. English metadata stays English. Independent of IMDb; no invented seasons, episode mappings or actor roles. Does not call the separate paid Poster API.".into(),
    }
}

impl VideoProviderHub {
    async fn omdb_json(
        &self, parameters: &[(&str, &str)], job: &ProviderJob, cancel: &CancellationToken,
    ) -> Result<Option<Value>, ProviderFailure> {
        let mut url = Url::parse(&self.omdb_base).map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.query_pairs_mut().append_pair("r", "json");
        for (key, value) in parameters { url.query_pairs_mut().append_pair(key, value); }
        let secret = if let Some(auth) = &self.auth {
            if !job.config.credential_ref.as_deref().is_some_and(|reference| reference.starts_with("omdb.auth.api_key.")) {
                return Err(ProviderFailure::InvalidConfiguration);
            }
            let resolved = auth.resolve(job.config.credential_ref.as_deref()).await?;
            if resolved.method != "api_key" { return Err(ProviderFailure::InvalidConfiguration); }
            resolved.secret
        } else {
            let reference = job.config.credential_ref.as_deref().unwrap_or(CREDENTIAL_REF);
            if reference != CREDENTIAL_REF { return Err(ProviderFailure::InvalidConfiguration); }
            self.credentials.get(reference, self.credential_mode).await?
        };
        let mut spec = RequestSpec::get(url.as_str(), EngineKind::Api);
        spec.secret_query = Some(("apikey".into(), secret.clone()));
        let mut effective = job.clone();
        effective.config.interval_ms = effective.config.interval_ms.max(1000);
        effective.config.concurrency = 1;
        let value = self.engine.execute(&effective, spec, cancel).await?.json()?;
        if reflects_secret(&value, secret.expose()) { return Err(ProviderFailure::ParseError); }
        if response(&value)? { Ok(Some(value)) } else { Ok(None) }
    }

    pub(super) async fn omdb_search(
        &self, query: &VideoQuery, job: &ProviderJob, cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        let (title, year, ids, _) = query.parts();
        let movie = matches!(query, VideoQuery::Movie { .. });
        if matches!(query, VideoQuery::Anime { .. }) { return Err(ProviderFailure::InvalidConfiguration); }
        let media_type = if movie { "movie" } else { "series" };
        if let Some(id) = ids.iter().find(|id| matches!(id.namespace.as_str(), "imdb" | "omdb")) {
            imdb_id(&id.value)?;
            let Some(value) = self.omdb_json(&[("i", &id.value), ("type", media_type)], job, cancel).await? else { return Ok(Vec::new()); };
            let result = candidate(&value, movie)?;
            if result.data().remote_id != id.value { return Err(ProviderFailure::IdentityMismatch); }
            return Ok(vec![result]);
        }
        if !ids.is_empty() { return Ok(Vec::new()); }
        let mut parameters = vec![("s", title), ("type", media_type), ("page", "1")];
        let year = year.map(|year| year.to_string());
        if let Some(year) = &year { parameters.push(("y", year)); }
        let Some(value) = self.omdb_json(&parameters, job, cancel).await? else { return Ok(Vec::new()); };
        let rows = array(&value, "Search")?;
        if rows.len() > MAX_RESULTS { return Err(ProviderFailure::ResponseTooLarge); }
        rows.iter().map(|row| candidate(row, movie)).collect()
    }

    pub(super) async fn omdb_detail(
        &self, input: &VideoCandidate, job: &ProviderJob, cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        let id = &input.data().remote_id;
        imdb_id(id)?;
        let movie = matches!(input, VideoCandidate::Movie(_));
        if matches!(input, VideoCandidate::Anime(_)) { return Err(ProviderFailure::InvalidConfiguration); }
        let value = self.omdb_json(&[("i", id), ("type", if movie { "movie" } else { "series" }), ("plot", "full")], job, cancel)
            .await?.ok_or(ProviderFailure::NotFound)?;
        metadata(&value, id, movie)
    }
}

fn imdb_id(value: &str) -> Result<(), ProviderFailure> {
    if !value.starts_with("tt") || !(3..=20).contains(&value.len()) || !value[2..].bytes().all(|byte| byte.is_ascii_digit()) {
        Err(ProviderFailure::InvalidConfiguration)
    } else { Ok(()) }
}

fn reflects_secret(value: &Value, secret: &str) -> bool {
    match value {
        Value::String(value) => value.contains(secret),
        Value::Array(values) => values.iter().any(|value| reflects_secret(value, secret)),
        Value::Object(values) => values.iter().any(|(key, value)| key.contains(secret) || reflects_secret(value, secret)),
        _ => false,
    }
}

fn response(value: &Value) -> Result<bool, ProviderFailure> {
    match value["Response"].as_str() {
        Some("True") => Ok(true),
        Some("False") => match value["Error"].as_str().unwrap_or_default() {
            "Movie not found!" | "Series not found!" | "Incorrect IMDb ID." => Ok(false),
            "Invalid API key!" | "No API key provided." => Err(ProviderFailure::CredentialsUnavailable),
            "Request limit reached!" => Err(ProviderFailure::RateLimited),
            _ => Err(ProviderFailure::ParseError),
        },
        _ => Err(ProviderFailure::ParseError),
    }
}

fn available<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value[key].as_str().map(str::trim).filter(|value| !value.is_empty() && *value != "N/A")
}

fn candidate(value: &Value, movie: bool) -> Result<VideoCandidate, ProviderFailure> {
    let id = available(value, "imdbID").ok_or(ProviderFailure::ParseError)?;
    imdb_id(id)?;
    if value["Type"].as_str() != Some(if movie { "movie" } else { "series" }) { return Err(ProviderFailure::IdentityMismatch); }
    let year = available(value, "Year").map(|year| {
        let year = year.get(..4).ok_or(ProviderFailure::ParseError)?.parse::<i32>().map_err(|_| ProviderFailure::ParseError)?;
        if !(1800..=2200).contains(&year) { return Err(ProviderFailure::ParseError); }
        Ok(year)
    }).transpose()?;
    let data = CandidateData {
        provider_id: "omdb".into(), remote_id: id.into(),
        title: available(value, "Title").ok_or(ProviderFailure::ParseError)?.into(), original_title: None, year,
        aliases: vec![], external_ids: vec![RemoteId { namespace: "imdb".into(), value: id.into() }],
        source_url: format!("https://www.imdb.com/title/{id}/"), language: "en-US".into(),
    };
    Ok(if movie { VideoCandidate::Movie(data) } else { VideoCandidate::Tv(data) })
}

fn list(value: &Value, key: &str) -> Result<Vec<String>, ProviderFailure> {
    let entries = available(value, key).map(|text| text.split(',').map(str::trim).filter(|entry| !entry.is_empty()).map(str::to_owned).collect::<Vec<_>>()).unwrap_or_default();
    if entries.len() > 1000 { return Err(ProviderFailure::ResponseTooLarge); }
    Ok(entries)
}

fn metadata(value: &Value, expected: &str, movie: bool) -> Result<VideoMetadata, ProviderFailure> {
    let identity = candidate(value, movie)?.data().clone();
    if identity.remote_id != expected { return Err(ProviderFailure::IdentityMismatch); }
    let genres = list(value, "Genre")?;
    if genres.iter().any(|genre| genre.eq_ignore_ascii_case("adult")) { return Err(ProviderFailure::IdentityMismatch); }
    let mut fields = standard_fields(&identity, available(value, "Plot"));
    for (source, key) in [("Genre", "genres"), ("Director", "directors"), ("Writer", "writers"), ("Actors", "cast"), ("Country", "countries"), ("Production", "production_companies")] {
        let entries = list(value, source)?;
        if !entries.is_empty() { fields.push(field(key, json!(entries), Some("en-US"))); }
    }
    let runtime = available(value, "Runtime").and_then(|text| text.strip_suffix(" min")).and_then(|text| text.parse::<u32>().ok()).filter(|minutes| (1..=14400).contains(minutes));
    if let Some(runtime) = runtime { fields.push(field("runtime_minutes", json!(runtime), None)); }
    if let Some(date) = available(value, "Released").and_then(|text| chrono::NaiveDate::parse_from_str(text, "%d %b %Y").ok()) {
        fields.push(field("premiered", json!(date.format("%Y-%m-%d").to_string()), None));
    }
    if let Some(certification) = available(value, "Rated") { fields.push(field("mpaa", json!(certification), None)); }
    let votes = available(value, "imdbVotes").and_then(|votes| votes.replace(',', "").parse::<u64>().ok());
    if let Some(rating) = available(value, "imdbRating").and_then(|rating| rating.parse::<f64>().ok()).filter(|rating| rating.is_finite() && (0.0..=10.0).contains(rating)) {
        fields.push(field("rating", json!(rating), None));
        if let Some(votes) = votes { fields.push(field("votes", json!(votes), None)); }
    }
    let mut ratings = Vec::new();
    if let Some(entries) = value["Ratings"].as_array() {
        if entries.len() > 100 { return Err(ProviderFailure::ResponseTooLarge); }
        for entry in entries {
            let Some(source) = available(entry, "Source") else { continue; };
            let Some(raw) = available(entry, "Value") else { continue; };
            let parsed = if let Some(percent) = raw.strip_suffix('%') { Some((percent, "100")) } else { raw.split_once('/') };
            let Some((rating, maximum)) = parsed else { continue; };
            let (Ok(rating), Ok(maximum)) = (rating.parse::<f64>(), maximum.parse::<f64>()) else { continue; };
            if !rating.is_finite() || !maximum.is_finite() || maximum <= 0.0 || !(0.0..=maximum).contains(&rating) { continue; }
            let mut entry = json!({"name":source,"value":rating,"max":maximum,"default":source == "Internet Movie Database"});
            if source == "Internet Movie Database" { if let Some(votes) = votes { entry["votes"] = json!(votes); } }
            ratings.push(entry);
        }
    }
    if !ratings.is_empty() { fields.push(field("ratings", json!(ratings), None)); }
    fields.push(field("attribution", json!(format!("OMDb; {}; CC BY-NC 4.0", identity.source_url)), None));
    fields.push(field("source_license", json!("CC BY-NC 4.0; https://creativecommons.org/licenses/by-nc/4.0/"), None));
    let mut artwork = Vec::new();
    if let Some(url) = available(value, "Poster") {
        validate_image_url(url, &["m.media-amazon.com", "images-na.ssl-images-amazon.com"])?;
        let parsed = Url::parse(url).map_err(|_| ProviderFailure::UnsafeTarget)?;
        if parsed.query().is_some() || url.contains(['\\', '%']) || url.chars().any(char::is_control) { return Err(ProviderFailure::UnsafeTarget); }
        artwork.push(ArtworkCandidate { url: url.into(), source_provider: "omdb".into(), kind: "poster".into(), language: None });
    }
    Ok(if movie {
        VideoMetadata::Movie(MovieDetail { identity, fields, artwork, runtime_minutes: runtime })
    } else {
        VideoMetadata::Tv(TvDetail { identity, fields, artwork, seasons: Vec::new(), order: EpisodeOrder::Aired })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use uuid::Uuid;

    fn movie() -> Value {
        json!({"Response":"True","Title":"Fixture Film","Year":"2024","Type":"movie","imdbID":"tt1234567","Plot":"Actual synopsis","Runtime":"95 min","Genre":"Drama, Thriller","Director":"Director","Writer":"Writer","Actors":"First Actor, Second Actor","Country":"Japan, France","Production":"Studio","Released":"02 Jan 2024","Rated":"PG-13","imdbRating":"8.4","imdbVotes":"1,234","Ratings":[{"Source":"Internet Movie Database","Value":"8.4/10"},{"Source":"Rotten Tomatoes","Value":"90%"},{"Source":"Metacritic","Value":"70/100"}],"Poster":"https://m.media-amazon.com/images/M/fixture.jpg"})
    }

    #[test]
    fn actual_metadata_keeps_imdb_identity_ratings_and_cast_without_inventing_roles() {
        let VideoMetadata::Movie(detail) = metadata(&movie(), "tt1234567", true).unwrap() else { panic!("movie expected"); };
        assert_eq!(detail.runtime_minutes, Some(95));
        assert_eq!(detail.identity.language, "en-US");
        assert_eq!(detail.identity.external_ids, vec![RemoteId { namespace: "imdb".into(), value: "tt1234567".into() }]);
        for (key, expected) in [("cast", json!(["First Actor", "Second Actor"])), ("directors", json!(["Director"])), ("writers", json!(["Writer"])), ("premiered", json!("2024-01-02")), ("votes", json!(1234))] {
            assert_eq!(detail.fields.iter().find(|field| field.key == key).unwrap().value, expected);
        }
        let ratings = &detail.fields.iter().find(|field| field.key == "ratings").unwrap().value;
        assert_eq!(ratings[0]["votes"], 1234);
        assert_eq!(ratings[1]["max"], 100.0);
        assert!(detail.fields.iter().any(|field| field.key == "source_license"));
        assert_eq!(detail.artwork[0].source_provider, "omdb");
        assert_eq!(metadata(&movie(), "tt9999999", true), Err(ProviderFailure::IdentityMismatch));
        assert_eq!(metadata(&movie(), "tt1234567", false), Err(ProviderFailure::IdentityMismatch));
    }

    #[test]
    fn missing_data_and_series_do_not_create_fake_metadata_or_episode_mappings() {
        let value = json!({"Title":"Fixture Series","Year":"2020–2024","Type":"series","imdbID":"tt7654321","Actors":"N/A","Poster":"N/A","imdbRating":"N/A"});
        let VideoMetadata::Tv(detail) = metadata(&value, "tt7654321", false).unwrap() else { panic!("series expected"); };
        assert_eq!(detail.identity.year, Some(2020));
        assert!(detail.seasons.is_empty());
        assert!(detail.artwork.is_empty());
        assert!(!detail.fields.iter().any(|field| matches!(field.key.as_str(), "cast" | "rating" | "runtime_minutes")));
        let mut unsafe_image = movie();
        for url in ["https://example.com/poster.jpg", "https://user:secret@m.media-amazon.com/a.jpg", "https://m.media-amazon.com/a.jpg?apikey=secret", "http://m.media-amazon.com/a.jpg"] {
            unsafe_image["Poster"] = json!(url);
            assert_eq!(metadata(&unsafe_image, "tt1234567", true), Err(ProviderFailure::UnsafeTarget));
        }
        let mut adult = movie();
        adult["Genre"] = json!("Adult");
        assert_eq!(metadata(&adult, "tt1234567", true), Err(ProviderFailure::IdentityMismatch));
    }

    #[test]
    fn api_failures_and_reflected_credentials_cannot_look_like_empty_success() {
        assert_eq!(response(&json!({"Response":"False","Error":"Movie not found!"})), Ok(false));
        assert_eq!(response(&json!({"Response":"False","Error":"Invalid API key!"})), Err(ProviderFailure::CredentialsUnavailable));
        assert_eq!(response(&json!({"Response":"False","Error":"Request limit reached!"})), Err(ProviderFailure::RateLimited));
        assert_eq!(response(&json!({"Response":"False","Error":"Unexpected"})), Err(ProviderFailure::ParseError));
        assert!(reflects_secret(&json!({"nested":[{"Title":"Reflected synthetic-private-key"}]}), "synthetic-private-key"));
        for id in ["tt", "nm1234567", "tt123?apikey=bad", "tt123/../../", "https://www.imdb.com/title/tt1234567/"] { assert!(imdb_id(id).is_err()); }
    }

    #[tokio::test]
    async fn documented_search_and_exact_id_paths_use_saved_query_auth_without_logging_the_key() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let requests = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let captured = requests.clone();
        let server = tokio::spawn(async move {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let length = stream.read(&mut chunk).await.unwrap();
                    if length == 0 { return; }
                    bytes.extend_from_slice(&chunk[..length]);
                    assert!(bytes.len() <= 16384);
                    if bytes.windows(4).any(|window| window == b"\r\n\r\n") { break; }
                }
                let request = String::from_utf8(bytes).unwrap();
                let path = request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                let url = Url::parse(&format!("http://localhost{path}")).unwrap();
                let body = if url.query_pairs().any(|(key, _)| key == "s") {
                    json!({"Response":"True","Search":[{"Title":"Fixture Film","Year":"2024","Type":"movie","imdbID":"tt1234567"}]})
                } else { movie() }.to_string();
                captured.lock().await.push(request);
                let reply = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        let temp = crate::test_support::tempdir().unwrap();
        let repo = crate::library::LibraryRepository::open(&temp.path().join("auth.sqlite")).await.unwrap();
        let store = Arc::new(CredentialStore::new());
        let auth = crate::provider_auth::ProviderAuthService::new(repo.clone(), store.clone());
        auth.save(crate::provider_auth::AuthSaveInput {
            provider_id: "omdb".into(), method: "api_key".into(), storage: "session_only".into(), secret: "synthetic-omdb-query-key".into(),
        }).await.unwrap();
        let mut hub = VideoProviderHub::new(Arc::new(NetworkEngine::for_loopback(4)), store).with_auth_service(auth.clone());
        hub.omdb_base = base;
        let id = Uuid::new_v4();
        let job = ProviderJob {
            id, provider_id: "omdb".into(), media_item_id: None, media_kind: Some(MediaKind::Movie), file_identity: None,
            region: None, correlation_id: id, operation: "search".into(), query: "Fixture".into(), language: "en-US".into(),
            config: ProviderConfig { enabled: true, max_retries: 0, credential_ref: auth.snapshot_ref("omdb").await.unwrap(), ..Default::default() }, created_at: Utc::now(),
        };
        let cancel = CancellationToken::new();
        let title_query = VideoQuery::Movie { title: "Fixture".into(), year: Some(2024), external_ids: vec![], language: "zh-CN".into() };
        let title_candidates = hub.omdb_search(&title_query, &job, &cancel).await.unwrap();
        let exact_query = VideoQuery::Movie { title: "Wrong filename".into(), year: None, external_ids: vec![RemoteId { namespace: "imdb".into(), value: "tt1234567".into() }], language: "zh-CN".into() };
        let exact_candidates = hub.omdb_search(&exact_query, &job, &cancel).await.unwrap();
        assert_eq!(title_candidates[0].data().remote_id, exact_candidates[0].data().remote_id);
        let detail = hub.omdb_detail(&exact_candidates[0], &job, &cancel).await.unwrap();
        assert_eq!(detail.identity().language, "en-US");
        server.await.unwrap();
        let raw = requests.lock().await.join("\n");
        assert!(raw.contains("s=Fixture"));
        assert!(raw.contains("y=2024"));
        assert!(raw.contains("i=tt1234567"));
        assert!(raw.contains("plot=full"));
        assert!(raw.contains("apikey=synthetic-omdb-query-key"));
        assert!(!raw.to_ascii_lowercase().contains("authorization:"));
        let logs = serde_json::to_string(&hub.engine.records().await).unwrap();
        assert!(!logs.contains("synthetic-omdb-query-key"));
        assert!(!logs.contains("Wrong filename"));
        repo.pool().close().await;
    }
}
