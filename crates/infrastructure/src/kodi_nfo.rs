//! Kodi metadata XML projection; no filesystem or network authority.
use crate::library::RepositoryError;
use mediaatlas_domain::{ArtworkKind, ExternalId, MediaItem, MediaKind, MetadataValue};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

fn xml(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;")
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
        .or_else(|| value.as_i64().map(|value| value.to_string()))
        .or_else(|| value.get("normalized").and_then(Value::as_str).map(str::to_owned))
        .filter(|value| !value.trim().is_empty())
}

fn preferred<'a>(values: &'a [MetadataValue], keys: &[&str]) -> Option<&'a MetadataValue> {
    values.iter().filter(|value| keys.contains(&value.field_key.as_str()))
        .max_by_key(|value| (value.is_protected(), std::cmp::Reverse(keys.iter().position(|key| *key == value.field_key))))
}

fn element(output: &mut String, tag: &str, value: &str) {
    output.push_str(&format!("  <{tag}>{}</{tag}>\n", xml(value)));
}

fn https(value: &str) -> bool {
    value.trim() == value && !value.chars().any(char::is_control) && reqwest::Url::parse(value).is_ok_and(|url| {
        url.scheme() == "https" && url.host_str().is_some()
            && url.username().is_empty() && url.password().is_none() && url.fragment().is_none()
    })
}

fn relative(value: &str) -> bool {
    !value.is_empty() && !value.contains(['\\', ':', '?', '#', '%'])
        && value.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
        && !value.chars().any(char::is_control)
}

fn image_reference(value: &str) -> bool {
    https(value) || relative(value)
}

pub(crate) fn actor_thumbnail_urls(values: &[MetadataValue]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    preferred(values, &["cast", "people"])
        .and_then(|value| value.value.as_array())
        .into_iter().flatten()
        .filter_map(|person| person.get("thumb").and_then(Value::as_str))
        .filter(|url| image_reference(url) && seen.insert((*url).to_owned()))
        .map(str::to_owned).collect()
}

pub(crate) fn render_nfo(
    item: &MediaItem,
    values: &[MetadataValue],
    ids: &[ExternalId],
    artwork: &[(ArtworkKind, String)],
    actor_thumbs: Option<&BTreeMap<String, String>>,
) -> Result<String, RepositoryError> {
    let root = match item.kind {
        MediaKind::TvSeries | MediaKind::TvSeason => "tvshow",
        MediaKind::TvEpisode => "episodedetails",
        _ => "movie",
    };
    let adult = item.kind == MediaKind::AdultVideo;
    let mut fields = BTreeMap::new();
    fields.insert("title", item.title.clone());
    if let Some(title) = &item.original_title { fields.insert("originaltitle", title.clone()); }
    if let Some(year) = item.year { fields.insert("year", year.to_string()); }
    if adult {
        if let Some(id) = ids.iter().find(|id| id.is_primary) {
            fields.insert("catalognumber", id.external_id.clone());
            fields.insert("catalognamespace", id.namespace.clone());
        }
    }
    // Repository locale order is stable; protected values override provider values.
    for protected in [false, true] {
        for value in values.iter().filter(|value| value.is_protected() == protected) {
            if adult && value.field_key == "catalog_numbers" {
                if let Some(number) = value.value.as_array().and_then(|numbers| {
                    numbers.iter().find(|number| number.get("is_primary").and_then(Value::as_bool) == Some(true))
                        .or_else(|| numbers.first())
                }) {
                    for (key, tag) in [("normalized", "catalognumber"), ("namespace", "catalognamespace")] {
                        if let Some(value) = number.get(key).and_then(Value::as_str) {
                            fields.insert(tag, value.to_owned());
                        }
                    }
                }
            }
            let tag = match value.field_key.as_str() {
                "title" => "title",
                "original_title" => "originaltitle",
                "overview" => "plot",
                "outline" => "outline",
                "sort_title" | "sorttitle" => "sorttitle",
                "show_title" | "showtitle" => "showtitle",
                "runtime" | "runtime_minutes" => "runtime",
                "year" => "year",
                "season" => "season",
                "episode" => "episode",
                "premiered" => "premiered",
                "release_date" if !adult => "premiered",
                "aired" | "air_date" => "aired",
                "tagline" => "tagline",
                "mpaa" => "mpaa",
                "status" if matches!(item.kind, MediaKind::TvSeries | MediaKind::TvSeason) => "status",
                "attribution" => "sourceattribution",
                "source_license" => "sourcelicense",
                "catalog_number" if adult => "catalognumber",
                "namespace" | "catalog_namespace" if adult => "catalognamespace",
                "studio" if adult => "studio",
                "label" if adult => "label",
                "series" if adult => "set",
                "release_date" if adult => "releasedate",
                _ => continue,
            };
            if let Some(value) = text(&value.value) {
                if matches!(tag, "premiered" | "aired" | "releasedate")
                    && (value.len() != 10 || chrono::NaiveDate::parse_from_str(&value, "%Y-%m-%d").is_err()) {
                    continue;
                }
                if matches!(tag, "runtime" | "year" | "season" | "episode") && value.parse::<u32>().is_err() {
                    continue;
                }
                if matches!(tag, "sourceattribution" | "sourcelicense") {
                    let credit = fields.entry(tag).or_default();
                    if !credit.lines().any(|line| line == value) {
                        if !credit.is_empty() { credit.push('\n'); }
                        credit.push_str(&value);
                    }
                } else {
                    fields.insert(tag, value);
                }
            }
        }
    }
    let mut output = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<{root}>\n");
    for (tag, value) in fields { element(&mut output, tag, &value); }
    for (keys, tag) in [
        (&["genres"][..], "genre"),
        (&["countries"][..], "country"),
        (&["production_companies", "studios"][..], "studio"),
        (&["writers", "credits"][..], "credits"),
        (&["directors"][..], "director"),
        (&["tags"][..], "tag"),
    ] {
        if let Some(value) = preferred(values, keys) {
            let entries = value.value.as_array().cloned().unwrap_or_else(|| vec![value.value.clone()]);
            let mut seen = BTreeSet::new();
            for entry in entries.iter().take(if adult { 200 } else { 1000 }) {
                if let Some(name) = entry.get("name").and_then(Value::as_str).map(str::to_owned).or_else(|| text(entry)) {
                    if seen.insert(name.clone()) { element(&mut output, tag, &name); }
                }
            }
        }
    }
    if !adult {
        if let Some(collection) = preferred(values, &["collection"]).map(|value| &value.value) {
            if let Some(name) = collection.get("name").and_then(Value::as_str).filter(|name| !name.trim().is_empty()) {
                output.push_str(&format!("  <set><name>{}</name>", xml(name)));
                if let Some(overview) = collection.get("overview").and_then(Value::as_str).filter(|value| !value.trim().is_empty()) {
                    output.push_str(&format!("<overview>{}</overview>", xml(overview)));
                }
                output.push_str("</set>\n");
            }
        }
    }
    let rating = preferred(values, &["rating"]);
    let ratings = preferred(values, &["ratings"])
        .filter(|ratings| rating.is_none_or(|rating| !rating.is_protected() || ratings.is_protected()))
        .and_then(|value| value.value.as_array());
    if let Some(ratings) = ratings.filter(|ratings| !ratings.is_empty()) {
        let ratings: Vec<_> = ratings.iter().filter(|rating| {
            match (rating.get("value").and_then(Value::as_f64), rating.get("max").and_then(Value::as_f64)) {
                (Some(value), Some(max)) => value.is_finite() && max.is_finite() && max > 0.0 && (0.0..=max).contains(&value),
                _ => false,
            }
        }).take(100).collect();
        if !ratings.is_empty() {
            let primary = ratings.iter().position(|rating| rating.get("default").and_then(Value::as_bool) == Some(true)).unwrap_or(0);
            output.push_str("  <ratings>");
            for (index, rating) in ratings.iter().enumerate() {
                let name = rating.get("name").and_then(Value::as_str).filter(|name| !name.is_empty()).unwrap_or("local");
                output.push_str(&format!("<rating name=\"{}\" max=\"{}\" default=\"{}\"><value>{}</value>", xml(name), rating["max"].as_f64().unwrap(), index == primary, rating["value"]));
                if let Some(votes) = rating.get("votes").and_then(Value::as_u64) { output.push_str(&format!("<votes>{votes}</votes>")); }
                output.push_str("</rating>");
            }
            output.push_str("</ratings>\n");
        }
    } else if let Some(rating) = rating.filter(|value| value.value.as_f64().is_some_and(|value| value.is_finite() && (0.0..=10.0).contains(&value))) {
        let provider = match rating.provider_id.as_deref() { Some("tmdb") => "themoviedb", Some(provider) if !provider.is_empty() => provider, _ => "local" };
        output.push_str(&format!("  <ratings><rating name=\"{}\" max=\"10\" default=\"true\"><value>{}</value>", xml(provider), rating.value));
        if let Some(votes) = preferred(values, &["votes"]).and_then(|value| value.value.as_u64()) {
            output.push_str(&format!("<votes>{votes}</votes>"));
        }
        output.push_str("</rating></ratings>\n");
    }
    let cast_keys: &[&str] = if adult { &["performers"] } else { &["cast", "people"] };
    if let Some(cast) = preferred(values, cast_keys).and_then(|value| value.value.as_array()) {
        if !adult && cast.len() > 1000 { return Err(RepositoryError::InvalidInput); }
        for person in cast.iter().take(if adult { 200 } else { 1000 }) {
            let Some(name) = person.get("name").and_then(Value::as_str).or_else(|| person.as_str()).filter(|name| !name.trim().is_empty()) else { continue; };
            output.push_str(&format!("  <actor><name>{}</name>", xml(name)));
            if let Some(role) = person.get("role").and_then(Value::as_str).filter(|role| !role.trim().is_empty()) {
                output.push_str(&format!("<role>{}</role>", xml(role)));
            }
            if let Some(order) = person.get("order").and_then(Value::as_u64) {
                output.push_str(&format!("<order>{order}</order>"));
            }
            if let Some(source) = person.get("thumb").and_then(Value::as_str) {
                let reference = match actor_thumbs {
                    Some(references) => references.get(source).map(String::as_str).filter(|value| relative(value)),
                    None => Some(source).filter(|value| https(value)),
                };
                if let Some(reference) = reference { output.push_str(&format!("<thumb>{}</thumb>", xml(reference))); }
            }
            output.push_str("</actor>\n");
        }
    }
    let mut backdrops = Vec::new();
    for (kind, reference) in artwork.iter().filter(|(_, reference)| image_reference(reference)).take(100) {
        match kind {
            ArtworkKind::Backdrop => backdrops.push(reference),
            ArtworkKind::Poster | ArtworkKind::Cover | ArtworkKind::Thumb => {
                let aspect = if item.kind == MediaKind::TvEpisode || *kind == ArtworkKind::Thumb { "thumb" } else { "poster" };
                output.push_str(&format!("  <thumb aspect=\"{aspect}\">{}</thumb>\n", xml(reference)));
            }
            _ => {}
        }
    }
    if !backdrops.is_empty() {
        output.push_str("  <fanart>");
        for reference in backdrops { output.push_str(&format!("<thumb>{}</thumb>", xml(reference))); }
        output.push_str("</fanart>\n");
    }
    let primary = ids.iter().position(|id| id.is_primary)
        .or_else(|| ids.iter().position(|id| id.namespace == "tmdb"))
        .or_else(|| (!ids.is_empty()).then_some(0));
    for (index, id) in ids.iter().enumerate() {
        output.push_str(&format!("  <uniqueid type=\"{}\" default=\"{}\">{}</uniqueid>\n", xml(&id.namespace), primary == Some(index), xml(&id.external_id)));
    }
    if item.kind == MediaKind::TvSeries {
        let guide: BTreeMap<_, _> = ids.iter().filter(|id| matches!(id.namespace.as_str(), "tmdb" | "imdb" | "tvdb" | "tvmaze" | "tvrage"))
            .map(|id| (id.namespace.as_str(), id.external_id.as_str())).collect();
        if !guide.is_empty() { element(&mut output, "episodeguide", &serde_json::to_string(&guide).map_err(|_| RepositoryError::InvalidInput)?); }
    }
    output.push_str(&format!("</{root}>\n"));
    if output.len() > 2 * 1024 * 1024 || output.to_ascii_uppercase().contains("<!DOCTYPE") || output.to_ascii_uppercase().contains("<!ENTITY") {
        return Err(RepositoryError::InvalidInput);
    }
    roxmltree::Document::parse(&output).map_err(|_| RepositoryError::InvalidInput)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn movie() -> MediaItem {
        MediaItem { id: Uuid::new_v4(), kind: MediaKind::Movie, title: "A & B".into(), original_title: None, year: Some(2024), created_at: Utc::now(), updated_at: Utc::now() }
    }

    fn value(item: &MediaItem, key: &str, value: Value) -> MetadataValue {
        MetadataValue { id: Uuid::new_v4(), media_item_id: item.id, field_key: key.into(), locale: None, value, provider_id: Some("tmdb".into()), source_url: None, fetched_at: None, confidence: None, is_user_modified: false, is_locked: false }
    }

    #[test]
    fn exports_real_kodi_fields_and_round_trips_cast_without_loading_artwork() {
        let item = movie();
        let values = vec![value(&item, "cast", json!([{"name":"Actor & Name","role":"The <lead>","order":0,"thumb":"https://image.tmdb.org/t/p/original/actor.jpg"}])), value(&item,"genres",json!(["Drama","Thriller"])), value(&item,"production_companies",json!(["Studio"])), value(&item,"directors",json!(["Director"])), value(&item,"writers",json!(["Writer"])), value(&item,"countries",json!(["Japan"])), value(&item,"premiered",json!("2024-01-02")), value(&item,"rating",json!(8.4)), value(&item,"votes",json!(123)), value(&item,"collection",json!({"name":"Set & Series","overview":"Real set summary"}))];
        let nfo = render_nfo(&item, &values, &[], &[(ArtworkKind::Poster,"https://image.tmdb.org/t/p/original/poster.jpg".into()),(ArtworkKind::Backdrop,"https://image.tmdb.org/t/p/original/background.jpg".into())],None).unwrap();
        for expected in ["<title>A &amp; B</title>","<genre>Drama</genre>","<studio>Studio</studio>","<director>Director</director>","<credits>Writer</credits>","<country>Japan</country>","<role>The &lt;lead&gt;</role>","<order>0</order>","<rating name=\"themoviedb\" max=\"10\" default=\"true\"><value>8.4</value><votes>123</votes>","<set><name>Set &amp; Series</name><overview>Real set summary</overview></set>","<thumb aspect=\"poster\">https://image.tmdb.org/t/p/original/poster.jpg</thumb>","<fanart><thumb>https://image.tmdb.org/t/p/original/background.jpg</thumb></fanart>"] { assert!(nfo.contains(expected), "{expected}"); }
        let parsed = mediaatlas_media_scanner::parse_nfo(&nfo).unwrap();
        assert_eq!(parsed.fields["cast"][0]["role"], "The <lead>");
        assert_eq!(parsed.fields["people"][0], "Actor & Name");
        assert_eq!(parsed.fields["rating"], 8.4);
        assert_eq!(parsed.fields["votes"], 123);
        assert_eq!(parsed.fields["collection"]["name"], "Set & Series");
    }

    #[test]
    fn offline_references_never_keep_remote_actor_images_or_escape_the_export_folder() {
        let item = movie();
        let url = "https://image.tmdb.org/t/p/original/a.jpg";
        let values = [value(&item,"cast",json!([{"name":"Actor","thumb":url},{"name":"Missing image","thumb":"https://image.tmdb.org/t/p/original/missing.jpg"}]))];
        let thumbs = BTreeMap::from([(url.to_owned(), ".actors/person.jpg".to_owned())]);
        let output = render_nfo(&item,&values,&[],&[(ArtworkKind::Poster,"poster.jpg".into()),(ArtworkKind::Backdrop,"fanart.jpg".into()),(ArtworkKind::Poster,"../outside.jpg".into())],Some(&thumbs)).unwrap();
        assert!(output.contains("<thumb>.actors/person.jpg</thumb>"));
        assert!(output.contains("<thumb aspect=\"poster\">poster.jpg</thumb>"));
        assert!(!output.contains("https://"));
        assert!(!output.contains("outside.jpg"));
    }

    #[test]
    fn online_actor_images_are_https_only_while_offline_preparation_sees_safe_local_references() {
        let item = movie();
        let remote = "https://image.tmdb.org/t/p/original/actor.jpg";
        let values = [value(&item, "cast", json!([
            {"name":"Online actor","thumb":remote},
            {"name":"Local actor","thumb":"Movie-actor.jpg"},
            {"name":"Unsafe actor","thumb":"../outside.jpg"},
            {"name":"Authenticated image","thumb":"https://user:secret@example.com/actor.jpg"},
            {"name":"Insecure image","thumb":"http://example.com/actor.jpg"}
        ]))];
        let output = render_nfo(&item, &values, &[], &[], None).unwrap();
        let document = roxmltree::Document::parse(&output).unwrap();
        let thumbs: Vec<_> = document.descendants()
            .filter(|node| node.has_tag_name("actor"))
            .flat_map(|actor| actor.children().filter(|child| child.has_tag_name("thumb")))
            .filter_map(|thumb| thumb.text()).collect();
        assert_eq!(thumbs, vec![remote]);
        assert_eq!(actor_thumbnail_urls(&values), vec![remote.to_owned(), "Movie-actor.jpg".to_owned()]);
        assert!(!output.contains("Movie-actor.jpg"));
        assert!(!output.contains("outside.jpg"));
        assert!(!output.contains("user:secret"));
    }

    #[test]
    fn protected_metadata_and_legacy_cast_are_retained_without_fabricated_fields() {
        let item = movie();
        let mut protected = value(&item,"genres",json!(["Manual"]));
        protected.is_locked = true;
        let output = render_nfo(&item,&[protected,value(&item,"genres",json!(["Provider"])),value(&item,"cast",json!(["Legacy actor"]))],&[],&[],None).unwrap();
        assert!(output.contains("<genre>Manual</genre>"));
        assert!(!output.contains("<genre>Provider</genre>"));
        assert!(output.contains("<actor><name>Legacy actor</name></actor>"));
        assert!(!output.contains("<ratings>"));
        assert!(!output.contains("<playcount>"));
    }

    #[test]
    fn chooses_one_real_unique_id_and_uses_rich_cast_before_legacy_people() {
        let item = movie();
        let ids: Vec<_> = [("imdb", "tt123"), ("tmdb", "456")].into_iter().map(|(namespace, external_id)| ExternalId { id:Uuid::new_v4(), media_item_id:item.id, provider_id:namespace.into(), namespace:namespace.into(), external_id:external_id.into(), is_primary:false, created_at:Utc::now() }).collect();
        let source = "https://image.tmdb.org/t/p/original/actor.jpg";
        let values = [value(&item,"cast",json!([{"name":"Rich actor","role":"Lead","thumb":source},{"name":"Other","thumb":source}])),value(&item,"people",json!(["Rich actor","Other"]))];
        let output = render_nfo(&item,&values,&ids,&[],None).unwrap();
        assert!(output.contains("<role>Lead</role>"));
        assert_eq!(output.matches("default=\"true\"").count(),1);
        assert!(output.contains("<uniqueid type=\"tmdb\" default=\"true\">456</uniqueid>"));
        assert_eq!(actor_thumbnail_urls(&values),vec![source.to_owned()]);
    }

    #[test]
    fn rejects_invalid_xml_and_oversized_output_and_drops_unsafe_image_references() {
        let mut item = movie();
        let output = render_nfo(&item,&[],&[],&[(ArtworkKind::Poster,"http://example.com/a.jpg".into()),(ArtworkKind::Backdrop,"https://user:secret@example.com/a.jpg".into()),(ArtworkKind::Poster,"//example.com/a.jpg".into())],None).unwrap();
        assert!(!output.contains("<thumb"));
        item.title = "invalid\0title".into();
        assert_eq!(render_nfo(&item,&[],&[],&[],None),Err(RepositoryError::InvalidInput));
        item.title = "x".repeat(2*1024*1024);
        assert_eq!(render_nfo(&item,&[],&[],&[],None),Err(RepositoryError::InvalidInput));
    }
}
