import { invoke, isTauri } from "@tauri-apps/api/core";
import { beforeEach, expect, it, vi } from "vitest";
import sharedRustDto from "../../../../crates/application/tests/fixtures/review_queue.json";
import { type ReviewQueue, reviewQueueApi } from "./review-queue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), isTauri: vi.fn() }));
const queueId = "11111111-1111-4111-8111-111111111111";
const mediaId = "22222222-2222-4222-8222-222222222222";
const previewId = "33333333-3333-4333-8333-333333333333";
const value: ReviewQueue = {
  id: queueId,
  revision: 0,
  current_media_id: mediaId,
  context: {
    kind: "movie",
    q: "Film",
    health: "incomplete_metadata",
    sort: "title_asc",
  },
  items: [
    {
      media_id: mediaId,
      position: 0,
      state: "pending",
      checkpoint: null,
      preview_id: null,
      needs_recheck: false,
      reason: null,
    },
  ],
  created_at: "2026-11-04T10:00:00Z",
  updated_at: "2026-11-04T10:00:00Z",
};
function respond(data: unknown) {
  vi.mocked(invoke).mockResolvedValue({ schema_version: 1, data });
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(true);
  respond(value);
});

it("uses explicit ledger commands and keeps confirmation separate from bookkeeping", async () => {
  await expect(
    reviewQueueApi.create([mediaId], value.context),
  ).resolves.toEqual(value);
  expect(invoke).toHaveBeenLastCalledWith("review_queue_create", {
    input: { media_ids: [mediaId], context: value.context },
  });
  await reviewQueueApi.get(queueId);
  expect(invoke).toHaveBeenLastCalledWith("review_queue_get", {
    input: { queue_id: queueId },
  });
  respond({
    ...value,
    revision: 1,
    items: [{ ...value.items[0], checkpoint: queueId, preview_id: previewId }],
  });
  await reviewQueueApi.checkpoint({
    queue_id: queueId,
    media_id: mediaId,
    expected_revision: 0,
    preview_id: previewId,
  });
  expect(invoke).toHaveBeenLastCalledWith("review_queue_checkpoint", {
    input: {
      queue_id: queueId,
      media_id: mediaId,
      expected_revision: 0,
      preview_id: previewId,
    },
  });
  await reviewQueueApi.update({
    queue_id: queueId,
    media_id: mediaId,
    expected_revision: 0,
    action: "defer",
  });
  expect(invoke).toHaveBeenLastCalledWith("review_queue_update", {
    input: {
      queue_id: queueId,
      media_id: mediaId,
      expected_revision: 0,
      action: "defer",
    },
  });
  respond([value]);
  await expect(reviewQueueApi.list()).resolves.toEqual([value]);
  expect(invoke).toHaveBeenLastCalledWith("review_queue_list", { input: {} });
  expect(
    vi
      .mocked(invoke)
      .mock.calls.every(([command]) =>
        String(command).startsWith("review_queue_"),
      ),
  ).toBe(true);
});

it("rejects an unbound checkpoint before the workspace could confirm it", async () => {
  await expect(
    reviewQueueApi.checkpoint({
      queue_id: queueId,
      media_id: mediaId,
      expected_revision: 0,
      preview_id: previewId,
    }),
  ).rejects.toMatchObject({ code: "invalid_response" });
});

it.each([
  { ...value, id: mediaId },
  { ...value, context: { kind: "adult_video" } },
  { ...value, context: { returnUrl: "https://external.example" } },
  { ...value, context: { root: null } },
  { ...value, items: [] },
  { ...value, items: [value.items[0], value.items[0]] },
  { ...value, items: [{ ...value.items[0], state: "success" }] },
  { ...value, items: [{ ...value.items[0], reason: "raw-error-secret" }] },
  { ...value, current_media_id: previewId },
])("rejects mismatched or unsafe ledger replies", async (data) => {
  respond(data);
  await expect(reviewQueueApi.get(queueId)).rejects.toMatchObject({
    code: "invalid_response",
  });
});

it("preserves safe IPC errors and never uses a browser to make ledger mutations", async () => {
  vi.mocked(invoke).mockRejectedValue("secret=not-for-display");
  await expect(reviewQueueApi.get(queueId)).rejects.toMatchObject({
    code: "request_failed",
    envelope: undefined,
  });
  vi.clearAllMocks();
  vi.mocked(isTauri).mockReturnValue(false);
  await expect(reviewQueueApi.create([mediaId], {})).rejects.toMatchObject({
    code: "desktop_required",
  });
  expect(invoke).not.toHaveBeenCalled();
});

it("accepts the same serialized DTO fixture verified by Rust", async () => {
  respond(sharedRustDto);
  await expect(reviewQueueApi.get(sharedRustDto.id)).resolves.toEqual(
    sharedRustDto,
  );
});
