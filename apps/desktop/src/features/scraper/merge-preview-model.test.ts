import { describe, expect, it } from "vitest";
import type { MediaDetail, MetadataValue } from "../../api/library";
import type {
  MergePreview,
  RemoteField,
  VideoMetadata,
} from "../../api/scraper";
import { syntheticIdentity } from "./fixtures";
import {
  buildMergePreview,
  mergeSelectionMatches,
} from "./merge-preview-model";

const target: MediaDetail = {
  media: {
    id: "local",
    kind: "movie",
    title: "Local title",
    original_title: null,
    year: null,
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
    locale: "zh-CN",
    provider_id: null,
    source_url: null,
    is_user_modified: false,
    is_locked: false,
    ...flags,
  };
}
function metadata(fields: RemoteField[]): VideoMetadata {
  return {
    kind: "movie",
    data: {
      identity: syntheticIdentity,
      fields,
      artwork: ["poster", "backdrop"].map((kind) => ({
        kind,
        source_provider: "tmdb",
        url: `https://example.com/${kind}.png`,
        language: null,
      })),
      runtime_minutes: null,
    },
  };
}
const fields: RemoteField[] = [
  { key: "title", value: "Remote title", language: "zh-CN" },
  { key: "overview", value: "Remote summary", language: "zh-CN" },
  { key: "genres", value: ["Drama"], language: null },
];

function project(
  overrides: Partial<Parameters<typeof buildMergePreview>[0]> = {},
) {
  return buildMergePreview({
    metadata: metadata(fields),
    target,
    selectedFields: fields.map((field) => field.key),
    selectedArtwork: [],
    onlyMissing: false,
    ...overrides,
  });
}

describe("local merge projection", () => {
  it("updates the merged result immediately from selected fields without mutating either snapshot", () => {
    const detail = {
      ...target,
      metadata_values: [local("overview", "Local summary")],
    };
    const first = project({ target: detail });
    expect(first.rows.map((row) => row.result)).toEqual([
      "added",
      "overwritten",
      "added",
    ]);
    expect(first.counts).toEqual({
      added: 2,
      overwritten: 1,
      unchanged: 0,
      skipped: 0,
      artwork: 0,
      total: 3,
    });
    expect(first.media.title).toBe("Remote title");
    expect(first.fields.find((field) => field.key === "overview")?.value).toBe(
      "Remote summary",
    );
    const second = project({ target: detail, selectedFields: ["genres"] });
    expect(second.media.title).toBe("Local title");
    expect(second.rows[1].value).toBe("Local summary");
    expect(second.rows[1].result).toBe("skipped");
    expect(second.effectiveFields).toEqual(["genres"]);
    expect(detail.metadata_values[0].value).toBe("Local summary");
    expect(fields[0].value).toBe("Remote title");
  });

  it("matches normalized language keys exactly and preserves unrelated translations", () => {
    const detail = {
      ...target,
      metadata_values: [
        local("overview", "English summary", { locale: "en-US" }),
        local("overview", "Chinese summary", { locale: "zh-cn" }),
        local("genres", ["Drama"], { locale: null }),
      ],
    };
    const result = project({ target: detail });
    expect(result.rows[1].old?.value).toBe("Chinese summary");
    expect(result.rows[1].result).toBe("overwritten");
    expect(result.rows[2].result).toBe("unchanged");
    expect(result.fields.filter((field) => field.key === "overview")).toEqual([
      { key: "overview", value: "English summary", language: "en-US" },
      fields[1],
    ]);
    expect(
      project({
        target: { ...target, metadata_values: [detail.metadata_values[0]] },
      }).rows[1].result,
    ).toBe("added");
  });

  it("protects locked and manually edited keys across languages, including canonical display values", () => {
    const detail = {
      ...target,
      metadata_values: [
        local("title", "Edited neutral title", {
          locale: null,
          is_user_modified: true,
        }),
        local("overview", "", { locale: "en-US", is_locked: true }),
      ],
    };
    const result = project({ target: detail });
    expect(result.effectiveFields).toEqual(["genres"]);
    expect(result.rows.slice(0, 2).map((row) => row.protected)).toEqual([
      true,
      true,
    ]);
    expect(result.media.title).toBe("Local title");
    expect(result.fields[0].value).toBe("Edited neutral title");
  });

  it("fills only empty fields and treats canonical values, other locales, zero and false as existing", () => {
    const remote = metadata([
      ...fields,
      { key: "rating", value: 8, language: null },
      { key: "available", value: true, language: null },
      { key: "year", value: 2024, language: null },
    ]);
    const detail = {
      ...target,
      metadata_values: [
        local("overview", "English summary", { locale: "en" }),
        local("genres", [], { locale: null }),
        local("rating", 0, { locale: null }),
        local("available", false, { locale: null }),
      ],
    };
    const result = project({
      metadata: remote,
      target: detail,
      selectedFields: remote.data.fields.map((field) => field.key),
      onlyMissing: true,
    });
    expect(result.effectiveFields).toEqual(["genres", "year"]);
    expect(result.media).toEqual({
      title: "Local title",
      original_title: null,
      year: 2024,
    });
    expect(
      project({ target: undefined, onlyMissing: true }).effectiveFields,
    ).toEqual([]);
  });

  it("distinguishes an unselected eligible missing field from an unavailable canonical field", () => {
    const result = project({ selectedFields: [], onlyMissing: true });
    expect(result.rows[0]).toMatchObject({
      key: "title",
      selected: false,
      eligible: false,
      result: "skipped",
    });
    expect(result.rows[1]).toMatchObject({
      key: "overview",
      selected: false,
      eligible: true,
      result: "skipped",
    });
    expect(result.effectiveFields).toEqual([]);
  });

  it("does not select empty provider values, invalid locale keys or ambiguous duplicate keys", () => {
    const remote = metadata([
      { key: "null", value: null, language: null },
      { key: "blank", value: "  ", language: null },
      { key: "array", value: [], language: null },
      { key: "invalid", value: "text", language: " zh-CN" },
      { key: "duplicate", value: "One", language: "en" },
      { key: "duplicate", value: "Two", language: "zh" },
      { key: "zero", value: 0, language: null },
      { key: "false", value: false, language: null },
    ]);
    const result = project({
      metadata: remote,
      selectedFields: remote.data.fields.map((field) => field.key),
    });
    expect(result.effectiveFields).toEqual(["zero", "false"]);
    expect(result.counts.total).toBe(2);
    expect(result.counts.skipped).toBe(6);
  });

  it("deduplicates valid artwork selections and avoids filling an existing artwork kind", () => {
    const result = project({
      selectedArtwork: [0, 0, 1, -1, 8, 0.5],
    });
    expect(result.effectiveArtwork).toEqual([0, 1]);
    expect(result.artwork.map((art) => art.kind)).toEqual([
      "poster",
      "backdrop",
    ]);
    expect(result.counts.artwork).toBe(2);
    expect(result.counts.total).toBe(5);
    expect(
      project({
        target: { ...target, poster_uri: "data:image/png;base64,fixture" },
        selectedArtwork: [0, 1],
        onlyMissing: true,
      }).effectiveArtwork,
    ).toEqual([1]);
    expect(
      project({
        target: {
          ...target,
          artworks: [
            {
              id: "backdrop",
              kind: "backdrop",
              cache_path: null,
              source_url: "https://example.com/existing.png",
            },
          ],
        },
        selectedArtwork: [0, 1],
        onlyMissing: true,
      }).effectiveArtwork,
    ).toEqual([0]);
  });

  it("uses fresh server protection without reviving stale selections or mismatched media and values", () => {
    const preview: MergePreview = {
      schema_version: 1,
      preview_id: "proof",
      media_item_id: target.media.id,
      fields: [
        {
          key: "title",
          language: "zh-cn",
          old: null,
          new: fields[0],
          provider_id: "tmdb",
          source_url: syntheticIdentity.source_url,
          protected: true,
          needs_review: false,
          selected: true,
        },
      ],
      artwork: [],
      confirmed: false,
    };
    expect(project({ preview }).effectiveFields).not.toContain("title");
    expect(
      project({
        preview: { ...preview, media_item_id: "other-media" },
      }).effectiveFields,
    ).toContain("title");
    expect(
      project({
        preview: {
          ...preview,
          fields: [
            {
              ...preview.fields[0],
              new: { ...fields[0], value: "Old candidate" },
            },
          ],
        },
      }).effectiveFields,
    ).toContain("title");
    expect(
      project({
        preview: {
          ...preview,
          fields: [{ ...preview.fields[0], protected: false }],
        },
        selectedFields: [],
      }).effectiveFields,
    ).toEqual([]);
  });

  it("keeps missing canonical facts missing and compares JSON objects independently of key order", () => {
    const remote = metadata([
      {
        key: "details",
        value: { a: 1, nested: [2, { z: true }] },
        language: null,
      },
    ]);
    const result = project({
      metadata: remote,
      selectedFields: ["details"],
      target: {
        ...target,
        metadata_values: [
          local(
            "details",
            { nested: [2, { z: true }], a: 1 },
            { locale: null },
          ),
        ],
      },
    });
    expect(result.rows[0].result).toBe("unchanged");
    expect(result.counts.total).toBe(0);
    expect(result.media.title).toBe("Local title");
    expect(result.media.year).toBeNull();
    expect(result.media.original_title).toBeNull();
  });
});

function serverProof(overrides: Partial<MergePreview> = {}): MergePreview {
  return {
    schema_version: 1,
    preview_id: "proof-1",
    media_item_id: target.media.id,
    fields: fields.map((field) => ({
      key: field.key,
      language: field.language,
      old: null,
      new: field,
      provider_id: "tmdb",
      source_url: syntheticIdentity.source_url,
      protected: false,
      needs_review: false,
      selected: true,
    })),
    artwork: [],
    confirmed: false,
    ...overrides,
  };
}

describe("server proof and visible merge selection", () => {
  it("matches exactly the unprotected visible fields and rejects applying a locally protected field", () => {
    const detail = {
      ...target,
      metadata_values: [local("title", "Locked title", { is_locked: true })],
    };
    const projection = project({ target: detail });
    const proof = serverProof();
    proof.fields[0] = {
      ...proof.fields[0],
      old: detail.metadata_values[0],
      protected: true,
      selected: false,
    };
    expect(mergeSelectionMatches(proof, projection)).toBe(true);
    proof.fields[0] = { ...proof.fields[0], protected: false, selected: true };
    expect(mergeSelectionMatches(proof, projection)).toBe(false);
  });

  it("rejects a concurrent fill that onlyMissing now skips even when another selected field still changes", () => {
    const proof = serverProof();
    proof.fields[0].selected = false;
    proof.fields[1].old = local("overview", "Filled concurrently");
    const projection = project({ onlyMissing: true, preview: proof });
    expect(projection.effectiveFields).toEqual(["genres"]);
    expect(projection.counts.total).toBe(1);
    expect(mergeSelectionMatches(proof, projection)).toBe(false);
    proof.fields[1].selected = false;
    expect(mergeSelectionMatches(proof, projection)).toBe(true);
  });

  it("ignores ordering and locale case but rejects mismatched media, locale, content and artwork", () => {
    const projection = project({ selectedArtwork: [0, 1] });
    const proof = serverProof({
      artwork: [...metadata(fields).data.artwork].reverse(),
    });
    proof.fields.reverse();
    const overview = proof.fields.find((field) => field.key === "overview");
    if (!overview) throw new Error("Fixture overview is required");
    overview.language = "en-US";
    expect(mergeSelectionMatches(proof, projection)).toBe(false);
    overview.language = "ZH-cn";
    expect(mergeSelectionMatches(proof, projection)).toBe(true);
    expect(
      mergeSelectionMatches({ ...proof, media_item_id: "other" }, projection),
    ).toBe(false);
    expect(
      mergeSelectionMatches(
        {
          ...proof,
          fields: proof.fields.map((field) => ({
            ...field,
            new: { ...field.new, value: "Unexpected value" },
          })),
        },
        projection,
      ),
    ).toBe(false);
    expect(
      mergeSelectionMatches(
        {
          ...proof,
          artwork: [
            { ...proof.artwork[0], source_provider: "other" },
            proof.artwork[1],
          ],
        },
        projection,
      ),
    ).toBe(false);
    expect(
      mergeSelectionMatches(
        { ...proof, artwork: [proof.artwork[0], proof.artwork[0]] },
        projection,
      ),
    ).toBe(false);
  });
});
