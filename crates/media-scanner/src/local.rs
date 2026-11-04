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
    let mut studios = Vec::new();
    for child in root.children().filter(roxmltree::Node::is_element) {
        let text = child
            .text()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let Some(value) = text else {
            if child.has_tag_name("actor") {
                if let Some(name) = child
                    .children()
                    .find(|n| n.has_tag_name("name"))
                    .and_then(|n| n.text())
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    people.push(Value::String(name.to_owned()));
                }
            }
            continue;
        };
        match child.tag_name().name() {
            "title" | "originaltitle" | "plot" | "outline" | "premiered" | "aired" | "album"
            | "artist" => {
                let key = match child.tag_name().name() {
                    "originaltitle" => "original_title",
                    "plot" => "overview",
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
    }
    if !studios.is_empty() {
        metadata.fields.insert("studios".into(), json!(studios));
    }
    metadata.external_ids.sort_by(|a, b| {
        (&a.provider_id, &a.namespace, &a.value).cmp(&(&b.provider_id, &b.namespace, &b.value))
    });
    metadata.external_ids.dedup();
    Ok(metadata)
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
