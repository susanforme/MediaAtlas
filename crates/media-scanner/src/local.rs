use crate::{LocalExternalId, LocalMetadata, ScanError};
use lofty::{config::ParseOptions, file::TaggedFileExt, prelude::Accessor, probe::Probe};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

const MAX_NFO_BYTES: u64 = 2 * 1024 * 1024;

pub fn read_nfo(path: &Path) -> Result<LocalMetadata, ScanError> {
    let info = fs::symlink_metadata(path).map_err(|_| ScanError::Io)?;
    if !info.is_file() || info.file_type().is_symlink() {
        return Err(ScanError::Io);
    }
    if info.len() > MAX_NFO_BYTES {
        return Err(ScanError::MetadataTooLarge);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| ScanError::Io)?
        .take(MAX_NFO_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ScanError::Io)?;
    if bytes.len() as u64 > MAX_NFO_BYTES {
        return Err(ScanError::MetadataTooLarge);
    }
    let content = std::str::from_utf8(&bytes).map_err(|_| ScanError::InvalidMetadata)?;
    parse_nfo(content)
}

/// Parse an already bounded UTF-8 snapshot without filesystem or network access.
pub fn parse_nfo(content: &str) -> Result<LocalMetadata, ScanError> {
    if content.len() as u64 > MAX_NFO_BYTES {
        return Err(ScanError::MetadataTooLarge);
    }
    let upper = content.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err(ScanError::UnsafeXml);
    }
    let document = roxmltree::Document::parse(content).map_err(|_| ScanError::InvalidMetadata)?;
    let root = document.root_element();
    if !matches!(
        root.tag_name().name(),
        "movie" | "tvshow" | "episodedetails" | "musicvideo" | "album" | "artist"
    ) {
        return Err(ScanError::InvalidMetadata);
    }
    let mut metadata = LocalMetadata::default();
    let mut genres = Vec::new();
    let mut people = Vec::new();
    let mut cast = Vec::new();
    let mut studios = Vec::new();
    let mut countries = Vec::new();
    let mut writers = Vec::new();
    let mut tags = Vec::new();
    let mut artwork = Vec::new();
    for child in root.children().filter(roxmltree::Node::is_element) {
        if child.has_tag_name("actor") {
            if let Some(name) = nfo_child_text(child, "name") {
                let mut person = serde_json::Map::new();
                person.insert("name".into(), json!(name));
                for key in ["role", "thumb"] {
                    if let Some(value) = nfo_child_text(child, key) {
                        person.insert(key.into(), json!(value));
                    }
                }
                if let Some(order) = nfo_child_text(child, "order").and_then(|value| value.parse::<u32>().ok()) {
                    person.insert("order".into(), json!(order));
                }
                people.push(json!(name));
                cast.push(Value::Object(person));
            }
            continue;
        }
        if child.has_tag_name("ratings") {
            let mut ratings = Vec::new();
            for rating in child.children().filter(|node| node.has_tag_name("rating")) {
                let Some(value) = nfo_child_text(rating, "value").and_then(|value| value.parse::<f64>().ok()).filter(|value| value.is_finite() && *value >= 0.0) else { continue; };
                let max = rating.attribute("max").unwrap_or("10").parse::<f64>().ok().filter(|max| max.is_finite() && *max > 0.0);
                let Some(max) = max.filter(|max| value <= *max) else { continue; };
                let mut entry = json!({"name":rating.attribute("name").unwrap_or("local"),"value":value,"max":max,"default":rating.attribute("default") == Some("true")});
                if let Some(votes) = nfo_child_text(rating, "votes").and_then(|value| value.parse::<u64>().ok()) {
                    entry["votes"] = json!(votes);
                }
                ratings.push(entry);
            }
            if let Some(rating) = ratings.iter().find(|rating| rating["default"] == true).or_else(|| ratings.first()) {
                metadata.fields.insert("rating".into(), json!(rating["value"].as_f64().unwrap() * 10.0 / rating["max"].as_f64().unwrap()));
                if let Some(votes) = rating.get("votes") { metadata.fields.insert("votes".into(), votes.clone()); }
            }
            if !ratings.is_empty() { metadata.fields.insert("ratings".into(), json!(ratings)); }
            continue;
        }
        if child.has_tag_name("set") {
            if let Some(name) = nfo_child_text(child, "name").or_else(|| child.text().map(str::trim).filter(|value| !value.is_empty())) {
                let mut collection = json!({"name":name});
                if let Some(overview) = nfo_child_text(child, "overview") { collection["overview"] = json!(overview); }
                metadata.fields.insert("collection".into(), collection);
            }
            continue;
        }
        if child.has_tag_name("fanart") {
            for image in child.children().filter(|node| node.has_tag_name("thumb")) {
                if let Some(url) = image.text().map(str::trim).filter(|value| !value.is_empty()) {
                    artwork.push(json!({"kind":"backdrop","url":url}));
                }
            }
            continue;
        }
        let text = child
            .text()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(value) = text else { continue; };
        match child.tag_name().name() {
            "title" | "originaltitle" | "plot" | "outline" | "premiered" | "aired" | "album"
            | "artist" | "tagline" | "mpaa" | "sorttitle" | "showtitle" | "status"
            | "sourceattribution" | "sourcelicense" => {
                let key = match child.tag_name().name() {
                    "originaltitle" => "original_title",
                    "plot" => "overview",
                    "sorttitle" => "sort_title",
                    "showtitle" => "show_title",
                    "sourceattribution" => "attribution",
                    "sourcelicense" => "source_license",
                    key => key,
                };
                metadata.fields.insert(key.to_owned(), json!(value));
            }
            "year" | "season" | "episode" | "runtime" | "track" => {
                if let Ok(number) = value.parse::<u32>() {
                    metadata
                        .fields
                        .insert(child.tag_name().name().to_owned(), json!(number));
                }
            }
            "genre" => genres.push(json!(value)),
            "studio" => studios.push(json!(value)),
            "country" => countries.push(json!(value)),
            "credits" => writers.push(json!(value)),
            "tag" => tags.push(json!(value)),
            "rating" => {
                if let Some(rating) = value.parse::<f64>().ok().filter(|value| value.is_finite() && (0.0..=10.0).contains(value)) {
                    metadata.fields.insert("rating".into(), json!(rating));
                }
            }
            "votes" => {
                if let Ok(votes) = value.replace(',', "").parse::<u64>() {
                    metadata.fields.insert("votes".into(), json!(votes));
                }
            }
            "thumb" => {
                let kind = match child.attribute("aspect") { Some("thumb") => "thumb", Some("poster") | None => "poster", _ => continue };
                artwork.push(json!({"kind":kind,"url":value}));
            }
            "director" => {
                metadata
                    .fields
                    .entry("directors".into())
                    .or_insert_with(|| json!([]))
                    .as_array_mut()
                    .unwrap()
                    .push(json!(value));
            }
            "imdbid" | "tmdbid" | "tvdbid" => {
                let provider = child.tag_name().name().trim_end_matches("id");
                metadata.external_ids.push(LocalExternalId {
                    provider_id: provider.to_owned(),
                    namespace: provider.to_owned(),
                    value: value.to_owned(),
                });
            }
            "uniqueid" => {
                if let Some(provider) = child
                    .attribute("type")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    metadata.external_ids.push(LocalExternalId {
                        provider_id: provider.to_ascii_lowercase(),
                        namespace: provider.to_ascii_lowercase(),
                        value: value.to_owned(),
                    });
                }
            }
            "id" => metadata.external_ids.push(LocalExternalId {
                provider_id: "local_nfo".into(),
                namespace: "unverified".into(),
                value: value.to_owned(),
            }),
            _ => {}
        }
    }
    if !genres.is_empty() {
        metadata.fields.insert("genres".into(), json!(genres));
    }
    if !people.is_empty() {
        metadata.fields.insert("people".into(), json!(people));
        metadata.fields.insert("cast".into(), json!(cast));
    }
    if !studios.is_empty() {
        metadata.fields.insert("studios".into(), json!(studios));
    }
    for (key, values) in [("countries", countries), ("writers", writers), ("tags", tags), ("nfo_artwork", artwork)] {
        if !values.is_empty() { metadata.fields.insert(key.into(), json!(values)); }
    }
    metadata.external_ids.sort_by(|a, b| {
        (&a.provider_id, &a.namespace, &a.value).cmp(&(&b.provider_id, &b.namespace, &b.value))
    });
    metadata.external_ids.dedup();
    Ok(metadata)
}

fn nfo_child_text<'a, 'input: 'a>(node: roxmltree::Node<'a, 'input>, name: &str) -> Option<&'a str> {
    node.children().find(|child| child.has_tag_name(name))
        .and_then(|child| child.text()).map(str::trim).filter(|value| !value.is_empty())
}

/// Read tags without embedded cover-art or media-properties decoding; never save.
pub fn read_audio_tags(path: &Path) -> Result<LocalMetadata, ScanError> {
    let info = fs::symlink_metadata(path).map_err(|_| ScanError::Io)?;
    if !info.is_file() || info.file_type().is_symlink() {
        return Err(ScanError::Io);
    }
    let tagged = Probe::open(path)
        .map_err(|_| ScanError::Io)?
        .options(
            ParseOptions::new()
                .read_properties(false)
                .read_cover_art(false),
        )
        .read()
        .map_err(|_| ScanError::InvalidMetadata)?;
    let tag = tagged
        .primary_tag()
        .or_else(|| tagged.first_tag())
        .ok_or(ScanError::InvalidMetadata)?;
    let mut metadata = LocalMetadata::default();
    for (key, value) in [
        ("title", tag.title()),
        ("artist", tag.artist()),
        ("album", tag.album()),
        ("genre", tag.genre()),
    ] {
        if let Some(value) = value.filter(|v| !v.trim().is_empty()) {
            metadata.fields.insert(key.into(), json!(value));
        }
    }
    if let Some(value) = tag.year() {
        metadata.fields.insert("year".into(), json!(value));
    }
    if let Some(value) = tag.track() {
        metadata.fields.insert("track_number".into(), json!(value));
    }
    if let Some(value) = tag.disk() {
        metadata.fields.insert("disc_number".into(), json!(value));
    }
    Ok(metadata)
}

#[cfg(test)]
mod nfo_tests {
    use super::*;

    #[test]
    fn retains_kodi_cast_ratings_collection_and_image_references_as_data_only() {
        let nfo = r#"<movie><title>Movie &amp; title</title><actor><name>Actor &amp; name</name><role>Lead</role><order>0</order><thumb>Movie-actor.jpg</thumb></actor><ratings><rating name="themoviedb" max="10" default="true"><value>8.4</value><votes>123</votes></rating><rating name="imdb" max="10"><value>8.1</value></rating></ratings><set><name>Collection</name><overview>Series plot</overview></set><genre>Drama</genre><genre>Thriller</genre><studio>Studio</studio><director>Director</director><credits>Writer</credits><country>Japan</country><tagline>Tagline</tagline><mpaa>JP: PG12</mpaa><premiered>2024-01-02</premiered><thumb aspect="poster">Movie-poster.jpg</thumb><fanart><thumb>Movie-fanart.jpg</thumb></fanart><sourceattribution>Actual credit</sourceattribution><sourcelicense>Actual license</sourcelicense></movie>"#;
        let metadata = parse_nfo(nfo).unwrap();
        assert_eq!(metadata.fields["cast"][0],json!({"name":"Actor & name","role":"Lead","order":0,"thumb":"Movie-actor.jpg"}));
        assert_eq!(metadata.fields["people"],json!(["Actor & name"]));
        assert_eq!(metadata.fields["ratings"].as_array().unwrap().len(),2);
        assert_eq!(metadata.fields["rating"],8.4);
        assert_eq!(metadata.fields["votes"],123);
        assert_eq!(metadata.fields["collection"],json!({"name":"Collection","overview":"Series plot"}));
        assert_eq!(metadata.fields["genres"],json!(["Drama","Thriller"]));
        assert_eq!(metadata.fields["studios"],json!(["Studio"]));
        assert_eq!(metadata.fields["directors"],json!(["Director"]));
        assert_eq!(metadata.fields["writers"],json!(["Writer"]));
        assert_eq!(metadata.fields["countries"],json!(["Japan"]));
        assert_eq!(metadata.fields["nfo_artwork"],json!([{"kind":"poster","url":"Movie-poster.jpg"},{"kind":"backdrop","url":"Movie-fanart.jpg"}]));
        assert_eq!(metadata.fields["attribution"],"Actual credit");
        assert_eq!(metadata.fields["source_license"],"Actual license");
    }

    #[test]
    fn malformed_optional_rating_fields_do_not_hide_valid_metadata() {
        let metadata = parse_nfo("<movie><title>Valid title</title><ratings><rating max=\"0\"><value>8</value></rating><rating max=\"10\"><value>NaN</value></rating></ratings><actor><name>Actor</name><order>-1</order></actor></movie>").unwrap();
        assert_eq!(metadata.fields["title"],"Valid title");
        assert!(!metadata.fields.contains_key("rating"));
        assert!(metadata.fields["cast"][0].get("order").is_none());
        assert_eq!(parse_nfo("<!DOCTYPE movie [<!ENTITY remote SYSTEM 'https://example.com/a'>]><movie><title>&remote;</title></movie>"),Err(ScanError::UnsafeXml));
    }
}
