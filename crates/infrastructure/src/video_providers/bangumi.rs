use super::*;
use crate::network::RequestMethod;

pub(super) fn descriptor() -> ProviderDescriptor {
    ProviderDescriptor {
        id: "bangumi".into(),
        name: "Bangumi".into(),
        description: "Public anime subjects, Chinese metadata and voice cast.".into(),
        website: Some("https://bgm.tv".into()),
        terms_url: Some("https://bgm.tv/about/copyright".into()),
        privacy_url: None,
        license: "Bangumi copyright policy; CC BY-SA metadata, existing artwork rights retained".into(),
        media_kinds: vec![MediaKind::Anime],
        languages: vec!["zh-CN".into(), "und".into()],
        capabilities: vec!["search".into(), "details".into(), "images".into(), "external_ids".into()],
        fixture: false,
        documentation_url: Some("https://bangumi.github.io/api/".into()),
        engine_types: vec!["api".into()],
        requires_api_key: false,
        requires_login: false,
        rate_limit_policy: "Enforced >= 1 s between requests, concurrency 1; honor 429 and Retry-After.".into(),
        attribution: "Bangumi contributors; https://bgm.tv; CC BY-SA, subject to existing artwork rights.".into(),
        data_usage_notes: "Default disabled. Public non-NSFW anime subjects only; source language is preserved, no episode-order mapping is invented.".into(),
    }
}

impl VideoProviderHub {
    async fn bangumi_json(
        &self,
        path: &str,
        body: Option<Value>,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Value, ProviderFailure> {
        let url = format!("{}{path}", self.bangumi_base);
        let mut request = RequestSpec::get(url, EngineKind::Api);
        if let Some(body) = body {
            request.method = RequestMethod::Post;
            request.json_body = Some(body);
        }
        request.user_agent = Some(format!("susanforme/MediaAtlas/{} (https://github.com/susanforme/MediaAtlas)", env!("CARGO_PKG_VERSION")));
        let mut effective = job.clone();
        effective.config.interval_ms = effective.config.interval_ms.max(1000);
        effective.config.concurrency = 1;
        effective.config.credential_ref = None;
        self.engine.execute(&effective, request, cancel).await?.json()
    }

    pub(super) async fn bangumi_search(
        &self,
        query: &VideoQuery,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<Vec<VideoCandidate>, ProviderFailure> {
        if !matches!(query, VideoQuery::Anime { .. }) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let (title, _, ids, language) = query.parts();
        if let Some(id) = ids.iter().find(|id| id.namespace == "bangumi") {
            numeric_id(&id.value)?;
            let value = self.bangumi_json(&format!("/v0/subjects/{}", id.value), None, job, cancel).await?;
            let candidate = identity(&value, language)?;
            if candidate.remote_id != id.value {
                return Err(ProviderFailure::IdentityMismatch);
            }
            return Ok(vec![VideoCandidate::Anime(candidate)]);
        }
        if !ids.is_empty() {
            return Ok(Vec::new());
        }
        let response = self.bangumi_json("/v0/search/subjects?limit=25&offset=0", Some(json!({
            "keyword":title,"sort":"match","filter":{"type":[2],"nsfw":false}
        })), job, cancel).await?;
        let rows = array(&response, "data")?;
        if rows.len() > 25 {
            return Err(ProviderFailure::ResponseTooLarge);
        }
        rows.iter().map(|value| identity(value, language).map(VideoCandidate::Anime)).collect()
    }

    pub(super) async fn bangumi_detail(
        &self,
        candidate: &VideoCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<VideoMetadata, ProviderFailure> {
        if !matches!(candidate, VideoCandidate::Anime(_)) {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let input = candidate.data();
        numeric_id(&input.remote_id)?;
        let value = self.bangumi_json(&format!("/v0/subjects/{}", input.remote_id), None, job, cancel).await?;
        let actual = identity(&value, &input.language)?;
        if actual.remote_id != input.remote_id {
            return Err(ProviderFailure::IdentityMismatch);
        }
        let characters = self.bangumi_json(&format!("/v0/subjects/{}/characters", input.remote_id), None, job, cancel).await?;
        metadata(&value, &characters, &input.remote_id, &input.language)
    }
}

fn infobox_values(value: &Value, keys: &[&str]) -> Result<Vec<String>, ProviderFailure> {
    let Some(rows) = value["infobox"].as_array() else { return Ok(Vec::new()); };
    if rows.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
    let mut result = Vec::new();
    for row in rows.iter().filter(|row| row["key"].as_str().is_some_and(|key| keys.contains(&key))) {
        let items = match row["value"].as_array() {
            Some(items) => items.clone(),
            None => vec![row["value"].clone()],
        };
        if items.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
        for item in &items {
            if let Some(name) = item.as_str().or_else(|| item["v"].as_str()).map(str::trim).filter(|name| !name.is_empty()) {
                if !result.iter().any(|existing| existing == name) { result.push(name.to_owned()); }
            }
        }
    }
    if result.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
    Ok(result)
}

fn identity(value: &Value, language: &str) -> Result<CandidateData, ProviderFailure> {
    if value["type"].as_u64() != Some(2) || value["nsfw"].as_bool() != Some(false) {
        return Err(ProviderFailure::IdentityMismatch);
    }
    let id = required_id(&value["id"])?;
    numeric_id(&id)?;
    let original = required_str(value, "name")?;
    let chinese = value["name_cn"].as_str().map(str::trim).filter(|name| !name.is_empty());
    let use_chinese = language.starts_with("zh") && chinese.is_some();
    let title = if use_chinese { chinese.unwrap() } else { original };
    let year = value["date"].as_str().filter(|date| !date.is_empty()).map(|date| {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|date| chrono::Datelike::year(&date)).map_err(|_| ProviderFailure::ParseError)
    }).transpose()?;
    if year.is_some_and(|year| !(1800..=2200).contains(&year)) { return Err(ProviderFailure::ParseError); }
    let mut aliases = infobox_values(value, &["别名"])?;
    for alias in [Some(original), chinese].into_iter().flatten() {
        if !aliases.iter().any(|existing| existing == alias) { aliases.push(alias.to_owned()); }
    }
    if aliases.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
    Ok(CandidateData {
        provider_id: "bangumi".into(), remote_id: id.clone(), title: title.into(),
        original_title: Some(original.into()), year, aliases,
        external_ids: vec![RemoteId { namespace: "bangumi".into(), value: id.clone() }],
        source_url: format!("https://bgm.tv/subject/{id}"),
        language: if use_chinese { "zh-CN" } else { "und" }.into(),
    })
}

fn thumbnail(value: &Value) -> Result<Option<&str>, ProviderFailure> {
    let Some(url) = value["large"].as_str().or_else(|| value["medium"].as_str()).filter(|url| !url.is_empty()) else { return Ok(None); };
    validate_image_url(url, &["lain.bgm.tv"])?;
    if url.contains(['?', '%', '\\']) || url.contains("..") || url.chars().any(char::is_control) {
        return Err(ProviderFailure::UnsafeTarget);
    }
    Ok(Some(url))
}

fn metadata(value: &Value, characters: &Value, expected: &str, language: &str) -> Result<VideoMetadata, ProviderFailure> {
    let identity = identity(value, language)?;
    if identity.remote_id != expected { return Err(ProviderFailure::IdentityMismatch); }
    let mut fields = standard_fields(&identity, None);
    if let Some(summary) = value["summary"].as_str().filter(|summary| !summary.trim().is_empty()) {
        fields.push(field("overview", json!(summary), Some("zh-CN")));
    }
    if let Some(date) = value["date"].as_str().filter(|date| !date.is_empty()) {
        fields.push(field("premiered", json!(date), None));
    }
    for (key, names) in [
        ("directors", &["导演", "監督", "总导演"][..]),
        ("writers", &["脚本", "脚本协力", "系列构成"][..]),
        ("studios", &["动画制作", "製作", "制作"][..]),
    ] {
        let names = infobox_values(value, names)?;
        if !names.is_empty() { fields.push(field(key, json!(names), None)); }
    }
    if let Some(score) = value["rating"]["score"].as_f64().filter(|score| (0.0..=10.0).contains(score)) {
        fields.push(field("rating", json!(score), None));
    }
    if let Some(votes) = value["rating"]["total"].as_u64() { fields.push(field("votes", json!(votes), None)); }
    if let Some(tags) = value["tags"].as_array() {
        if tags.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
        let tags = tags.iter().filter_map(|tag| tag["name"].as_str()).filter(|tag| !tag.trim().is_empty()).collect::<Vec<_>>();
        if !tags.is_empty() { fields.push(field("tags", json!(tags), Some("zh-CN"))); }
    }
    let characters = characters.as_array().ok_or(ProviderFailure::ParseError)?;
    if characters.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
    let mut cast = Vec::new();
    for character in characters {
        let role = required_str(character, "name")?;
        let actors = array(character, "actors")?;
        if actors.len() > 200 { return Err(ProviderFailure::ResponseTooLarge); }
        for actor in actors {
            if cast.len() >= 200 { return Err(ProviderFailure::ResponseTooLarge); }
            let mut person = json!({"name":required_str(actor,"name")?,"role":role,"order":cast.len()});
            if let Some(url) = thumbnail(&actor["images"])? { person["thumb"] = json!(url); }
            cast.push(person);
        }
    }
    if !cast.is_empty() { fields.push(field("cast", json!(cast), None)); }
    fields.push(field("aliases", json!(identity.aliases), None));
    fields.push(field("attribution", json!(format!("Bangumi contributors; {}; CC BY-SA; existing artwork rights retained", identity.source_url)), None));
    fields.push(field("source_license", json!("Bangumi copyright policy; https://bgm.tv/about/copyright"), None));
    let mut artwork = Vec::new();
    if let Some(url) = thumbnail(&value["images"])? {
        artwork.push(ArtworkCandidate { url: url.into(), source_provider: "bangumi".into(), kind: "poster".into(), language: None });
    }
    let format = value["platform"].as_str().filter(|format| !format.is_empty()).map(str::to_owned);
    if let Some(format) = &format { fields.push(field("format", json!(format), None)); }
    let episodes = u32_value(&value["eps"])?.filter(|count| *count > 0);
    if let Some(episodes) = episodes { fields.push(field("episodes", json!(episodes), None)); }
    Ok(VideoMetadata::Anime(AnimeDetail {
        aliases: identity.aliases.clone(), identity, fields, artwork,
        format, season: None, episodes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> Value {
        json!({"id":12,"type":2,"nsfw":false,"name":"Fixture Anime","name_cn":"架空动画",
            "summary":"中文简介","date":"2024-04-01","platform":"TV","eps":12,
            "rating":{"score":8.3,"total":123},"tags":[{"name":"科幻"}],
            "infobox":[{"key":"导演","value":"Director"},{"key":"脚本","value":[{"v":"Writer"}]},
                {"key":"动画制作","value":"Studio"},{"key":"别名","value":[{"v":"Alias"}]}],
            "images":{"large":"https://lain.bgm.tv/pic/cover/l/fixture.jpg"}})
    }

    #[test]
    fn maps_real_subject_metadata_and_voice_actors_without_inventing_episode_order() {
        let characters = json!([{"name":"Lead character","actors":[{"name":"Voice actor","images":{"large":"https://lain.bgm.tv/pic/crt/l/actor.jpg"}}]}]);
        let VideoMetadata::Anime(detail) = metadata(&subject(), &characters, "12", "zh-CN").unwrap() else { panic!("anime expected"); };
        assert_eq!(detail.identity.title, "架空动画");
        assert_eq!(detail.identity.original_title.as_deref(), Some("Fixture Anime"));
        assert_eq!(detail.identity.language, "zh-CN");
        assert_eq!(detail.episodes, Some(12));
        assert!(detail.season.is_none());
        for (key, expected) in [("directors",json!(["Director"])),("writers",json!(["Writer"])),("studios",json!(["Studio"])),("rating",json!(8.3)),("votes",json!(123)),("tags",json!(["科幻"]))] {
            assert_eq!(detail.fields.iter().find(|field|field.key==key).unwrap().value,expected);
        }
        let cast = &detail.fields.iter().find(|field|field.key=="cast").unwrap().value;
        assert_eq!(cast[0]["name"], "Voice actor");
        assert_eq!(cast[0]["role"], "Lead character");
        assert!(detail.fields.iter().any(|field|field.key=="source_license"));
        assert_eq!(detail.artwork[0].source_provider, "bangumi");
        assert_eq!(identity(&subject(), "en-US").unwrap().language, "und");
    }

    #[test]
    fn rejects_adult_missing_adult_flag_identity_mismatch_unsafe_images_and_oversized_cast() {
        let mut value = subject();
        value["nsfw"] = json!(true);
        assert_eq!(identity(&value, "zh-CN"), Err(ProviderFailure::IdentityMismatch));
        value["nsfw"] = Value::Null;
        assert_eq!(identity(&value, "zh-CN"), Err(ProviderFailure::IdentityMismatch));
        value = subject();
        value["type"] = json!(6);
        assert_eq!(identity(&value,"zh-CN"),Err(ProviderFailure::IdentityMismatch));
        assert_eq!(metadata(&subject(),&json!([]),"13","zh-CN"),Err(ProviderFailure::IdentityMismatch));
        for url in ["http://lain.bgm.tv/a.jpg","https://evil.example/a.jpg","https://lain.bgm.tv/a.jpg?token=x","https://lain.bgm.tv/%2e%2e/a.jpg"] {
            value=subject(); value["images"]["large"]=json!(url);
            assert_eq!(metadata(&value,&json!([]),"12","zh-CN"),Err(ProviderFailure::UnsafeTarget));
        }
        let characters=json!([{"name":"Role","actors":vec![json!({"name":"Actor"});201]}]);
        assert_eq!(metadata(&subject(),&characters,"12","zh-CN"),Err(ProviderFailure::ResponseTooLarge));
    }

    #[tokio::test]
    async fn mock_search_and_exact_detail_use_non_nsfw_filter_and_no_credentials() {
        use chrono::Utc;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::sync::Mutex;
        use uuid::Uuid;
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base=format!("http://{}",listener.local_addr().unwrap());
        let requests=Arc::new(Mutex::new(Vec::new()));
        let saved=requests.clone();
        let server=tokio::spawn(async move {
            loop {
                let Ok((mut stream,_))=listener.accept().await else { break; };
                let saved=saved.clone();
                tokio::spawn(async move {
                    let mut bytes=Vec::new();
                    loop {
                        let mut chunk=[0;4096];
                        let count=stream.read(&mut chunk).await.unwrap();
                        if count==0 { return; }
                        bytes.extend_from_slice(&chunk[..count]);
                        assert!(bytes.len()<=16384);
                        if let Some(end)=bytes.windows(4).position(|bytes|bytes==b"\r\n\r\n") {
                            let headers=String::from_utf8_lossy(&bytes[..end]);
                            let length=headers.lines().find_map(|line|line.split_once(':').filter(|(key,_)|key.eq_ignore_ascii_case("content-length")).and_then(|(_,value)|value.trim().parse::<usize>().ok())).unwrap_or(0);
                            if bytes.len()>=end+4+length { break; }
                        }
                    }
                    let request=String::from_utf8(bytes).unwrap();
                    let path=request.lines().next().unwrap().split_whitespace().nth(1).unwrap();
                    let body=if path.starts_with("/v0/search/subjects?") { json!({"data":[subject()]}) }
                        else if path.ends_with("/characters") { json!([]) } else { subject() }.to_string();
                    saved.lock().await.push(request);
                    let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        let mut hub=VideoProviderHub::new(Arc::new(NetworkEngine::for_loopback(1)),Arc::new(CredentialStore::new()));
        hub.bangumi_base=base;
        let job=ProviderJob { id:Uuid::new_v4(),provider_id:"bangumi".into(),media_item_id:None,media_kind:Some(MediaKind::Anime),file_identity:None,region:None,
            correlation_id:Uuid::new_v4(),operation:"search".into(),query:"Fixture".into(),language:"zh-CN".into(),
            config:ProviderConfig {enabled:true,interval_ms:0,max_retries:0,..Default::default()},created_at:Utc::now() };
        let cancel=CancellationToken::new();
        let query=VideoQuery::Anime {title:"Fixture".into(),year:None,external_ids:vec![],language:"zh-CN".into()};
        let candidates=hub.bangumi_search(&query,&job,&cancel).await.unwrap();
        assert_eq!(candidates.len(),1);
        let exact=VideoQuery::Anime {title:String::new(),year:None,external_ids:vec![RemoteId {namespace:"bangumi".into(),value:"12".into()}],language:"zh-CN".into()};
        let precise=hub.bangumi_search(&exact,&job,&cancel).await.unwrap();
        assert_eq!(precise[0].data().remote_id,"12");
        hub.bangumi_detail(&precise[0],&job,&cancel).await.unwrap();
        let foreign=VideoQuery::Anime {title:"Wrong".into(),year:None,external_ids:vec![RemoteId {namespace:"imdb".into(),value:"tt123".into()}],language:"zh-CN".into()};
        assert!(hub.bangumi_search(&foreign,&job,&cancel).await.unwrap().is_empty());
        let requests=requests.lock().await;
        assert_eq!(requests.len(),4);
        assert!(requests[0].starts_with("POST /v0/search/subjects?limit=25&offset=0 "));
        let (_,body)=requests[0].split_once("\r\n\r\n").unwrap();
        let body:Value=serde_json::from_str(body).unwrap();
        assert_eq!(body["filter"],json!({"type":[2],"nsfw":false}));
        assert!(requests.iter().all(|request|!request.to_ascii_lowercase().contains("authorization:")));
        assert!(requests[0].contains("susanforme/MediaAtlas/"));
        server.abort();
    }
}
