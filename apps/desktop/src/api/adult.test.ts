import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import {
  syntheticAdultDetail,
  syntheticAdultMerge,
  syntheticAdultOutput,
  syntheticAdultSearch,
} from "../features/adult/fixtures";
import {
  adultApi,
  adultBatchPreview,
  adultBatchResult,
  adultDetail,
  adultSearchPreview,
  adultSettings,
} from "./adult";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
});
function respond(data: unknown) {
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data });
}
it("uses dedicated adult IPC envelopes with typed independent catalog identity", async () => {
  respond(syntheticAdultSearch);
  const query = {
    catalog_number: "SYNTHETIC-001",
    namespace: "dmm",
    studio: null,
    language: "en-US",
  };
  expect(await adultApi.search(query, ["dmm"])).toEqual(syntheticAdultSearch);
  expect(invoke).toHaveBeenLastCalledWith("adult_search", {
    input: { query, providers: ["dmm"] },
  });
  respond(syntheticAdultDetail);
  expect(await adultApi.detail("synthetic-run", 0)).toEqual(
    syntheticAdultDetail,
  );
  respond(syntheticAdultMerge);
  await adultApi.preview({
    run_id: "synthetic-run",
    candidate_index: 0,
    media_item_id: "synthetic-media",
    selected_fields: ["catalog_numbers"],
    selected_artwork: [1],
  });
  expect(invoke).toHaveBeenLastCalledWith("adult_preview", {
    input: {
      run_id: "synthetic-run",
      candidate_index: 0,
      media_item_id: "synthetic-media",
      selected_fields: ["catalog_numbers"],
      selected_artwork: [1],
    },
  });
});
it.each([
  {
    ...syntheticAdultDetail,
    identity: {
      ...syntheticAdultDetail.identity,
      catalog_number: {
        ...syntheticAdultDetail.identity.catalog_number,
        part: { kind: "disc", number: 1 },
      },
    },
  },
  {
    ...syntheticAdultDetail,
    identity: {
      ...syntheticAdultDetail.identity,
      source_url: "http://unsafe.test",
    },
  },
  {
    ...syntheticAdultDetail,
    performers: [{ name: "Same name without provider identity" }],
  },
  {
    ...syntheticAdultDetail,
    artwork: [
      {
        url: "https://u:p@example.test/image",
        source_provider: "dmm",
        kind: "cover",
        language: null,
      },
    ],
  },
])(
  "rejects malformed adult details before the UI sees them",
  async (detail) => {
    expect(adultDetail(detail)).toBe(false);
    respond(detail);
    await expect(adultApi.detail("synthetic-run", 0)).rejects.toMatchObject({
      code: "invalid_response",
    });
  },
);
it("rejects ordinary movie candidates and unsupported settings response fields", () => {
  expect(
    adultSearchPreview({
      ...syntheticAdultSearch,
      candidates: [
        {
          ...syntheticAdultSearch.candidates[0],
          candidate: { kind: "movie", data: syntheticAdultDetail.identity },
        },
      ],
    }),
  ).toBe(false);
  expect(
    adultSettings({
      entry_visible: true,
      covers_visible: false,
      remember_recent: false,
      unlocked: "true",
    }),
  ).toBe(false);
});
it("keeps output preview, execute and cancel as separate commands", async () => {
  const batch = {
    schema_version: 1,
    batch_id: "synthetic-batch",
    state: "planned",
    items: [syntheticAdultOutput],
  };
  respond(batch);
  await adultApi.batchPreview(["synthetic-media"]);
  expect(invoke).toHaveBeenLastCalledWith("adult_batch_preview", {
    input: { media_ids: ["synthetic-media"] },
  });
  const cancelled = {
    ...batch,
    state: "cancelled",
    items: [
      {
        media_item_id: "synthetic-media",
        preview_id: "synthetic-output",
        state: "cancelled",
        result: null,
        error_code: null,
      },
    ],
  };
  respond(cancelled);
  await adultApi.batchCancel("synthetic-batch");
  expect(invoke).toHaveBeenLastCalledWith("adult_batch_cancel", {
    input: { batch_id: "synthetic-batch" },
  });
  expect(adultBatchResult(cancelled)).toBe(true);
  expect(
    adultBatchPreview({
      ...batch,
      items: Array(33).fill(syntheticAdultOutput),
    }),
  ).toBe(false);
  expect(
    vi
      .mocked(invoke)
      .mock.calls.some(
        ([name]) =>
          name === "adult_output_execute" || name === "adult_batch_execute",
      ),
  ).toBe(false);
});
it("accepts only validated local thumbnail data and refuses remote images", async () => {
  respond({ thumbnail: "https://pics.dmm.co.jp/synthetic.png" });
  await expect(
    adultApi.artworkPreview("synthetic-run", 0, 1),
  ).rejects.toMatchObject({ code: "invalid_response" });
  respond({ thumbnail: "data:image/png;base64,aGVsbG8=" });
  await expect(
    adultApi.artworkPreview("synthetic-run", 0, 1),
  ).resolves.toHaveProperty("thumbnail");
});
it("refuses an adult channel response containing ordinary media", async () => {
  respond({
    items: [
      {
        media: { id: "ordinary", title: "Ordinary", kind: "movie" },
        poster_uri: null,
        health: [],
      },
    ],
    next_cursor: null,
  });
  await expect(
    adultApi.media({
      root_id: null,
      kind: "adult_video",
      query: null,
      health: null,
      genre: null,
      person: null,
      studio: null,
      year: null,
      sort: "updated_desc",
      cursor: null,
      limit: 48,
      include_adult: true,
    }),
  ).rejects.toMatchObject({ code: "invalid_response" });
});
