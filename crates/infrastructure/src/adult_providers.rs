//! Independent adult metadata adapters. All HTTP uses the bounded P3 engine.
//! Official affiliate access and public CC0 catalog metadata are opt-in only.
use crate::{
    credentials::CredentialStore,
    network::{CancellationToken, EngineKind, NetworkEngine, RequestSpec, SecretString},
    provider_auth::{parse_affiliate_credential, ProviderAuthService},
};
use mediaatlas_domain::*;
use reqwest::Url;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct AdultProviderHub {
    engine: Arc<NetworkEngine>,
    _credentials: Arc<CredentialStore>,
    auth: Option<ProviderAuthService>,
    dmm_endpoint: String,
    wikidata_endpoint: String,
}
impl AdultProviderHub {
    pub fn new(engine: Arc<NetworkEngine>, credentials: Arc<CredentialStore>) -> Self {
        Self {
            engine,
            _credentials: credentials,
            auth: None,
            dmm_endpoint: "https://api.dmm.com/affiliate/v3/ItemList".into(),
            wikidata_endpoint: "https://query.wikidata.org/sparql".into(),
        }
    }
    pub fn with_auth_service(mut self, auth: ProviderAuthService) -> Self {
        self.auth = Some(auth);
        self
    }
    #[cfg(test)]
    pub(crate) fn with_mock_endpoints(mut self, dmm: &str, wikidata: &str) -> Self {
        self.dmm_endpoint = dmm.into();
        self.wikidata_endpoint = wikidata.into();
        self
    }
    pub fn descriptors() -> Vec<ProviderDescriptor> {
        vec![
            descriptor(
                "dmm",
                "FANZA official affiliate API",
                "https://affiliate.dmm.com",
                "https://affiliate.dmm.com/api/",
                true,
            ),
            descriptor(
                "wikidata",
                "Wikidata catalog supplement",
                "https://www.wikidata.org",
                "https://www.wikidata.org/wiki/Wikidata:Licensing",
                false,
            ),
        ]
    }
    pub async fn search(
        &self,
        provider: &str,
        query: &AdultQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<AdultCandidate>, ProviderFailure> {
        validate_query(query)?;
        if provider != job.provider_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        match provider {
            "dmm" => {
                let value = self
                    .dmm_request(Some(&query.catalog_number), None, job, cancel)
                    .await?;
                dmm_items(&value)?.iter().map(dmm_candidate).collect()
            }
            "wikidata" => {
                let normalized = normalize_catalog_number(&query.catalog_number, &query.namespace)?;
                let compact = normalized.normalized.replace('-', "");
                let sparql = format!("SELECT ?item ?itemLabel ?catalog ?catalogNamespace ?publisher ?publisherLabel ?adultType WHERE {{ ?item wdt:P31/wdt:P279* wd:Q185529. BIND(wd:Q185529 AS ?adultType) ?item p:P528 ?statement. ?statement ps:P528 ?catalog. FILTER(UCASE(REPLACE(STR(?catalog), '[-_]', '')) = '{compact}') ?statement pq:P972 ?catalogNamespace. ?item wdt:P123 ?publisher. SERVICE wikibase:label {{ bd:serviceParam wikibase:language 'ja,en'. }} }} LIMIT 26");
                let value = self.wikidata_request(&sparql, job, cancel).await?;
                bindings(&value)?.iter().map(wikidata_candidate).collect()
            }
            _ => Err(ProviderFailure::InvalidConfiguration),
        }
    }
    pub async fn detail(
        &self,
        candidate: &AdultCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<AdultDetail, ProviderFailure> {
        if candidate.provider_id != job.provider_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        let detail = match candidate.provider_id.as_str() {
            "dmm" => {
                validate_remote_id(&candidate.remote_id)?;
                let value = self
                    .dmm_request(None, Some(&candidate.remote_id), job, cancel)
                    .await?;
                let items = dmm_items(&value)?;
                if items.len() != 1 {
                    return Err(ProviderFailure::IdentityMismatch);
                }
                dmm_detail(&items[0])?
            }
            "wikidata" => {
                validate_qid(&candidate.remote_id)?;
                let sparql = format!("SELECT ?item ?itemLabel ?catalog ?catalogNamespace ?publisher ?publisherLabel ?labelLabel ?seriesLabel ?release ?adultType WHERE {{ VALUES ?item {{ wd:{} }} ?item wdt:P31/wdt:P279* wd:Q185529. BIND(wd:Q185529 AS ?adultType) ?item p:P528 ?statement. ?statement ps:P528 ?catalog. ?statement pq:P972 ?catalogNamespace. ?item wdt:P123 ?publisher. OPTIONAL {{ ?item wdt:P264 ?label. }} OPTIONAL {{ ?item wdt:P179 ?series. }} OPTIONAL {{ ?item wdt:P577 ?release. }} SERVICE wikibase:label {{ bd:serviceParam wikibase:language 'ja,en'. }} }} LIMIT 26", candidate.remote_id);
                let value = self.wikidata_request(&sparql, job, cancel).await?;
                let rows = bindings(&value)?;
                let matching = rows
                    .iter()
                    .filter_map(|row| wikidata_candidate(row).ok().map(|identity| (row, identity)))
                    .filter(|(_, identity)| {
                        catalog_equal(&identity.catalog_number, &candidate.catalog_number)
                    })
                    .collect::<Vec<_>>();
                if matching.len() != 1 {
                    return Err(ProviderFailure::IdentityMismatch);
                }
                let (row, identity) = &matching[0];
                AdultDetail {
                    identity: identity.clone(),
                    label: binding(row, "labelLabel"),
                    series: binding(row, "seriesLabel"),
                    release_date: normalized_date(binding(row, "release").as_deref())?,
                    runtime_minutes: None,
                    performers: vec![],
                    directors: vec![],
                    artwork: vec![],
                }
            }
            _ => return Err(ProviderFailure::InvalidConfiguration),
        };
        verify_detail(candidate, &detail)?;
        Ok(detail)
    }
    async fn dmm_request(
        &self,
        keyword: Option<&str>,
        cid: Option<&str>,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Value, ProviderFailure> {
        let auth = self
            .auth
            .as_ref()
            .ok_or(ProviderFailure::CredentialsUnavailable)?
            .resolve(job.config.credential_ref.as_deref())
            .await?;
        if auth.method != "affiliate"
            || !job
                .config
                .credential_ref
                .as_deref()
                .is_some_and(|reference| reference.starts_with("dmm.auth.affiliate."))
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let credentials = parse_affiliate_credential(auth.secret.expose())?;
        let mut url =
            Url::parse(&self.dmm_endpoint).map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.query_pairs_mut().extend_pairs([
            ("site", "FANZA"),
            ("service", "digital"),
            ("floor", "videoa"),
            ("hits", "25"),
            ("offset", "1"),
            ("sort", "match"),
            ("output", "json"),
        ]);
        if let Some(keyword) = keyword {
            url.query_pairs_mut().append_pair("keyword", keyword);
        }
        if let Some(cid) = cid {
            url.query_pairs_mut().append_pair("cid", cid);
        }
        let mut spec = RequestSpec::get(url.to_string(), EngineKind::Api);
        spec.user_agent = Some("MediaAtlas/0.1 (desktop metadata client)".into());
        spec.secret_queries = vec![
            (
                "api_id".into(),
                SecretString::new(credentials.api_id.clone()),
            ),
            (
                "affiliate_id".into(),
                SecretString::new(credentials.affiliate_id.clone()),
            ),
        ];
        let mut bounded = job.clone();
        bounded.config.interval_ms = bounded.config.interval_ms.max(1000);
        bounded.config.concurrency = 1;
        let value = self.engine.execute(&bounded, spec, cancel).await?.json()?;
        // Discard request echoes/tracking fields. Credential reflection in an
        // item is rejected before any domain DTO can reach SQLite or the UI.
        if let Some(items) = value["result"]["items"].as_array() {
            let serialized =
                serde_json::to_string(items).map_err(|_| ProviderFailure::ParseError)?;
            if serialized.contains(&credentials.api_id)
                || serialized.contains(&credentials.affiliate_id)
            {
                return Err(ProviderFailure::ParseError);
            }
        }
        match value["result"]["status"].as_u64() {
            Some(200) => Ok(value),
            Some(401) => Err(ProviderFailure::CredentialsUnavailable),
            Some(403) => Err(ProviderFailure::Blocked),
            Some(429) => Err(ProviderFailure::RateLimited),
            _ => Err(ProviderFailure::ParseError),
        }
    }
    async fn wikidata_request(
        &self,
        query: &str,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Value, ProviderFailure> {
        let mut url = Url::parse(&self.wikidata_endpoint)
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("format", "json");
        let mut spec = RequestSpec::get(url.to_string(), EngineKind::Api);
        spec.user_agent = Some("MediaAtlas/0.1 (desktop catalog metadata supplement)".into());
        let mut bounded = job.clone();
        bounded.config.interval_ms = bounded.config.interval_ms.max(1000);
        bounded.config.concurrency = 1;
        self.engine.execute(&bounded, spec, cancel).await?.json()
    }
}
fn descriptor(
    id: &str,
    name: &str,
    site: &str,
    terms: &str,
    authenticated: bool,
) -> ProviderDescriptor {
    ProviderDescriptor { id: id.into(), name: name.into(), description: "Independent adult catalog metadata; live access unverified".into(), website: Some(site.into()), terms_url: Some(terms.into()), privacy_url: None, license: if authenticated { "Official affiliate API terms; no redistribution rights inferred" } else { "CC0 structured data" }.into(), media_kinds: vec![MediaKind::AdultVideo], languages: vec!["ja".into(), "en".into()], capabilities: if authenticated { vec!["adult_search".into(), "adult_detail".into(), "adult_artwork".into()] } else { vec!["adult_search".into(), "adult_detail".into()] }, fixture: false, documentation_url: Some(terms.into()), engine_types: vec!["api".into()], requires_api_key: authenticated, requires_login: false, rate_limit_policy: "One request per second; single concurrent request; honor 403/429/CAPTCHA".into(), attribution: if authenticated { "DMM/FANZA affiliate API; affiliate terms apply" } else { "Wikidata contributors, CC0" }.into(), data_usage_notes: "Disabled by default. Metadata rights and account authorization require independent verification. No gated content access.".into() }
}
fn validate_query(query: &AdultQuery) -> Result<(), ProviderFailure> {
    normalize_catalog_number(&query.catalog_number, &query.namespace)?;
    normalize_locale(Some(&query.language)).map_err(|_| ProviderFailure::InvalidConfiguration)?;
    if query.studio.as_ref().is_some_and(|studio| {
        studio.is_empty() || studio.len() > 256 || studio.chars().any(char::is_control)
    }) {
        return Err(ProviderFailure::InvalidConfiguration);
    }
    Ok(())
}
fn validate_remote_id(value: &str) -> Result<(), ProviderFailure> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        Err(ProviderFailure::InvalidConfiguration)
    } else {
        Ok(())
    }
}
fn validate_qid(value: &str) -> Result<(), ProviderFailure> {
    if value.len() < 2
        || value.len() > 20
        || !value.starts_with('Q')
        || !value[1..].bytes().all(|b| b.is_ascii_digit())
    {
        Err(ProviderFailure::InvalidConfiguration)
    } else {
        Ok(())
    }
}
fn text(value: &Value, key: &str) -> Result<String, ProviderFailure> {
    let value = value[key]
        .as_str()
        .filter(|v| !v.is_empty() && v.len() <= 4096 && !v.chars().any(char::is_control))
        .ok_or(ProviderFailure::ParseError)?;
    Ok(value.into())
}
fn dmm_items(value: &Value) -> Result<&Vec<Value>, ProviderFailure> {
    let items = value["result"]["items"]
        .as_array()
        .ok_or(ProviderFailure::ParseError)?;
    if items.len() > 25 {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    Ok(items)
}
fn first_name(value: &Value, category: &str) -> Option<String> {
    value["iteminfo"][category].as_array()?.first()?["name"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .map(str::to_owned)
}
fn dmm_studio(value: &Value) -> Result<String, ProviderFailure> {
    let makers = value["iteminfo"]["maker"]
        .as_array()
        .ok_or(ProviderFailure::IdentityMismatch)?;
    if makers.len() != 1 {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let maker = &makers[0];
    let id = maker["id"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| maker["id"].as_u64().map(|v| v.to_string()))
        .ok_or(ProviderFailure::IdentityMismatch)?;
    validate_remote_id(&id)?;
    Ok(format!("{} (dmm:maker:{id})", text(maker, "name")?))
}
fn dmm_candidate(value: &Value) -> Result<AdultCandidate, ProviderFailure> {
    let remote_id = text(value, "content_id")?;
    validate_remote_id(&remote_id)?;
    let raw = text(value, "product_id")?;
    let mut catalog = normalize_catalog_number(&raw, "dmm.product")?;
    catalog.aliases.push(remote_id.clone());
    let source_url = format!("https://www.dmm.co.jp/digital/videoa/-/detail/=/cid={remote_id}/");
    Ok(AdultCandidate {
        provider_id: "dmm".into(),
        remote_id,
        catalog_number: catalog,
        title: text(value, "title")?,
        studio: Some(dmm_studio(value)?),
        source_url,
        language: "ja".into(),
    })
}
fn dmm_people(value: &Value, category: &str) -> Result<Vec<AdultPerson>, ProviderFailure> {
    let Some(values) = value["iteminfo"][category].as_array() else {
        return Ok(vec![]);
    };
    if values.len() > 100 {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    values
        .iter()
        .map(|entry| {
            let remote_id = entry["id"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| entry["id"].as_u64().map(|v| v.to_string()))
                .ok_or(ProviderFailure::ParseError)?;
            validate_remote_id(&remote_id)?;
            Ok(AdultPerson {
                provider_id: "dmm".into(),
                remote_id,
                name: text(entry, "name")?,
                aliases: vec![],
            })
        })
        .collect()
}
fn dmm_detail(value: &Value) -> Result<AdultDetail, ProviderFailure> {
    let mut artwork = vec![];
    if let Some(cover) = value["imageURL"]["large"].as_str() {
        artwork.push(dmm_image(cover, "cover")?);
    }
    if let Some(samples) = value["sampleImageURL"]["sample_l"]["image"].as_array() {
        if samples.len() > 99 {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        for sample in samples {
            artwork.push(dmm_image(
                sample.as_str().ok_or(ProviderFailure::ParseError)?,
                "sample",
            )?);
        }
    }
    let runtime_minutes = value["volume"]
        .as_str()
        .and_then(|s| s.trim_end_matches('分').parse::<u32>().ok())
        .filter(|v| *v <= 1440);
    Ok(AdultDetail {
        identity: dmm_candidate(value)?,
        label: first_name(value, "label"),
        series: first_name(value, "series"),
        release_date: normalized_date(value["date"].as_str())?,
        runtime_minutes,
        performers: dmm_people(value, "actress")?,
        directors: dmm_people(value, "director")?,
        artwork,
    })
}
fn normalized_date(value: Option<&str>) -> Result<Option<String>, ProviderFailure> {
    value
        .filter(|v| !v.is_empty())
        .map(|value| {
            let date = value.get(..10).ok_or(ProviderFailure::ParseError)?;
            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| ProviderFailure::ParseError)?;
            if value.len() > 10 && !matches!(value.as_bytes()[10], b' ' | b'T') {
                return Err(ProviderFailure::ParseError);
            }
            Ok(date.to_owned())
        })
        .transpose()
}
fn dmm_image(value: &str, kind: &str) -> Result<ArtworkCandidate, ProviderFailure> {
    let url = Url::parse(value).map_err(|_| ProviderFailure::UnsafeTarget)?;
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("pics.dmm.co.jp" | "pics.dmm.com"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || url.port().is_some_and(|v| v != 443)
    {
        return Err(ProviderFailure::UnsafeTarget);
    }
    Ok(ArtworkCandidate {
        url: value.into(),
        source_provider: "dmm".into(),
        kind: kind.into(),
        language: Some("ja".into()),
    })
}
fn bindings(value: &Value) -> Result<&Vec<Value>, ProviderFailure> {
    let values = value["results"]["bindings"]
        .as_array()
        .ok_or(ProviderFailure::ParseError)?;
    if values.len() > 25 {
        return Err(ProviderFailure::ResponseTooLarge);
    }
    Ok(values)
}
fn binding(value: &Value, name: &str) -> Option<String> {
    value[name]["value"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 4096 && !s.chars().any(char::is_control))
        .map(str::to_owned)
}
fn entity_id(value: &str) -> Result<&str, ProviderFailure> {
    let id = value
        .strip_prefix("http://www.wikidata.org/entity/")
        .or_else(|| value.strip_prefix("https://www.wikidata.org/entity/"))
        .ok_or(ProviderFailure::IdentityMismatch)?;
    validate_qid(id)?;
    Ok(id)
}
fn wikidata_candidate(value: &Value) -> Result<AdultCandidate, ProviderFailure> {
    let entity = binding(value, "item").ok_or(ProviderFailure::ParseError)?;
    let remote_id = entity_id(&entity)?.to_owned();
    let raw = binding(value, "catalog").ok_or(ProviderFailure::ParseError)?;
    let adult_type = binding(value, "adultType").ok_or(ProviderFailure::IdentityMismatch)?;
    if entity_id(&adult_type)? != "Q185529" {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let catalog_entity =
        binding(value, "catalogNamespace").ok_or(ProviderFailure::IdentityMismatch)?;
    let namespace = format!("wikidata.catalog:{}", entity_id(&catalog_entity)?);
    let publisher = binding(value, "publisher").ok_or(ProviderFailure::IdentityMismatch)?;
    let publisher_id = entity_id(&publisher)?;
    let studio = Some(match binding(value, "publisherLabel") {
        Some(name) => format!("{name} ({publisher_id})"),
        None => publisher_id.into(),
    });
    Ok(AdultCandidate {
        provider_id: "wikidata".into(),
        remote_id: remote_id.clone(),
        catalog_number: normalize_catalog_number(&raw, &namespace)?,
        title: binding(value, "itemLabel").unwrap_or_else(|| remote_id.clone()),
        studio,
        source_url: format!("https://www.wikidata.org/wiki/{remote_id}"),
        language: "und".into(),
    })
}
fn catalog_equal(left: &CatalogNumber, right: &CatalogNumber) -> bool {
    left.normalized == right.normalized
        && left.namespace == right.namespace
        && left.part == right.part
}
fn verify_detail(candidate: &AdultCandidate, detail: &AdultDetail) -> Result<(), ProviderFailure> {
    if candidate.provider_id != detail.identity.provider_id
        || candidate.remote_id != detail.identity.remote_id
        || !catalog_equal(&candidate.catalog_number, &detail.identity.catalog_number)
        || candidate.studio != detail.identity.studio
    {
        Err(ProviderFailure::IdentityMismatch)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{library::LibraryRepository, provider_auth::AuthSaveInput};
    use chrono::Utc;
    use serde_json::json;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::Mutex,
    };
    use uuid::Uuid;
    type Requests = Arc<Mutex<Vec<String>>>;
    async fn server(
        dmm: Value,
        wikidata: Value,
    ) -> (String, Requests, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(vec![]));
        let captured = requests.clone();
        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0; 4096];
                    let count = socket.read(&mut buffer).await.unwrap();
                    if count == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                    if bytes.windows(4).any(|v| v == b"\r\n\r\n") {
                        break;
                    }
                    assert!(bytes.len() < 32768);
                }
                let request = String::from_utf8_lossy(&bytes).into_owned();
                let value = if request.starts_with("GET /dmm") {
                    if request.contains("cid=") && dmm["synthetic_detail_response"].is_object() {
                        &dmm["synthetic_detail_response"]
                    } else {
                        &dmm
                    }
                } else {
                    &wikidata
                };
                captured.lock().await.push(request);
                let body = value.to_string();
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (format!("http://{address}"), requests, handle)
    }
    fn job(provider: &str) -> ProviderJob {
        ProviderJob {
            id: Uuid::new_v4(),
            provider_id: provider.into(),
            media_item_id: None,
            media_kind: Some(MediaKind::AdultVideo),
            file_identity: None,
            region: None,
            correlation_id: Uuid::new_v4(),
            operation: "adult_search".into(),
            query: "SYN-001".into(),
            language: "ja".into(),
            config: ProviderConfig {
                enabled: true,
                max_retries: 0,
                interval_ms: 0,
                ..Default::default()
            },
            created_at: Utc::now(),
        }
    }
    fn query() -> AdultQuery {
        AdultQuery {
            catalog_number: "SYN-001".into(),
            namespace: "dmm.product".into(),
            studio: None,
            language: "zh-CN".into(),
        }
    }
    fn item() -> Value {
        json!({"content_id":"syn00001", "product_id":"SYN-001", "title":"Synthetic catalog fixture", "URL":"https://invalid.example/?api_id=never-used", "affiliateURL":"https://invalid.example/?affiliate_id=never-used", "date":"2026-01-01 00:00:00", "volume":"120", "iteminfo":{"maker":[{"id":1,"name":"Synthetic studio"}],"label":[{"id":2,"name":"Synthetic label"}],"series":[{"id":3,"name":"Synthetic series"}],"actress":[{"id":10,"name":"Synthetic performer"},{"id":11,"name":"Synthetic performer"}],"director":[{"id":12,"name":"Synthetic director"}]}, "imageURL":{"large":"https://pics.dmm.co.jp/synthetic/cover.jpg"}, "sampleImageURL":{"sample_l":{"image":["https://pics.dmm.com/synthetic/sample.jpg"]}}})
    }
    fn row() -> Value {
        json!({"item":{"type":"uri","value":"http://www.wikidata.org/entity/Q999999"},"itemLabel":{"type":"literal","value":"Synthetic catalog fixture"},"catalog":{"type":"literal","value":"SYN-001"},"catalogNamespace":{"type":"uri","value":"http://www.wikidata.org/entity/Q123"},"publisher":{"type":"uri","value":"http://www.wikidata.org/entity/Q456"},"publisherLabel":{"type":"literal","value":"Synthetic studio"},"adultType":{"type":"uri","value":"http://www.wikidata.org/entity/Q185529"}})
    }
    async fn setup(
        item: Value,
        row: Value,
        detail_item: Option<Value>,
    ) -> (
        tempfile::TempDir,
        AdultProviderHub,
        ProviderJob,
        Requests,
        tokio::task::JoinHandle<()>,
    ) {
        let (base, requests, handle) = server(json!({"result":{"status":200,"items":[item]},"synthetic_detail_response":detail_item.map(|item| json!({"result":{"status":200,"items":[item]}})),"request":{"parameters":{"api_id":"synthetic-api-id","affiliate_id":"synthetic-affiliate-id"}}}), json!({"results":{"bindings":[row]}})).await;
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("provider.sqlite"))
            .await
            .unwrap();
        let credentials = Arc::new(CredentialStore::new());
        let auth = ProviderAuthService::new(repo, credentials.clone());
        auth.save(AuthSaveInput {
            provider_id: "dmm".into(),
            method: "affiliate".into(),
            storage: "session_only".into(),
            secret: r#"{"api_id":"synthetic-api-id","affiliate_id":"synthetic-affiliate-id"}"#
                .into(),
        })
        .await
        .unwrap();
        let mut job = job("dmm");
        job.config.credential_ref = auth.snapshot_ref("dmm").await.unwrap();
        let hub = AdultProviderHub::new(Arc::new(NetworkEngine::for_loopback(4)), credentials)
            .with_auth_service(auth)
            .with_mock_endpoints(&format!("{base}/dmm"), &format!("{base}/wikidata"));
        (dir, hub, job, requests, handle)
    }
    #[tokio::test]
    async fn dmm_actual_api_search_detail_dual_query_credentials_and_scoped_people() {
        let (_dir, hub, job, requests, handle) = setup(item(), row(), None).await;
        let candidates = hub
            .search("dmm", &query(), &job, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(candidates[0].catalog_number.normalized, "SYN-001");
        let detail = hub
            .detail(&candidates[0], &job, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(detail.runtime_minutes, Some(120));
        assert_eq!(
            detail.identity.studio.as_deref(),
            Some("Synthetic studio (dmm:maker:1)")
        );
        assert_eq!(detail.label.as_deref(), Some("Synthetic label"));
        assert_eq!(detail.performers[0].name, detail.performers[1].name);
        assert_ne!(
            detail.performers[0].remote_id,
            detail.performers[1].remote_id
        );
        assert_eq!(detail.artwork.len(), 2);
        let raw = requests.lock().await.join("\n");
        assert!(raw.contains("api_id=synthetic-api-id"));
        assert!(raw.contains("affiliate_id=synthetic-affiliate-id"));
        assert!(raw.contains("site=FANZA"));
        assert!(raw.contains("cid=syn00001"));
        let serialized = serde_json::to_string(&detail).unwrap();
        assert!(!serialized.contains("never-used"));
        assert!(!serialized.contains("synthetic-api-id"));
        assert!(!serialized.contains("synthetic-affiliate-id"));
        let records = serde_json::to_string(&hub.engine.records().await).unwrap();
        assert!(!records.contains("synthetic-api-id"));
        assert!(!records.contains("synthetic-affiliate-id"));
        assert!(!records.contains("SYN-001"));
        handle.abort();
    }
    #[tokio::test]
    async fn detail_wrong_number_studio_and_image_tracking_are_rejected() {
        let (_dir, hub, job, _, handle) = setup(item(), row(), None).await;
        let candidate = hub
            .search("dmm", &query(), &job, &CancellationToken::new())
            .await
            .unwrap()
            .remove(0);
        let mut wrong = candidate.clone();
        wrong.studio = Some("Different issuer".into());
        assert_eq!(
            hub.detail(&wrong, &job, &CancellationToken::new()).await,
            Err(ProviderFailure::IdentityMismatch)
        );
        let mut reissued = item();
        reissued["iteminfo"]["maker"][0]["id"] = serde_json::json!(999);
        assert_ne!(dmm_candidate(&reissued).unwrap().studio, candidate.studio);
        assert_eq!(
            verify_detail(&candidate, &dmm_detail(&reissued).unwrap()),
            Err(ProviderFailure::IdentityMismatch)
        );
        let mut missing = item();
        missing["iteminfo"]["maker"] = serde_json::json!([]);
        assert_eq!(
            dmm_candidate(&missing),
            Err(ProviderFailure::IdentityMismatch)
        );
        let mut wrong = candidate;
        wrong.catalog_number = normalize_catalog_number("SYN-002", "dmm.product").unwrap();
        assert_eq!(
            hub.detail(&wrong, &job, &CancellationToken::new()).await,
            Err(ProviderFailure::IdentityMismatch)
        );
        assert_eq!(
            dmm_image(
                "https://pics.dmm.com/synthetic/a.jpg?affiliate_id=secret",
                "cover"
            ),
            Err(ProviderFailure::UnsafeTarget)
        );
        assert_eq!(
            dmm_image("https://evil.example/synthetic/a.jpg", "cover"),
            Err(ProviderFailure::UnsafeTarget)
        );
        handle.abort();
    }
    #[tokio::test]
    async fn actual_search_detail_reject_same_studio_name_with_different_issuer_id() {
        let mut reissue = item();
        reissue["iteminfo"]["maker"][0]["id"] = json!(999);
        let (_dir, hub, job, requests, handle) = setup(item(), row(), Some(reissue)).await;
        let candidate = hub
            .search("dmm", &query(), &job, &CancellationToken::new())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(
            hub.detail(&candidate, &job, &CancellationToken::new())
                .await,
            Err(ProviderFailure::IdentityMismatch)
        );
        assert_eq!(requests.lock().await.len(), 2);
        handle.abort();
    }
    #[tokio::test]
    async fn wikidata_actual_query_requires_adult_type_catalog_namespace_and_publisher() {
        let (_dir, hub, _, requests, handle) = setup(item(), row(), None).await;
        let job = job("wikidata");
        let candidate = hub
            .search("wikidata", &query(), &job, &CancellationToken::new())
            .await
            .unwrap()
            .remove(0);
        assert_eq!(candidate.catalog_number.namespace, "wikidata.catalog:Q123");
        assert_eq!(candidate.studio.as_deref(), Some("Synthetic studio (Q456)"));
        let detail = hub
            .detail(&candidate, &job, &CancellationToken::new())
            .await
            .unwrap();
        assert!(detail.artwork.is_empty());
        let raw = requests.lock().await.join("\n");
        let request_url = raw
            .lines()
            .find(|line| line.starts_with("GET"))
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        let url = Url::parse(&format!("http://fixture.test{request_url}")).unwrap();
        let query = url
            .query_pairs()
            .find(|(key, _)| key == "query")
            .unwrap()
            .1
            .into_owned();
        assert!(query.contains("wdt:P31/wdt:P279* wd:Q185529"));
        assert!(query.contains("?statement pq:P972 ?catalogNamespace"));
        assert!(query.contains("?item wdt:P123 ?publisher"));
        for name in ["adultType", "catalogNamespace", "publisher"] {
            let mut missing = row();
            missing.as_object_mut().unwrap().remove(name);
            assert_eq!(
                wikidata_candidate(&missing),
                Err(ProviderFailure::IdentityMismatch)
            );
        }
        let mut ordinary = row();
        ordinary["adultType"]["value"] = json!("http://www.wikidata.org/entity/Q11424");
        assert_eq!(
            wikidata_candidate(&ordinary),
            Err(ProviderFailure::IdentityMismatch)
        );
        handle.abort();
    }
    #[tokio::test]
    async fn credential_reflection_disabled_cancelled_and_response_bounds_fail_explicitly() {
        let mut reflected = item();
        reflected["title"] = json!("synthetic-api-id");
        let (_dir, hub, job, requests, handle) = setup(reflected, row(), None).await;
        assert_eq!(
            hub.search("dmm", &query(), &job, &CancellationToken::new())
                .await,
            Err(ProviderFailure::ParseError)
        );
        let mut disabled = job.clone();
        disabled.config.enabled = false;
        assert_eq!(
            hub.search("dmm", &query(), &disabled, &CancellationToken::new())
                .await,
            Err(ProviderFailure::Disabled)
        );
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            hub.search("dmm", &query(), &job, &cancel).await,
            Err(ProviderFailure::Cancelled)
        );
        assert_eq!(requests.lock().await.len(), 1);
        assert_eq!(
            dmm_items(&json!({"result":{"items":vec![item();26]}})),
            Err(ProviderFailure::ResponseTooLarge)
        );
        assert_eq!(
            bindings(&json!({"results":{"bindings":vec![row();26]}})),
            Err(ProviderFailure::ResponseTooLarge)
        );
        handle.abort();
    }
}
