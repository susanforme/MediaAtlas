/** Synthetic fixtures for automated tests only. Never used by the production UI. */
import type {
  MergePreview,
  OutputPreview,
  SearchPreview,
  VideoMetadata,
} from "../../api/scraper";
export const syntheticIdentity = {
  provider_id: "tmdb",
  remote_id: "fixture-1",
  title: "Synthetic film",
  original_title: null,
  year: 2024,
  aliases: [],
  external_ids: [{ namespace: "tmdb", value: "fixture-1" }],
  source_url: "https://www.themoviedb.org/movie/fixture-1",
  language: "en-US",
};
export const syntheticSearch: SearchPreview = {
  schema_version: 1,
  run_id: "run-1",
  candidates: [
    {
      candidate: { kind: "movie", data: syntheticIdentity },
      reasons: ["title_exact", "year_conflict"],
      needs_review: true,
      duplicate_of: null,
    },
  ],
  failures: [],
};
export const syntheticMetadata: VideoMetadata = {
  kind: "movie",
  data: {
    identity: syntheticIdentity,
    fields: [
      { key: "title", value: "Synthetic film", language: "en-US" },
      {
        key: "overview",
        value: "Synthetic provider summary",
        language: "en-US",
      },
    ],
    artwork: [
      {
        url: "https://image.tmdb.org/t/p/w500/fixture.jpg",
        source_provider: "tmdb",
        kind: "poster",
        language: null,
      },
    ],
    runtime_minutes: 90,
  },
};
export const syntheticMerge: MergePreview = {
  schema_version: 1,
  preview_id: "merge-1",
  media_item_id: "media-1",
  fields: [
    {
      key: "title",
      language: "en-US",
      old: {
        id: "value-1",
        field_key: "title",
        locale: "en-US",
        value: "My manual title",
        provider_id: null,
        source_url: null,
        is_user_modified: true,
        is_locked: true,
      },
      new: syntheticMetadata.data.fields[0],
      provider_id: "tmdb",
      source_url: syntheticIdentity.source_url,
      protected: true,
      needs_review: true,
      selected: false,
    },
    {
      key: "overview",
      language: "en-US",
      old: null,
      new: syntheticMetadata.data.fields[1],
      provider_id: "tmdb",
      source_url: syntheticIdentity.source_url,
      protected: false,
      needs_review: false,
      selected: true,
    },
  ],
  artwork: [],
  confirmed: false,
};
export const syntheticOutput: OutputPreview = {
  schema_version: 1,
  preview_id: "output-1",
  media_item_id: "media-1",
  strategy: "preserve",
  files: [
    {
      destination: "/synthetic/library/film.nfo",
      kind: "nfo",
      old_text: "<movie><title>My manual title</title></movie>",
      new_text:
        "<movie><title>My manual title</title><plot>Synthetic provider summary</plot></movie>",
      expected_hash: "old-hash",
      content_hash: "new-hash",
      protected: true,
    },
  ],
  state: "previewed",
};
