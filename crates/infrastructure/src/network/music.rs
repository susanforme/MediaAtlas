//! Built-in music destinations are constrained at every HTTP redirect hop.
use mediaatlas_domain::ProviderFailure;
use reqwest::Url;
use uuid::Uuid;

pub(crate) fn validate_music_destination(
    provider: &str,
    original: &Url,
    current: &Url,
) -> Result<(), ProviderFailure> {
    if !matches!(provider, "musicbrainz" | "coverartarchive") {
        return Ok(());
    }
    #[cfg(test)]
    if original.host_str() == Some("127.0.0.1") && current.origin() == original.origin() {
        return Ok(());
    }
    if current.scheme() != "https"
        || current.port().is_some_and(|p| p != 443)
        || current.fragment().is_some()
        || !current.username().is_empty()
        || current.password().is_some()
    {
        return Err(ProviderFailure::UnsafeTarget);
    }
    if provider == "musicbrainz" {
        return if current.host_str() == Some("musicbrainz.org")
            && current.path().starts_with("/ws/2/")
        {
            Ok(())
        } else {
            Err(ProviderFailure::UnsafeTarget)
        };
    }
    let original_parts = original
        .path_segments()
        .ok_or(ProviderFailure::UnsafeTarget)?
        .collect::<Vec<_>>();
    let mbid = original_parts
        .get(1)
        .filter(|_| original_parts.first() == Some(&"release"))
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(ProviderFailure::UnsafeTarget)?;
    if current.query().is_some() {
        return Err(ProviderFailure::UnsafeTarget);
    }
    let parts = current
        .path_segments()
        .ok_or(ProviderFailure::UnsafeTarget)?
        .collect::<Vec<_>>();
    if current.host_str() == Some("coverartarchive.org") {
        if parts.len() >= 2
            && parts[0] == "release"
            && Uuid::parse_str(parts[1]).ok() == Some(mbid)
            && parts.len() <= 3
        {
            return Ok(());
        }
        return Err(ProviderFailure::UnsafeTarget);
    }
    let host = current.host_str().unwrap_or_default();
    let numbered_archive = host
        .strip_prefix("ia")
        .and_then(|s| {
            s.strip_suffix(".us.archive.org")
                .or_else(|| s.strip_suffix(".archive.org"))
        })
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
    if host != "archive.org" && !numbered_archive {
        return Err(ProviderFailure::UnsafeTarget);
    }
    let item = format!("mbid-{mbid}");
    let valid_path = (parts.len() == 3 && parts[0] == "download" && parts[1] == item)
        || (parts.len() == 4
            && !parts[0].is_empty()
            && parts[0].bytes().all(|b| b.is_ascii_digit())
            && parts[1] == "items"
            && parts[2] == item);
    if !valid_path
        || !parts.last().is_some_and(|name| {
            name.starts_with(&item)
                && name.ends_with(".jpg")
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return Err(ProviderFailure::UnsafeTarget);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redirects_keep_release_identity_and_restrict_archive_resources() {
        let original = Url::parse(
            "https://coverartarchive.org/release/11111111-1111-4111-8111-111111111111/front",
        )
        .unwrap();
        for target in ["https://archive.org/download/mbid-11111111-1111-4111-8111-111111111111/mbid-11111111-1111-4111-8111-111111111111-1.jpg", "https://ia800123.us.archive.org/0/items/mbid-11111111-1111-4111-8111-111111111111/mbid-11111111-1111-4111-8111-111111111111-1_thumb250.jpg"] {
            assert!(validate_music_destination("coverartarchive",&original,&Url::parse(target).unwrap()).is_ok());
        }
        for target in ["http://coverartarchive.org/release/11111111-1111-4111-8111-111111111111/front", "https://evil.archive.org/download/x", "https://archive.org/download/unrelated/x.jpg", "https://ia800123.us.archive.org/0/items/mbid-22222222-2222-4222-8222-222222222222/a.jpg", "https://example.org/picture.jpg", "https://archive.org/download/mbid-11111111-1111-4111-8111-111111111111/unrelated.jpg"] {
            assert_eq!(validate_music_destination("coverartarchive",&original,&Url::parse(target).unwrap()),Err(ProviderFailure::UnsafeTarget));
        }
    }
}
