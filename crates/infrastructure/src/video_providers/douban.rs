//! Public Douban subject pages; no login, challenge bypass or private API.
use super::*;
use scraper::{Html, Selector};
use std::collections::BTreeSet;

const IMAGE_HOSTS: &[&str] = &["img1.doubanio.com", "img2.doubanio.com", "img3.doubanio.com", "img9.doubanio.com"];

pub(super) fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: "douban".into(), name: "Douban".into(),
        description: "Chinese movie and television subject metadata from public pages.".into(),
        website: Some("https://movie.douban.com".into()),
        terms_url: Some("https://www.douban.com/about/agreement".into()),
        privacy_url: Some("https://www.douban.com/about/privacy".into()),
        license: "Douban and individual contributors retain rights; no redistribution license inferred.".into(),
        media_kinds: vec![MediaKind::Movie, MediaKind::TvSeries],
        languages: vec!["zh-CN".into()],
        capabilities: vec!["search".into(), "details".into(), "images".into(), "external_ids".into()],
        fixture: false,
        documentation_url: Some("https://github.com/susanforme/MediaAtlas/blob/main/docs/provider-access.md".into()),
        engine_types: vec!["api".into(), "html".into()],
        requires_api_key: false, requires_login: false,
        rate_limit_policy: "Public pages only; enforced >= 6 s between requests, concurrency 1; stop on redirects, verification, 403 and 429.".into(),
        attribution: "Metadata supplied by Douban; retain the subject source URL.".into(),
        data_usage_notes: "Default disabled. Exact subject IDs and title suggestions only; no IMDb-to-Douban guessing, private APIs or cookies. Each TV subject remains independent; no invented season or episode mapping.".into(),
    }
}

impl VideoProviderHub {
    async fn douban_response(
        &self, path: &str, query: Option<&str>, engine: EngineKind,
        job: &ProviderJob, cancel: &CancellationToken,
    ) -> Result<crate::network::EngineResponse, ProviderFailure> {
        let mut url = Url::parse(&format!("{}{path}", self.douban_base))
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        if let Some(query) = query { url.query_pairs_mut().append_pair("q", query); }
        let mut effective = job.clone();
        effective.config.interval_ms = effective.config.interval_ms.max(6000);
        effective.config.concurrency = 1;
        effective.config.credential_ref = None;
        let mut spec = RequestSpec::get(url.as_str(), engine);
        spec.user_agent = Some("MediaAtlas/0.1.0 (public metadata; https://github.com/susanforme/MediaAtlas)".into());
        let response = self.engine.execute(&effective, spec, cancel).await?;
        check_verification(response.text()?)?;
        Ok(response)
    }

    pub(super) async fn douban_search(
        &self, query: &VideoQuery, job: &ProviderJob, cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        let tv = matches!(query, VideoQuery::Tv { .. });
        if matches!(query, VideoQuery::Anime { .. }) { return Err(ProviderFailure::InvalidConfiguration); }
        let (title, _, ids, _) = query.parts();
        if let Some(id) = ids.iter().find(|id| id.namespace == "douban") {
            numeric_id(&id.value)?;
            let response = self.douban_response(&format!("/subject/{}/", id.value), None, EngineKind::Html, job, cancel).await?;
            return Ok(vec![subject(response.text()?, &id.value, tv)?.identity]);
        }
        // An explicit identity never falls back to an unrelated title search.
        if !ids.is_empty() { return Err(ProviderFailure::InvalidConfiguration); }
        let response = self.douban_response("/j/subject_suggest", Some(title), EngineKind::Api, job, cancel).await?;
        suggestions(&response.json()?, tv)
    }

    pub(super) async fn douban_detail(
        &self, candidate: &VideoCandidate, job: &ProviderJob, cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        let tv = matches!(candidate, VideoCandidate::Tv(_));
        if matches!(candidate, VideoCandidate::Anime(_)) { return Err(ProviderFailure::InvalidConfiguration); }
        let id = &candidate.data().remote_id;
        numeric_id(id)?;
        let response = self.douban_response(&format!("/subject/{id}/"), None, EngineKind::Html, job, cancel).await?;
        let parsed = subject(response.text()?, id, tv)?;
        let identity = parsed.identity.data().clone();
        if candidate.data().external_ids.iter().filter(|id| id.namespace == "imdb")
            .any(|expected| !identity.external_ids.contains(expected))
        { return Err(ProviderFailure::IdentityMismatch); }
        Ok(if tv {
            VideoMetadata::Tv(TvDetail { identity, fields: parsed.fields, artwork: parsed.artwork,
                seasons: vec![], order: EpisodeOrder::Aired })
        } else {
            VideoMetadata::Movie(MovieDetail { identity, fields: parsed.fields, artwork: parsed.artwork,
                runtime_minutes: parsed.runtime })
        })
    }
}

fn check_verification(body: &str) -> Result<(), ProviderFailure> {
    let lower = body.to_ascii_lowercase();
    if lower.contains("captcha") || body.contains("机器人验证") || body.contains("安全验证")
        || body.contains("异常请求") || body.contains("访问豆瓣的方式有点像机器人程序")
    { return Err(ProviderFailure::Captcha); }
    if lower.contains("sec.douban.com") || body.contains("检测到有异常请求") {
        return Err(ProviderFailure::Blocked);
    }
    Ok(())
}

fn source(id: &str) -> String { format!("https://movie.douban.com/subject/{id}/") }

fn subject_id(reference: &str) -> Result<String, ProviderFailure> {
    let base = Url::parse("https://movie.douban.com").map_err(|_| ProviderFailure::ParseError)?;
    let url = base.join(reference).map_err(|_| ProviderFailure::UnsafeTarget)?;
    if url.scheme() != "https" || url.host_str() != Some("movie.douban.com")
        || !url.username().is_empty() || url.password().is_some() || url.port().is_some()
        || url.fragment().is_some() || reference.contains('\\') || url.path().contains('%')
        || reference.split('?').next().is_some_and(|path| path.contains(".."))
        || reference.chars().any(char::is_control)
    { return Err(ProviderFailure::UnsafeTarget); }
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    if parts.len() != 2 || parts[0] != "subject" { return Err(ProviderFailure::IdentityMismatch); }
    numeric_id(parts[1])?;
    Ok(parts[1].into())
}

fn image_url(reference: &str) -> Result<(), ProviderFailure> {
    validate_image_url(reference, IMAGE_HOSTS)?;
    let url = Url::parse(reference).map_err(|_| ProviderFailure::UnsafeTarget)?;
    if url.query().is_some() || url.fragment().is_some() || reference.contains(['\\', '%'])
        || reference.contains("..") || reference.chars().any(char::is_control)
    { return Err(ProviderFailure::UnsafeTarget); }
    Ok(())
}

fn suggestions(data: &Value, tv: bool) -> Result<Vec<VideoCandidate>, ProviderFailure> {
    let rows = data.as_array().ok_or(ProviderFailure::ParseError)?;
    if rows.len() > MAX_RESULTS { return Err(ProviderFailure::ResponseTooLarge); }
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        if row["type"].as_str() != Some("movie") { continue; }
        let episodes = row["episode"].as_str().unwrap_or_default();
        let is_tv = !episodes.is_empty() && episodes.parse::<u32>().is_ok_and(|episodes| episodes > 0);
        if tv != is_tv { continue; }
        let id = required_id(&row["id"])?;
        numeric_id(&id)?;
        if subject_id(required_str(row, "url")?)? != id { return Err(ProviderFailure::IdentityMismatch); }
        if !seen.insert(id.clone()) { continue; }
        let year = row["year"].as_str().filter(|year| !year.is_empty()).map(|year| {
            year.parse::<i32>().ok().filter(|year| (1800..=2200).contains(year)).ok_or(ProviderFailure::ParseError)
        }).transpose()?;
        let identity = CandidateData { provider_id: "douban".into(), remote_id: id.clone(),
            title: required_str(row, "title")?.into(),
            original_title: row["sub_title"].as_str().filter(|title| !title.trim().is_empty()).map(str::to_owned),
            year, aliases: vec![], external_ids: vec![RemoteId { namespace: "douban".into(), value: id.clone() }],
            source_url: source(&id), language: "zh-CN".into() };
        output.push(if tv { VideoCandidate::Tv(identity) } else { VideoCandidate::Movie(identity) });
    }
    Ok(output)
}

struct Subject {
    identity: VideoCandidate,
    fields: Vec<RemoteField>,
    artwork: Vec<ArtworkCandidate>,
    runtime: Option<u32>,
}

fn selector(value: &str) -> Result<Selector, ProviderFailure> {
    Selector::parse(value).map_err(|_| ProviderFailure::ParseError)
}

fn first_text(document: &Html, css: &str) -> Result<Option<String>, ProviderFailure> {
    Ok(document.select(&selector(css)?).next().map(|node| node.text().collect::<String>().trim().to_owned())
        .filter(|value| !value.is_empty()))
}

fn info_values(info: &str, label: &str) -> Vec<String> {
    info.lines().find_map(|line| line.trim().strip_prefix(label))
        .map(|value| value.split('/').map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn names(data: &Value) -> Result<Vec<String>, ProviderFailure> {
    let rows: Vec<&Value> = match data { Value::Array(rows) => rows.iter().collect(), Value::Null => vec![], _ => vec![data] };
    if rows.len() > 1000 { return Err(ProviderFailure::ResponseTooLarge); }
    let mut seen = BTreeSet::new();
    Ok(rows.into_iter().filter_map(|row| row.as_str().or_else(|| row["name"].as_str()))
        .filter(|name| !name.trim().is_empty() && seen.insert((*name).to_owned())).map(str::to_owned).collect())
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str().and_then(|value| value.parse().ok())).filter(|value| value.is_finite())
}

fn subject(body: &str, expected: &str, tv: bool) -> Result<Subject, ProviderFailure> {
    numeric_id(expected)?;
    check_verification(body)?;
    let document = Html::parse_document(body);
    let json_ld = parse_html(body).json_ld()?;
    let data = json_ld.iter().flat_map(|entry| entry.as_array().map(|items| items.iter().collect::<Vec<_>>()).unwrap_or_else(|| vec![entry]))
        .find(|entry| matches!(entry["@type"].as_str(), Some("Movie" | "TVSeries")))
        .ok_or(ProviderFailure::ParseError)?;
    let canonical = document.select(&selector("link[rel='canonical']")?).next().and_then(|node| node.value().attr("href"));
    let actual = data["url"].as_str().or(canonical).ok_or(ProviderFailure::IdentityMismatch)?;
    if subject_id(actual)? != expected { return Err(ProviderFailure::IdentityMismatch); }
    if canonical.is_some_and(|canonical| subject_id(canonical).ok().as_deref() != Some(expected)) {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let info = first_text(&document, "#info")?.unwrap_or_default();
    let is_tv = data["@type"].as_str() == Some("TVSeries") || info.contains("首播:") || info.contains("首播：");
    if is_tv != tv { return Err(ProviderFailure::IdentityMismatch); }
    let mut ids = vec![RemoteId { namespace: "douban".into(), value: expected.into() }];
    for anchor in document.select(&selector("#info a[href]")?) {
        if let Some(href) = anchor.value().attr("href") {
            if let Ok(url) = Url::parse(href) {
                if matches!(url.host_str(), Some("www.imdb.com" | "imdb.com")) {
                    let parts = url.path().trim_matches('/').split('/').collect::<Vec<_>>();
                    if parts.len() == 2 && parts[0] == "title" {
                        let imdb = parts[1];
                        if imdb.starts_with("tt") && imdb.len() > 2 && imdb.len() <= 20 && imdb[2..].bytes().all(|byte| byte.is_ascii_digit()) {
                            ids.push(RemoteId { namespace: "imdb".into(), value: imdb.into() });
                        }
                    }
                }
            }
        }
    }
    let date = data["datePublished"].as_str().filter(|date| date.len() == 10 && chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok());
    let year = date.and_then(|date| date[..4].parse::<i32>().ok())
        .or(first_text(&document, "h1 .year")?.and_then(|year| year.trim_matches(['(', ')']).parse().ok()))
        .filter(|year| (1800..=2200).contains(year));
    let identity = CandidateData { provider_id: "douban".into(), remote_id: expected.into(),
        title: required_str(data, "name")?.into(), original_title: None, year, aliases: vec![],
        external_ids: ids, source_url: source(expected), language: "zh-CN".into() };
    let overview = first_text(&document, "#link-report-intra .all, #link-report .all")?
        .or(first_text(&document, "span[property='v:summary']")?)
        .or_else(|| data["description"].as_str().filter(|value| !value.trim().is_empty()).map(str::to_owned));
    let mut fields = standard_fields(&identity, overview.as_deref());
    if let Some(date) = date { fields.push(field("premiered", json!(date), None)); }
    for (key, property) in [("directors", "director"), ("writers", "author"), ("genres", "genre")] {
        let values = names(&data[property])?;
        if !values.is_empty() { fields.push(field(key, json!(values), Some("zh-CN"))); }
    }
    let countries = info_values(&info, "制片国家/地区:");
    if !countries.is_empty() { fields.push(field("countries", json!(countries), Some("zh-CN"))); }
    let writers = info_values(&info, "编剧:");
    if !writers.is_empty() && !fields.iter().any(|field| field.key == "writers") {
        fields.push(field("writers", json!(writers), Some("zh-CN")));
    }
    let mut cast = names(&data["actor"] )?.into_iter().map(|name| json!({"name":name})).collect::<Vec<_>>();
    for node in document.select(&selector("#celebrities li.celebrity")?) {
        let fragment = Html::parse_fragment(&node.html());
        let Some(name) = first_text(&fragment, "a.name")? else { continue; };
        let Some(person) = cast.iter_mut().find(|person| person["name"].as_str() == Some(name.as_str())) else { continue; };
        if let Some(role) = first_text(&fragment, "span.role")? {
            if let Some(role) = role.strip_prefix("饰 ").or_else(|| role.strip_prefix("配音 ")) {
                if !role.trim().is_empty() { person["role"] = json!(role.trim()); }
            }
        }
        if let Some(style) = fragment.select(&selector(".avatar[style]")?).next().and_then(|avatar| avatar.value().attr("style")) {
            if let Some((_, rest)) = style.split_once("url(") {
                if let Some((reference, _)) = rest.split_once(')') {
                    let reference = reference.trim_matches(['\'', '"', ' ']);
                    image_url(reference)?;
                    person["thumb"] = json!(reference);
                }
            }
        }
    }
    if !cast.is_empty() { fields.push(field("cast", json!(cast), Some("zh-CN"))); }
    if let Some(rating) = number(&data["aggregateRating"]["ratingValue"]).filter(|rating| (0.0..=10.0).contains(rating)) {
        fields.push(field("rating", json!(rating), None));
    }
    if let Some(votes) = scalar_id(&data["aggregateRating"]["ratingCount"]).and_then(|votes| votes.parse::<u64>().ok()) {
        fields.push(field("votes", json!(votes), None));
    }
    let runtime = data["duration"].as_str().and_then(|duration| duration.strip_prefix("PT")).and_then(|duration| duration.strip_suffix('M'))
        .and_then(|duration| duration.parse::<u32>().ok()).filter(|runtime| *runtime > 0 && *runtime <= 10000)
        .or_else(|| document.select(&selector("span[property='v:runtime']").ok()?).next()
            .and_then(|node| node.value().attr("content")).and_then(|value| value.parse::<u32>().ok()).filter(|runtime| *runtime > 0 && *runtime <= 10000));
    if let Some(runtime) = runtime { fields.push(field("runtime_minutes", json!(runtime), None)); }
    let mut artwork = vec![];
    if let Some(reference) = data["image"].as_str() {
        image_url(reference)?;
        artwork.push(ArtworkCandidate { url: reference.into(), source_provider: "douban".into(), kind: "poster".into(), language: None });
    }
    fields.push(field("attribution", json!(format!("Douban; {}", identity.source_url)), None));
    Ok(Subject { identity: if tv { VideoCandidate::Tv(identity) } else { VideoCandidate::Movie(identity) }, fields, artwork, runtime })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn movie() -> String {
        format!(r#"<html><head><link rel="canonical" href="https://movie.douban.com/subject/123/"><script type="application/ld+json">{}</script></head><body><div id="content"><h1><span class="year">(2024)</span></h1><div id="info">制片国家/地区: 中国大陆 / 日本
编剧: Writer A / Writer B
IMDb: <a href="https://www.imdb.com/title/tt1234567/">tt1234567</a></div><span property="v:summary">Public &amp; actual plot.</span><ul id="celebrities"><li class="celebrity"><div class="avatar" style="background-image: url(https://img2.doubanio.com/view/personage/public/actor.jpg)"></div><a class="name">Actor</a><span class="role">饰 Hero</span></li></ul></div></body></html>"#, json!({"@type":"Movie","name":"Fixture Movie","url":"/subject/123/","datePublished":"2024-02-03","duration":"PT95M","director":[{"name":"Director"}],"author":[{"name":"Writer"}],"actor":[{"name":"Actor"}],"genre":["Drama","Comedy"],"aggregateRating":{"ratingValue":"8.2","ratingCount":"42"},"image":"https://img1.doubanio.com/view/photo/s_ratio_poster/public/poster.jpg"}))
    }

    #[test]
    fn public_subject_keeps_actual_kodi_fields_and_checks_identity() {
        let parsed = subject(&movie(), "123", false).unwrap();
        assert_eq!(parsed.identity.data().remote_id, "123");
        assert!(parsed.identity.data().external_ids.contains(&RemoteId { namespace:"imdb".into(), value:"tt1234567".into() }));
        assert_eq!(parsed.runtime, Some(95));
        for (key, expected) in [
            ("overview", json!("Public & actual plot.")), ("cast", json!([{"name":"Actor","role":"Hero","thumb":"https://img2.doubanio.com/view/personage/public/actor.jpg"}])),
            ("directors",json!(["Director"])), ("writers",json!(["Writer"])), ("genres",json!(["Drama","Comedy"])),
            ("rating",json!(8.2)), ("votes",json!(42)), ("countries",json!(["中国大陆","日本"])), ("premiered",json!("2024-02-03")),
        ] { assert_eq!(parsed.fields.iter().find(|field| field.key == key).map(|field| &field.value), Some(&expected), "{key}"); }
        assert_eq!(parsed.artwork[0].source_provider, "douban");
        assert!(matches!(subject(&movie(), "124", false), Err(ProviderFailure::IdentityMismatch)));
        assert!(matches!(subject(&movie(), "123", true), Err(ProviderFailure::IdentityMismatch)));
    }

    #[test]
    fn title_suggestions_do_not_mix_movies_with_television_or_follow_arbitrary_urls() {
        let rows = json!([
            {"id":"123","type":"movie","episode":"","title":"Film","url":"https://movie.douban.com/subject/123/?suggest=%E7%94%B5%E5%BD%B1","year":"2024","sub_title":"Original Film"},
            {"id":"124","type":"movie","episode":"12","title":"Series","url":"https://movie.douban.com/subject/124/","year":"2023"},
            {"id":"125","type":"book","title":"Book"}
        ]);
        assert_eq!(suggestions(&rows,false).unwrap().len(),1);
        assert_eq!(suggestions(&rows,true).unwrap()[0].data().remote_id,"124");
        let mut unsafe_rows=rows.clone();
        unsafe_rows[0]["url"]=json!("https://evil.example/subject/123/");
        assert_eq!(suggestions(&unsafe_rows,false),Err(ProviderFailure::UnsafeTarget));
        assert_eq!(suggestions(&json!(vec![rows[0].clone();51]),false),Err(ProviderFailure::ResponseTooLarge));
    }

    #[test]
    fn verification_invalid_images_and_subject_replacements_are_rejected() {
        assert!(matches!(subject("<html>安全验证</html>","123",false),Err(ProviderFailure::Captcha)));
        let changed=movie().replace("/subject/123/","/subject/124/");
        assert!(matches!(subject(&changed,"123",false),Err(ProviderFailure::IdentityMismatch)));
        for reference in ["https://img1.doubanio.com.evil.example/image.jpg","http://img1.doubanio.com/image.jpg","https://img1.doubanio.com/image.jpg?token=secret","https://img1.doubanio.com/%2e%2e/secret"] {
            assert_eq!(image_url(reference),Err(ProviderFailure::UnsafeTarget));
        }
        let tv=movie().replace("\"Movie\"","\"TVSeries\"");
        let parsed=subject(&tv,"123",true).unwrap();
        assert!(matches!(parsed.identity,VideoCandidate::Tv(_)));
    }

    #[tokio::test]
    async fn exact_subject_lookup_uses_public_path_and_rejects_unrelated_ids_without_searching() {
        use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![];
            loop {
                let mut chunk = [0_u8; 4096];
                let count = socket.read(&mut chunk).await.unwrap();
                if count == 0 { break; }
                request.extend_from_slice(&chunk[..count]);
                assert!(request.len() < 16384);
                if request.windows(4).any(|part| part == b"\r\n\r\n") { break; }
            }
            let body = movie();
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            socket.write_all(response.as_bytes()).await.unwrap();
            String::from_utf8(request).unwrap()
        });
        let mut hub = VideoProviderHub::new(Arc::new(NetworkEngine::for_loopback(2)), Arc::new(CredentialStore::new()));
        hub.douban_base = format!("http://{address}");
        let id = uuid::Uuid::new_v4();
        let job = ProviderJob { id, provider_id:"douban".into(), media_item_id:None,
            media_kind:Some(MediaKind::Movie), file_identity:None, region:None,
            correlation_id:id, operation:"search".into(), query:String::new(), language:"zh-CN".into(),
            config:ProviderConfig { enabled:true, max_retries:0, ..Default::default() }, created_at:chrono::Utc::now() };
        let query = VideoQuery::Movie { title:String::new(), year:None, language:"zh-CN".into(),
            external_ids:vec![RemoteId { namespace:"douban".into(), value:"123".into() }] };
        let candidates = hub.douban_search(&query,&job,&CancellationToken::new()).await.unwrap();
        assert_eq!(candidates[0].data().remote_id,"123");
        let request = server.await.unwrap();
        assert!(request.starts_with("GET /subject/123/ HTTP/1.1"));
        assert!(!request.to_ascii_lowercase().contains("cookie:"));
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
        let query = VideoQuery::Movie { title:"Do not guess this title".into(), year:None, language:"zh-CN".into(),
            external_ids:vec![RemoteId { namespace:"imdb".into(), value:"tt1234567".into() }] };
        assert_eq!(hub.douban_search(&query,&job,&CancellationToken::new()).await,Err(ProviderFailure::InvalidConfiguration));
    }
}
