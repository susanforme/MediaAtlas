import { expect, it } from "vitest";
import type { MetadataValue } from "../../api/library";
import { videoDetailFields } from "./video-detail-model";

function value(
  field_key: string,
  fieldValue: unknown,
  locale: string | null = null,
): MetadataValue {
  return {
    id: `${field_key}:${locale}`,
    field_key,
    value: fieldValue,
    locale,
    provider_id: "local_nfo",
    source_url: null,
    is_locked: true,
    is_user_modified: true,
  };
}

function fields(metadata_values: MetadataValue[], language = "en-US") {
  return videoDetailFields({ metadata_values }, language);
}

it("uses the requested library locale and preserves local NFO and edited values", () => {
  const result = fields([
    value("overview", "中文简介", "zh-CN"),
    value("overview", "Edited overview", "en-US"),
    value("runtime", 95),
    value("premiered", "2026-03-20"),
    value("directors", ["Director"]),
    value("people", ["Local actor"]),
  ]);
  expect(result.overview).toBe("Edited overview");
  expect(result.runtime).toBe(95);
  expect(result.premiered).toBe("2026-03-20");
  expect(result.directors).toEqual(["Director"]);
  expect(result.cast).toEqual(["Local actor"]);
});

it("keeps real ratings attributed and rejects unsupported metadata shapes", () => {
  expect(fields([value("rating", 7.8)]).rating).toEqual({
    value: 7.8,
    provider: "local_nfo",
  });
  const result = fields([
    value("rating", 78),
    value("runtime_minutes", -95),
    value("overview", { url: "https://example.test" }),
    value("genres", ["Drama", null, "Drama", "", 1]),
  ]);
  expect(result.rating).toBeNull();
  expect(result.runtime).toBeNull();
  expect(result.overview).toBe("");
  expect(result.genres).toEqual(["Drama"]);
});

it("prefers typed runtime and cast fields and keeps missing facts empty", () => {
  const result = fields([
    value("runtime_minutes", 100),
    value("runtime", 95),
    value("cast", ["Source actor"]),
    value("people", ["Local actor"]),
  ]);
  expect(result.runtime).toBe(100);
  expect(result.cast).toEqual(["Source actor"]);
  expect(fields([])).toMatchObject({
    overview: "",
    genres: [],
    cast: [],
    runtime: null,
    rating: null,
  });
});
