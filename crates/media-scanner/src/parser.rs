use crate::ParsedMedia;
use mediaatlas_domain::MediaKind;
use regex::Regex;
use std::sync::LazyLock;

static YEAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[ ._\-\[(])((?:19|20)\d{2})(?:$|[ ._\-\])])").unwrap());
static TV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:s(\d{1,3})[ ._-]*e(\d{1,4})|(\d{1,3})x(\d{1,4}))").unwrap()
});
static PART: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[ ._\-])(?:cd|disc|part|pt)[ ._\-]?(\d{1,2})(?:$|[ ._\-])").unwrap()
});
static CATALOG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[ ._\-\[(])([a-z]{2,12})[-_ ]?(\d{2,7})(?:$|[ ._\-\])])").unwrap()
});
static FC2: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|[ ._\-\[(])fc2[-_ ]?(?:ppv[-_ ]?)?(\d{4,8})(?:$|[ ._\-\])])").unwrap()
});
// In a Mixed root only a separated catalog at the beginning of the filename
// or the distinctive FC2 marker is inferred; it still requires user review.
static MIXED_CATALOG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^([a-z]{2,12})[-_](\d{2,7})(?:$|[ ._\-\[(])").unwrap());
static ABSOLUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?: - |[._ ])(\d{2,4})(?:v\d)?(?:$|[ ._\[(])").unwrap());
static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[[^\]]*\]").unwrap());
static TECHNICAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:2160p|1080p|720p|480p|4k|bluray|blu-ray|bdrip|webrip|web-dl|hdtv|x264|x265|h264|h265|hevc|av1|remux|aac|dts|hdr|dv)\b.*$").unwrap()
});
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ ._]+|\s{2,}").unwrap());

fn clean_title(value: &str) -> String {
    let no_brackets = BRACKETS.replace_all(value, " ");
    let no_technical = TECHNICAL.replace_all(&no_brackets, " ");
    let no_part = PART.replace_all(&no_technical, " ");
    SPACES
        .replace_all(&no_part, " ")
        .trim_matches([' ', '-', '(', ')'])
        .to_owned()
}

/// Heuristic filename parsing; uncertain candidates retain a review marker.
pub fn parse_filename(path: &str, kind: Option<MediaKind>) -> ParsedMedia {
    let normalized = path.replace('\\', "/");
    let (parent, filename) = normalized
        .rsplit_once('/')
        .unwrap_or(("", normalized.as_str()));
    let stem = filename.rsplit_once('.').map_or(filename, |(stem, _)| stem);
    let mixed_adult = kind.is_none()
        && (FC2.is_match(stem)
            || MIXED_CATALOG.captures(stem).is_some_and(|captures| {
                let prefix = captures[1].to_ascii_lowercase();
                let number = captures[2].parse::<u32>().unwrap_or_default();
                !(1900..=2099).contains(&number)
                    && !matches!(
                        prefix.as_str(),
                        "cd" | "disc"
                            | "part"
                            | "pt"
                            | "season"
                            | "episode"
                            | "ep"
                            | "vol"
                            | "volume"
                    )
            }));
    let kind = if mixed_adult {
        Some(MediaKind::AdultVideo)
    } else {
        kind
    };
    let year_match = YEAR.captures(stem);
    let year = year_match.as_ref().and_then(|c| c[1].parse().ok());
    let part = PART.captures(stem).and_then(|c| c[1].parse().ok());
    let mut parsed = ParsedMedia {
        kind: kind.unwrap_or(MediaKind::Movie),
        title: String::new(),
        year,
        part,
        series_title: None,
        season_number: None,
        episode_number: None,
        absolute_number: None,
        catalog_number: None,
        group_key: String::new(),
        needs_review: false,
        warnings: Vec::new(),
    };
    let music = matches!(
        kind,
        Some(
            MediaKind::MusicArtist
                | MediaKind::MusicReleaseGroup
                | MediaKind::MusicRelease
                | MediaKind::MusicTrack
                | MediaKind::MusicRecording
        )
    );
    if let Some(captures) = TV
        .captures(stem)
        .filter(|_| kind != Some(MediaKind::AdultVideo) && !music)
    {
        let marker = captures.get(0).unwrap();
        parsed.season_number = captures
            .get(1)
            .or_else(|| captures.get(3))
            .and_then(|c| c.as_str().parse().ok());
        parsed.episode_number = captures
            .get(2)
            .or_else(|| captures.get(4))
            .and_then(|c| c.as_str().parse().ok());
        parsed.title = clean_title(&stem[..marker.start()]);
        if parsed.title.is_empty() {
            parsed.title = clean_title(parent.rsplit('/').next().unwrap_or(""));
            parsed.needs_review = true;
        }
        parsed.series_title = Some(parsed.title.clone());
        if kind != Some(MediaKind::Anime) && kind != Some(MediaKind::AdultVideo) {
            parsed.kind = MediaKind::TvEpisode;
        }
    } else if kind == Some(MediaKind::AdultVideo) {
        if let Some(captures) = FC2.captures(stem) {
            let number = format!("FC2-PPV-{}", &captures[1]);
            parsed.title = number.clone();
            parsed.catalog_number = Some(number);
        } else if let Some(captures) = CATALOG.captures(stem) {
            let number = format!("{}-{}", captures[1].to_ascii_uppercase(), &captures[2]);
            parsed.title = number.clone();
            parsed.catalog_number = Some(number);
        } else {
            parsed.title = clean_title(stem);
            parsed.needs_review = true;
            parsed.warnings.push("catalog_number_unrecognized".into());
        }
    } else if kind == Some(MediaKind::Anime) {
        if let Some(captures) = ABSOLUTE.captures(stem) {
            let number = captures[1].parse().ok();
            if number != year.map(|value| value as u32) {
                parsed.absolute_number = number;
                parsed.title = clean_title(&stem[..captures.get(0).unwrap().start()]);
                parsed.series_title = Some(parsed.title.clone());
                parsed.needs_review = true;
                parsed
                    .warnings
                    .push("anime_absolute_order_requires_review".into());
            }
        }
        if parsed.title.is_empty() {
            parsed.title = clean_title(stem);
            parsed.needs_review = true;
        }
    } else {
        let title_end = year_match
            .as_ref()
            .and_then(|c| c.get(0))
            .map_or(stem.len(), |m| m.start());
        parsed.title = clean_title(&stem[..title_end]);
        if matches!(
            kind,
            Some(MediaKind::TvSeries | MediaKind::TvSeason | MediaKind::TvEpisode)
        ) {
            parsed.needs_review = true;
            parsed.warnings.push("episode_number_unrecognized".into());
        }
        if matches!(
            kind,
            Some(
                MediaKind::MusicArtist
                    | MediaKind::MusicReleaseGroup
                    | MediaKind::MusicRelease
                    | MediaKind::MusicTrack
                    | MediaKind::MusicRecording
            )
        ) {
            parsed.kind = MediaKind::MusicTrack;
        }
        if kind.is_none() {
            parsed.needs_review = true;
            parsed.warnings.push("media_kind_inferred".into());
        }
    }
    if parsed.title.is_empty() {
        parsed.title = stem.to_owned();
        parsed.needs_review = true;
        parsed.warnings.push("title_unrecognized".into());
    }
    let identity = if let Some(catalog) = &parsed.catalog_number {
        format!("adult:{catalog}")
    } else if parsed.episode_number.is_some() || parsed.absolute_number.is_some() {
        format!(
            "{}:{:?}:{:?}:{:?}",
            parsed.title.to_ascii_lowercase(),
            parsed.season_number,
            parsed.episode_number,
            parsed.absolute_number
        )
    } else {
        format!("{}:{:?}", parsed.title.to_ascii_lowercase(), parsed.year)
    };
    parsed.group_key = format!("{}|{}|{identity}", parent, parsed.kind.as_str());
    if mixed_adult {
        parsed.needs_review = true;
        parsed.warnings.push("mixed_adult_inferred".into());
    }
    parsed
}
