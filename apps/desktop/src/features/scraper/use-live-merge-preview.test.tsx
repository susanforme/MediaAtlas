import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  type MergePreview,
  type MergeRequest,
  scraperApi,
} from "../../api/scraper";
import { useLiveMergePreview } from "./use-live-merge-preview";

const request: MergeRequest = {
  run_id: "run",
  candidate_index: 0,
  media_item_id: "media",
  selected_fields: ["title"],
  selected_artwork: [],
};
const preview: MergePreview = {
  schema_version: 1,
  preview_id: "preview",
  media_item_id: "media",
  fields: [],
  artwork: [],
  confirmed: false,
};
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}
async function tick(milliseconds = 200) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(milliseconds);
  });
}

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

it("debounces changing selections, keeps fields editable and validates only the latest request", async () => {
  const api = vi.spyOn(scraperApi, "preview").mockResolvedValue(preview);
  const confirm = vi.spyOn(scraperApi, "confirm");
  const { result, rerender } = renderHook(
    (value: MergeRequest | null) => useLiveMergePreview(value),
    { initialProps: request as MergeRequest | null },
  );
  expect(result.current.pending).toBe(true);
  await tick(150);
  rerender({ ...request, selected_fields: ["overview"] });
  await tick(150);
  expect(api).not.toHaveBeenCalled();
  await tick(50);
  expect(api).toHaveBeenCalledExactlyOnceWith({
    ...request,
    selected_fields: ["overview"],
  });
  expect(result.current.preview).toEqual(preview);
  expect(result.current.pending).toBe(false);
  rerender({ ...request, selected_fields: ["overview"] });
  await tick();
  expect(api).toHaveBeenCalledTimes(1);
  expect(confirm).not.toHaveBeenCalled();
});

it("hides a previous proof immediately and ignores responses that finish out of order", async () => {
  const first = deferred<MergePreview>();
  const second = deferred<MergePreview>();
  const api = vi
    .spyOn(scraperApi, "preview")
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise)
    .mockResolvedValue({ ...preview, preview_id: "fresh-original-selection" });
  const { result, rerender } = renderHook(
    (value: MergeRequest | null) => useLiveMergePreview(value),
    { initialProps: request as MergeRequest | null },
  );
  await tick();
  rerender({ ...request, selected_fields: ["overview"] });
  expect(result.current.preview).toBeUndefined();
  expect(result.current.pending).toBe(true);
  await tick();
  await act(async () => {
    second.resolve({ ...preview, preview_id: "latest" });
  });
  expect(result.current.preview?.preview_id).toBe("latest");
  await act(async () => {
    first.resolve({ ...preview, preview_id: "stale" });
  });
  expect(result.current.preview?.preview_id).toBe("latest");
  rerender({ ...request });
  expect(result.current.preview).toBeUndefined();
  await tick();
  expect(api).toHaveBeenCalledTimes(3);
  expect(result.current.preview?.preview_id).toBe("fresh-original-selection");
});

it("null selection cancels the timer and a returning selection requires a fresh proof", async () => {
  const api = vi.spyOn(scraperApi, "preview").mockResolvedValue(preview);
  const { result, rerender } = renderHook(
    (value: MergeRequest | null) => useLiveMergePreview(value),
    { initialProps: request as MergeRequest | null },
  );
  rerender(null);
  await tick();
  expect(api).not.toHaveBeenCalled();
  expect(result.current.pending).toBe(false);
  rerender(request);
  await tick();
  expect(result.current.preview).toEqual(preview);
  rerender(null);
  expect(result.current.preview).toBeUndefined();
  rerender(request);
  expect(result.current.preview).toBeUndefined();
  expect(result.current.pending).toBe(true);
  await tick();
  expect(api).toHaveBeenCalledTimes(2);
});

it("shows a failed validation without automatic retries and retries only on request", async () => {
  const error = new Error("validation unavailable");
  const api = vi
    .spyOn(scraperApi, "preview")
    .mockRejectedValueOnce(error)
    .mockResolvedValue(preview);
  const { result } = renderHook(() => useLiveMergePreview(request));
  await tick();
  expect(result.current.error).toBe(error);
  expect(result.current.preview).toBeUndefined();
  expect(result.current.pending).toBe(false);
  await tick(1000);
  expect(api).toHaveBeenCalledTimes(1);
  act(() => result.current.retry());
  expect(result.current.error).toBeUndefined();
  expect(result.current.pending).toBe(true);
  await tick();
  expect(result.current.preview).toEqual(preview);
  expect(api).toHaveBeenCalledTimes(2);
});

it("unmount cancels unsent validation and drops late responses from an already sent request", async () => {
  const response = deferred<MergePreview>();
  const api = vi.spyOn(scraperApi, "preview").mockReturnValue(response.promise);
  const unsent = renderHook(() => useLiveMergePreview(request));
  unsent.unmount();
  await tick();
  expect(api).not.toHaveBeenCalled();
  const sent = renderHook(() => useLiveMergePreview(request));
  await tick();
  expect(api).toHaveBeenCalledTimes(1);
  sent.unmount();
  await act(async () => response.resolve(preview));
  expect(sent.result.current.preview).toBeUndefined();
});
