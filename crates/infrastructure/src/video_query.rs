//! Convert user-supplied identity hints to IDs; input URLs are never fetched.
use mediaatlas_domain::{ProviderFailure, RemoteId, VideoQuery};
use reqwest::Url;

fn invalid<T>() -> Result<T, ProviderFailure> {
    Err(ProviderFailure::InvalidConfiguration)
}

fn numeric(value: &str) -> bool {
    !value.is_empty() && value.len() <= 12 && value.bytes().all(|byte| byte.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|value| value > 0)
}

fn normalize_id(namespace: &str, value: &str) -> Result<RemoteId, ProviderFailure> {
    if value.len() > 2048 || value.chars().any(char::is_control) { return invalid(); }
    let namespace = namespace.trim().to_ascii_lowercase();
    if namespace.len() > 32 || !namespace.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-_".contains(&byte)) { return invalid(); }
    let value = value.trim();
    let mut detected = None;
    let mut id = value.to_owned();
    let host_like = value.split('/').next().is_some_and(|part| part.contains('.'));
    if value.contains("://") || host_like {
        let address_path = value.split(['?', '#']).next().unwrap_or_default();
        if value.contains('\\') || address_path.contains('%') || address_path.split('/').any(|part| matches!(part, "." | "..")) { return invalid(); }
        let address = if value.contains("://") { value.to_owned() } else { format!("https://{value}") };
        let url = Url::parse(&address).map_err(|_| ProviderFailure::InvalidConfiguration)?;
        if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty()
            || url.password().is_some() || url.port().is_some() { return invalid(); }
        let parts = url.path_segments().ok_or(ProviderFailure::InvalidConfiguration)?
            .filter(|part| !part.is_empty()).collect::<Vec<_>>();
        let host = url.host_str().unwrap_or_default();
        let (source, segment) = match host {
            "imdb.com" | "www.imdb.com" | "m.imdb.com" if parts.first() == Some(&"title") => ("imdb", 1),
            "themoviedb.org" | "www.themoviedb.org" if matches!(parts.first().copied(), Some("movie" | "tv")) => ("tmdb", 1),
            "movie.douban.com" if parts.first() == Some(&"subject") => ("douban", 1),
            "bgm.tv" | "bangumi.tv" | "chii.in" if parts.first() == Some(&"subject") => ("bangumi", 1),
            "anilist.co" | "www.anilist.co" if parts.first() == Some(&"anime") => ("anilist", 1),
            "myanimelist.net" | "www.myanimelist.net" if parts.first() == Some(&"anime") => ("mal", 1),
            "tvmaze.com" | "www.tvmaze.com" if parts.first() == Some(&"shows") => ("tvmaze", 1),
            _ => return invalid(),
        };
        id = parts.get(segment).ok_or(ProviderFailure::InvalidConfiguration)?.to_string();
        if source == "tmdb" { id = id.split('-').next().unwrap_or_default().to_owned(); }
        detected = Some(source.to_owned());
    } else if let Some((source, raw)) = value.split_once(':') {
        if !matches!(source.to_ascii_lowercase().as_str(), "imdb" | "tmdb" | "douban" | "bangumi" | "anilist" | "mal" | "tvmaze" | "tvdb") {
            return invalid();
        }
        detected = Some(source.to_ascii_lowercase());
        id = raw.trim().to_owned();
    } else if value.to_ascii_lowercase().starts_with("tt") {
        detected = Some("imdb".into());
    }
    if let Some(detected) = &detected {
        if !namespace.is_empty() && &namespace != detected { return invalid(); }
    }
    let namespace = detected.unwrap_or(namespace);
    match namespace.as_str() {
        "imdb" => {
            id.make_ascii_lowercase();
            if numeric(&id) { id = format!("tt{id}"); }
            if !id.strip_prefix("tt").is_some_and(numeric) { return invalid(); }
        }
        "tmdb" | "douban" | "bangumi" | "anilist" | "mal" | "tvmaze" | "tvdb" => {
            if !numeric(&id) { return invalid(); }
        }
        _ => {
            if namespace.is_empty() || namespace.len() > 32 || id.is_empty() || id.len() > 128
                || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)) {
                return invalid();
            }
        }
    }
    Ok(RemoteId { namespace, value: id })
}

pub(crate) fn normalize(query: &VideoQuery) -> Result<VideoQuery, ProviderFailure> {
    let mut query = query.clone();
    let (title, ids) = match &mut query {
        VideoQuery::Movie { title, external_ids, .. }
        | VideoQuery::Tv { title, external_ids, .. }
        | VideoQuery::Anime { title, external_ids, .. } => (title, external_ids),
    };
    if ids.len() > 8 || title.len() > 2048 || title.chars().any(char::is_control) { return invalid(); }
    let mut normalized = Vec::<RemoteId>::new();
    for id in ids.iter() {
        let id = normalize_id(&id.namespace, &id.value)?;
        if normalized.iter().any(|known| known.namespace == id.namespace && known != &id) { return invalid(); }
        if !normalized.contains(&id) { normalized.push(id); }
    }
    let input = title.trim();
    let lower = input.to_ascii_lowercase();
    let identity = input.contains("://") || lower.starts_with("www.")
        || ["movie.douban.com/", "imdb.com/", "m.imdb.com/", "bgm.tv/", "bangumi.tv/", "chii.in/", "themoviedb.org/", "anilist.co/", "myanimelist.net/", "tvmaze.com/"].iter()
            .any(|host| lower.starts_with(host))
        || input.to_ascii_lowercase().strip_prefix("tt").is_some_and(numeric)
        || ["imdb:", "tmdb:", "douban:", "bangumi:", "anilist:", "mal:", "tvmaze:", "tvdb:"].iter()
            .any(|prefix| input.to_ascii_lowercase().starts_with(prefix));
    if identity {
        let id = normalize_id("", input)?;
        if normalized.iter().any(|known| known.namespace == id.namespace && known != &id) { return invalid(); }
        if !normalized.contains(&id) { normalized.push(id); }
        title.clear();
    } else { *title = input.to_owned(); }
    *ids = normalized;
    Ok(query)
}

pub(crate) fn supports_id(provider: &str, namespace: &str) -> bool {
    match provider {
        "tmdb" => matches!(namespace, "tmdb" | "imdb" | "tvdb"),
        "omdb" => namespace == "imdb",
        "tvmaze" => matches!(namespace, "tvmaze" | "imdb" | "tvdb"),
        "anilist" => matches!(namespace, "anilist" | "mal"),
        "bangumi" => namespace == "bangumi",
        "douban" => namespace == "douban",
        "wikipedia.html" => namespace == "wikipedia.en",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(title: &str, namespace: &str, id: &str) -> VideoQuery {
        VideoQuery::Movie { title: title.into(), year: None, language: "zh-CN".into(),
            external_ids: if id.is_empty() { vec![] } else { vec![RemoteId { namespace: namespace.into(), value: id.into() }] } }
    }

    #[test]
    fn imdb_links_and_ids_work_in_both_input_locations() {
        for input in ["tt0133093", "TT0133093", "imdb:tt0133093", "https://www.imdb.com/title/tt0133093/?ref_=search%3Dtracking", "http://m.imdb.com/title/tt0133093/fullcredits/", "imdb.com/title/tt0133093"] {
            for original in [query(input,"",""), query("Wrong original title","",input)] {
                let normalized = normalize(&original).unwrap();
                assert_eq!(normalized.parts().2, &[RemoteId { namespace:"imdb".into(), value:"tt0133093".into() }]);
                assert_eq!(normalize(&normalized).unwrap(), normalized);
            }
        }
        assert_eq!(normalize(&query("", "IMDb", "0133093")).unwrap().parts().2[0].value, "tt0133093");
    }

    #[test]
    fn provider_addresses_are_ids_and_never_arbitrary_fetch_targets() {
        for (address, namespace, id) in [
            ("https://movie.douban.com/subject/1291843/", "douban", "1291843"),
            ("https://www.themoviedb.org/movie/603-the-matrix", "tmdb", "603"),
            ("https://bgm.tv/subject/253", "bangumi", "253"),
        ] {
            assert_eq!(normalize(&query(address,"","")).unwrap().parts().2[0], RemoteId { namespace:namespace.into(), value:id.into() });
        }
        for address in ["https://imdb.com.evil.test/title/tt0133093", "https://www.imdb.com@evil.test/title/tt0133093", "https://www.imdb.com/name/nm123", "https://127.0.0.1/title/tt123", "https://www.imdb.com/title/tt%31", "https://user:secret@www.imdb.com/title/tt123", "https://www.imdb.com/title/../title/tt123"] {
            assert!(normalize(&query(address,"","")).is_err(), "{address}");
        }
        assert!(normalize(&query("tt123", "imdb", "tt456")).is_err());
        assert!(normalize(&query("", "douban", "https://www.imdb.com/title/tt123")).is_err());
    }
}
