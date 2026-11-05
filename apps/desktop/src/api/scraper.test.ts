import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import {
  syntheticMerge,
  syntheticMetadata,
  syntheticOutput,
  syntheticSearch,
} from "../features/scraper/fixtures";
import { mergePreview, scraperApi, scraperOutputApi } from "./scraper";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
});
function respond(data: unknown) {
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data });
}
it("accepts legacy previews and complete ID before/after arrays but rejects malformed identity changes", () => {
  expect(mergePreview(syntheticMerge)).toBe(true);
  expect(mergePreview({ ...syntheticMerge, external_ids: [
    { namespace: "imdb", old: ["tt1"], new: ["tt2"] },
    { namespace: "douban", old: ["1"], new: [] },
    { namespace: "tmdb", old: [], new: ["2"] },
  ] })).toBe(true);
  for (const external_ids of [
    null,
    {},
    [{ namespace: "imdb", old: "tt1", new: ["tt2"] }],
    [{ namespace: "imdb", old: ["tt1"], new: [null] }],
    [{ namespace: "", old: [], new: ["tt2"] }],
    [{ namespace: "imdb", old: [], new: ["bad\nvalue"] }],
    [{ namespace: "imdb", old: [], new: ["x".repeat(257)] }],
  ]) expect(mergePreview({ ...syntheticMerge, external_ids })).toBe(false);
});
it("uses versioned typed search/detail/merge envelopes with immutable snapshot indices", async () => {
  respond(syntheticSearch);
  const query = {
    kind: "movie" as const,
    title: "Synthetic film",
    year: 2024,
    external_ids: [],
    language: "en-US",
  };
  expect(await scraperApi.search({ query, providers: ["tmdb"] })).toEqual(
    syntheticSearch,
  );
  expect(invoke).toHaveBeenLastCalledWith("scraper_search", {
    input: { query, providers: ["tmdb"] },
  });
  respond(syntheticMetadata);
  expect(await scraperApi.detail("run-1", 0)).toEqual(syntheticMetadata);
  expect(invoke).toHaveBeenLastCalledWith("scraper_detail", {
    input: { run_id: "run-1", candidate_index: 0 },
  });
  respond(syntheticMerge);
  await scraperApi.preview({
    run_id: "run-1",
    candidate_index: 0,
    media_item_id: "media-1",
    selected_fields: ["overview"],
    selected_artwork: [],
  });
  expect(invoke).toHaveBeenLastCalledWith("scraper_preview", {
    input: expect.objectContaining({
      selected_fields: ["overview"],
      selected_artwork: [],
    }),
  });
  respond(syntheticOutput);
  await scraperOutputApi.preview("media-1");
  expect(invoke).toHaveBeenLastCalledWith("scraper_output_preview", {
    input: { media_id: "media-1", mode: "online" },
  });
  respond({
    schema_version: 1,
    preview_id: "output-1",
    written: 1,
    preserved: 0,
    already_applied: false,
    state: "succeeded",
  });
  await scraperOutputApi.execute("output-1");
  expect(invoke).toHaveBeenLastCalledWith("scraper_output_execute", {
    input: { preview_id: "output-1" },
  });
});
it.each([
  { ...syntheticSearch, schema_version: 2 },
  {
    ...syntheticSearch,
    candidates: [
      {
        ...syntheticSearch.candidates[0],
        candidate: {
          kind: "adult_video",
          data: syntheticMetadata.data.identity,
        },
      },
    ],
  },
  {
    ...syntheticSearch,
    failures: [{ provider_id: "tmdb", failure: "secret-value" }],
  },
])("rejects malformed or out of scope search responses", async (data) => {
  respond(data);
  await expect(
    scraperApi.search({
      query: {
        kind: "movie",
        title: "x",
        year: null,
        external_ids: [],
        language: "en-US",
      },
      providers: ["tmdb"],
    }),
  ).rejects.toMatchObject({ code: "invalid_response" });
});
it("requires TV season/episode identity and numbering in detail responses", async () => {
  respond({
    kind: "tv",
    data: {
      ...syntheticMetadata.data,
      order: "aired",
      seasons: [
        {
          remote_id: "season",
          season_number: 1,
          title: "Season",
          episodes: [
            {
              remote_id: "episode",
              season_number: 1,
              episode_number: 1,
              title: "Episode",
              overview: null,
              order: "aired",
            },
          ],
        },
      ],
    },
  });
  await expect(scraperApi.detail("run-1", 0)).resolves.toMatchObject({
    kind: "tv",
  });
  respond({
    kind: "tv",
    data: {
      ...syntheticMetadata.data,
      order: "aired",
      seasons: [{ episodes: [] }],
    },
  });
  await expect(scraperApi.detail("run-1", 0)).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("accepts only bounded backend data thumbnails and refuses remote or SVG image payloads", async () => {
  respond({ thumbnail: "data:image/png;base64,aGVsbG8=" });
  await scraperApi.artworkPreview("run-1", 0, 1);
  expect(invoke).toHaveBeenLastCalledWith("scraper_artwork_preview", {
    input: { run_id: "run-1", candidate_index: 0, artwork_index: 1 },
  });
  for (const thumbnail of [
    "https://image.tmdb.org/raw.jpg",
    "data:image/svg+xml;base64,aGVsbG8=",
    "data:image/png;base64,<script>",
  ]) {
    respond({ thumbnail });
    await expect(
      scraperApi.artworkPreview("run-1", 0, 0),
    ).rejects.toMatchObject({ code: "invalid_response" });
  }
});
it("rejects missing protection flags and unsupported output strategies", async () => {
  respond({
    ...syntheticMerge,
    fields: [{ ...syntheticMerge.fields[0], protected: undefined }],
  });
  await expect(
    scraperApi.preview({
      run_id: "run-1",
      candidate_index: 0,
      media_item_id: "media-1",
      selected_fields: [],
      selected_artwork: [],
    }),
  ).rejects.toMatchObject({ code: "invalid_response" });
  respond({ ...syntheticOutput, strategy: "move" });
  await expect(scraperOutputApi.preview("media-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("accepts an explicit update of an existing NFO without allowing media replacement", async () => {
  respond({ ...syntheticOutput, strategy: "update_nfo" });
  await expect(scraperOutputApi.preview("media-1")).resolves.toMatchObject({
    strategy: "update_nfo",
  });
});
it("sends explicit offline mode and accepts Kodi fanart and actor image plans", async () => {
  const image = { ...syntheticOutput.files[0], old_text: null, new_text: null };
  respond({
    ...syntheticOutput,
    files: [
      ...syntheticOutput.files,
      { ...image, destination: "/synthetic/film-fanart.jpg", kind: "fanart" },
      { ...image, destination: "/synthetic/film-actor.jpg", kind: "actor_thumb" },
    ],
  });
  await expect(scraperOutputApi.preview("media-1", "offline")).resolves.toMatchObject({
    files: expect.arrayContaining([
      expect.objectContaining({ kind: "fanart" }),
      expect.objectContaining({ kind: "actor_thumb" }),
    ]),
  });
  expect(invoke).toHaveBeenLastCalledWith("scraper_output_preview", {
    input: { media_id: "media-1", mode: "offline" },
  });
});
it("keeps browser mode explicit without invoking the backend", async () => {
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(scraperOutputApi.history()).rejects.toMatchObject({
    code: "desktop_required",
  });
  expect(invoke).not.toHaveBeenCalled();
});
it("loads nullable persisted media type details and restores immutable search previews", async () => {
  respond(null);
  await expect(scraperApi.mediaDetail("media-1")).resolves.toBeNull();
  expect(invoke).toHaveBeenLastCalledWith("scraper_media_detail", {
    input: { media_id: "media-1" },
  });
  respond(syntheticMetadata);
  await expect(scraperApi.mediaDetail("media-1")).resolves.toEqual(
    syntheticMetadata,
  );
  respond(syntheticSearch);
  await expect(scraperApi.searchPreview("saved-run")).resolves.toEqual(
    syntheticSearch,
  );
  expect(invoke).toHaveBeenLastCalledWith("scraper_search_preview", {
    input: { run_id: "saved-run" },
  });
  respond({ kind: "adult_video", data: syntheticMetadata.data });
  await expect(scraperApi.mediaDetail("media-1")).rejects.toMatchObject({
    code: "invalid_response",
  });
});
it("explicitly discards an output preview to release backend leases without writing", async () => {
  respond(null);
  await expect(scraperOutputApi.discard("output-1")).resolves.toBeNull();
  expect(invoke).toHaveBeenLastCalledWith("scraper_output_discard", {
    input: { preview_id: "output-1" },
  });
});
