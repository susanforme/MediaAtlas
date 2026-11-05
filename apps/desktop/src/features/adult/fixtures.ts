// Neutral synthetic fixtures; these are never live provider results or real adult artwork.
import type { AdultDetail, AdultOutputPreview, AdultSearchPreview } from "../../api/adult";
import type { MediaPage } from "../../api/library";
import type { MergePreview } from "../../api/scraper";
export const syntheticAdultDetail: AdultDetail = {
  identity: {
    provider_id: "dmm",
    remote_id: "synthetic001",
    catalog_number: {
      raw: "Synthetic_001-CD1",
      normalized: "SYNTHETIC-001",
      namespace: "dmm",
      is_primary: true,
      aliases: ["SYNTHETIC001"],
      part: { kind: "cd", number: 1 },
    },
    title: "Synthetic catalog fixture",
    studio: "Synthetic Studio",
    source_url: "https://example.test/synthetic",
    language: "ja",
  },
  label: "Synthetic Label",
  series: "Synthetic Series",
  release_date: "2024-01-01",
  runtime_minutes: 120,
  performers: [
    {
      provider_id: "dmm",
      remote_id: "person-1",
      name: "Synthetic Person",
      aliases: ["Synthetic Alias"],
    },
  ],
  directors: [],
  artwork: [
    {
      url: "https://pics.dmm.co.jp/synthetic.png",
      source_provider: "dmm",
      kind: "cover",
      language: null,
    },
    {
      url: "https://pics.dmm.co.jp/synthetic-sample.png",
      source_provider: "dmm",
      kind: "sample",
      language: null,
    },
  ],
};
export const syntheticAdultSearch: AdultSearchPreview = {
  schema_version: 1,
  run_id: "synthetic-run",
  candidates: [
    {
      candidate: syntheticAdultDetail.identity,
      reasons: ["catalog_exact", "manual_selection_required"],
      needs_review: true,
      duplicate_of: null,
    },
  ],
  failures: [{ provider_id: "wikidata", failure: "not_found" }],
};
export const syntheticAdultMedia: MediaPage = {
  items: [
    {
      media: {
        id: "synthetic-media",
        kind: "adult_video",
        title: "Synthetic local adult fixture",
        original_title: null,
        year: 2024,
        created_at: "2026-11-03T00:00:00Z",
        updated_at: "2026-11-03T00:00:00Z",
      },
      poster_uri: "data:image/png;base64,aGVsbG8=",
      health: [],
    },
  ],
  next_cursor: null,
};
export const syntheticAdultMerge: MergePreview = {
  schema_version: 1,
  preview_id: "synthetic-merge",
  media_item_id: "synthetic-media",
  fields: [
    {
      key: "title",
      language: "ja",
      old: null,
      new: { key: "title", value: "Synthetic catalog fixture", language: "ja" },
      provider_id: "dmm",
      source_url: "https://example.test/synthetic",
      protected: true,
      needs_review: true,
      selected: true,
    },
  ],
  artwork: [],
  confirmed: false,
};
export const syntheticAdultOutput: AdultOutputPreview = {
  schema_version: 1,
  preview_id: "synthetic-output",
  media_item_id: "synthetic-media",
  strategy: "preserve",
  state: "planned",
  files: [
    {
      destination: "synthetic.nfo",
      kind: "nfo",
      old_text: null,
      new_text: "<movie><title>Synthetic fixture</title></movie>",
      expected_hash: null,
      content_hash: "synthetic-hash",
      protected: false,
    },
  ],
};
