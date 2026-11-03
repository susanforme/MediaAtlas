import { expect, it } from "vitest";
import type { MediaDetail, MetadataValue } from "../../api/library";
import type { RemoteField } from "../../api/scraper";
import { selectScraperFields } from "./field-selection";
import { syntheticMerge } from "./fixtures";

const target: MediaDetail = {
  media: {
    id: "local",
    kind: "movie",
    title: "Local title",
    original_title: null,
    year: 2024,
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  },
  metadata_values: [],
  files: [],
  artworks: [],
  external_ids: [],
  relations: [],
  health: [],
  poster_uri: null,
};
function local(
  field_key: string,
  value: unknown,
  flags: Partial<MetadataValue> = {},
): MetadataValue {
  return {
    id: field_key,
    field_key,
    value,
    locale: null,
    provider_id: null,
    source_url: null,
    is_locked: false,
    is_user_modified: false,
    ...flags,
  };
}
const fields: RemoteField[] = [
  "title",
  "overview",
  "rating",
  "genres",
  "manual",
  "locked",
  "runtime_minutes",
  "overview",
].map((key) => ({ key, value: "Remote value", language: null }));

it("fills only actual missing values, keeping canonical metadata, zero and protected empty fields", () => {
  const detail = {
    ...target,
    metadata_values: [
      local("overview", " "),
      local("rating", 0),
      local("genres", []),
      local("manual", "", { is_user_modified: true }),
      local("locked", null, { is_locked: true }),
    ],
  };
  expect(selectScraperFields("missing", fields, detail)).toEqual([
    "overview",
    "genres",
    "runtime_minutes",
  ]);
  expect(selectScraperFields("all", fields, detail)).toEqual([
    "title",
    "overview",
    "rating",
    "genres",
    "runtime_minutes",
  ]);
  expect(selectScraperFields("none", fields, detail)).toEqual([]);
});
it("respects backend preview protection and cannot infer missing fields without a real local target", () => {
  expect(
    selectScraperFields("all", fields, target, syntheticMerge),
  ).not.toContain("title");
  expect(selectScraperFields("missing", fields)).toEqual([]);
  expect(selectScraperFields("all", [], target)).toEqual([]);
  const filled = { ...target, metadata_values: [local("overview", false)] };
  expect(selectScraperFields("missing", fields, filled)).not.toContain(
    "overview",
  );
});
