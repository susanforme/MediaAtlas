/** Synthetic test data only. Never used by production Music pages. */
import type { MusicReleaseGraph, TagPreview, TagResult } from "../../api/music";
export const syntheticMusicGraph: MusicReleaseGraph = {
  artists: [
    {
      media_item_id: "artist-1",
      name: "Synthetic artist",
      sort_name: null,
      aliases: ["合成艺人"],
    },
  ],
  release_group: {
    media_item_id: "group-1",
    title: "Synthetic album",
    artist_credits: [
      {
        artist_id: "artist-1",
        credited_name: "Synthetic artist",
        join_phrase: "",
      },
    ],
    primary_type: "Album",
    secondary_types: [],
    first_release_date: null,
  },
  release: {
    media_item_id: "release-1",
    release_group_id: "group-1",
    title: "Synthetic album",
    artist_credits: [
      {
        artist_id: "artist-1",
        credited_name: "Synthetic artist",
        join_phrase: "",
      },
    ],
    country: "JP",
    release_date: "2024-01-01",
    status: "Official",
    labels: [],
  },
  media: [
    {
      id: "disc-1",
      release_id: "release-1",
      position: 1,
      title: null,
      format: "CD",
    },
    {
      id: "disc-2",
      release_id: "release-1",
      position: 2,
      title: "Bonus",
      format: "CD",
    },
  ],
  tracks: [
    {
      media_item_id: "track-1",
      medium_id: "disc-1",
      recording_id: "recording-1",
      position: 1,
      number: "1",
      title: "Online track",
      length_ms: 183000,
      artist_credits: [
        {
          artist_id: "artist-1",
          credited_name: "Synthetic artist",
          join_phrase: "",
        },
      ],
    },
    {
      media_item_id: "track-2",
      medium_id: "disc-2",
      recording_id: "recording-2",
      position: 1,
      number: "1",
      title: "Bonus track",
      length_ms: null,
      artist_credits: [],
    },
  ],
  recordings: [
    {
      media_item_id: "recording-1",
      title: "Online track",
      length_ms: 183000,
      artist_credits: [],
    },
    {
      media_item_id: "recording-2",
      title: "Bonus track",
      length_ms: null,
      artist_credits: [],
    },
  ],
  identifiers: [
    {
      media_item_id: "release-1",
      kind: "music_brainz",
      namespace: "release",
      value: "mbid-1",
    },
  ],
  files: [
    {
      track_id: "track-1",
      file_id: "file-1",
      relative_path: "disc1/track.flac",
    },
  ],
  source_url: "https://musicbrainz.org/release/mbid-1",
  fetched_at: "2026-01-01T00:00:00Z",
  locale: "en-US",
  field_sources: [
    {
      id: "meta-1",
      media_item_id: "release-1",
      field_key: "title",
      locale: "zh-CN",
      value: "保护标题",
      provider_id: null,
      source_url: null,
      is_locked: true,
      is_user_modified: true,
    },
  ],
};
export const syntheticTagPreview: TagPreview = {
  id: "preview-1",
  file_id: "file-1",
  changes: [{ field: "title", before: "Local title", after: "Online track" }],
  kind: "write",
  state: "previewed",
};
export const syntheticTagResult: TagResult = {
  operation_id: "op-1",
  file_id: "file-1",
  state: "succeeded",
  backup_relative: ".mediaatlas-backup-op-1",
  error_code: null,
};
