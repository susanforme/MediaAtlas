import { QueryClient } from "@tanstack/react-query";
import { waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { adultApi } from "../api/adult";
import type { AdultSettings } from "../api/adult-privacy";
import { libraryApi } from "../api/library";
import { musicApi } from "../api/music";
import { providerApi } from "../api/providers";
import { type ReviewQueue, reviewQueueApi } from "../api/review-queue";
import { scraperOutputApi } from "../api/scraper";
import { prepareLibraryPage } from "../features/library/route-data";
import { syntheticMusicGraph } from "../features/music/fixtures";
import { musicAlbumQuery, musicCoverQuery } from "../features/music/queries";
import { prepareAlbumPage } from "../features/music/route-data";
import { reviewQueuesQuery } from "../features/scraper/queries";
import { prepareScraperPage } from "../features/scraper/route-data";
import { prepareToolsPage } from "../features/tools/route-data";
import { cancelRoutePreparations, prepareDesktopRoute, type RoutePreparation } from "./route-transition";

vi.mock("../api/review-queue", async (original) => {
  const module = await original<typeof import("../api/review-queue")>();
  return { ...module, reviewQueueApi: { ...module.reviewQueueApi, get: vi.fn(), list: vi.fn(), create: vi.fn() } };
});
vi.mock("../api/scraper", async (original) => {
  const module = await original<typeof import("../api/scraper")>();
  return { ...module, scraperOutputApi: { ...module.scraperOutputApi, history: vi.fn(), defaultMode: vi.fn() } };
});

const closed: AdultSettings = { entry_visible: false, unlocked: false, covers_visible: false, remember_recent: false };
const granted: AdultSettings = { ...closed, entry_visible: true, unlocked: true, covers_visible: true };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function context(settings = closed): RoutePreparation {
  return { queryClient: new QueryClient({ defaultOptions: { queries: { retry: false, retryOnMount: false } } }), readSettings: () => settings, check: () => {} };
}

beforeEach(() => {
  vi.restoreAllMocks();
  vi.mocked(reviewQueueApi.get).mockReset();
  vi.mocked(reviewQueueApi.list).mockReset().mockResolvedValue([]);
  vi.mocked(reviewQueueApi.create).mockReset();
  vi.mocked(scraperOutputApi.history).mockReset().mockResolvedValue([]);
  vi.mocked(scraperOutputApi.defaultMode).mockReset().mockResolvedValue("online");
  vi.spyOn(providerApi, "snapshot").mockResolvedValue({ schema_version: 1, providers: [], tasks: [], requests: [] });
  vi.spyOn(libraryApi, "roots").mockResolvedValue([]);
  vi.spyOn(libraryApi, "toolsMedia").mockResolvedValue({ items: [], next_cursor: null });
  vi.spyOn(libraryApi, "toolsDetail").mockRejectedValue(new Error("Missing media"));
  vi.spyOn(adultApi, "roots").mockResolvedValue([]);
  vi.spyOn(adultApi, "media").mockResolvedValue({ items: [], next_cursor: null });
});
afterEach(cancelRoutePreparations);

it("does not delay an album's core data for a slow or failed optional cover", async () => {
  const route = context();
  const cover = deferred<Awaited<ReturnType<typeof musicApi.cachedCover>>>();
  vi.spyOn(musicApi, "album").mockResolvedValue(syntheticMusicGraph);
  vi.spyOn(musicApi, "cachedCover").mockReturnValue(cover.promise);
  await expect(prepareAlbumPage(route, "album")).resolves.toBeUndefined();
  expect(route.queryClient.getQueryData(musicAlbumQuery("album").queryKey)).toBe(syntheticMusicGraph);
  expect(route.queryClient.getQueryState(musicCoverQuery("album").queryKey)?.fetchStatus).toBe("fetching");
  cover.reject(new Error("Cover unavailable"));
  await waitFor(() => expect(route.queryClient.getQueryState(musicCoverQuery("album").queryKey)?.status).toBe("error"));
  expect(route.queryClient.getQueryData(musicAlbumQuery("album").queryKey)).toBe(syntheticMusicGraph);
});

it("retains local tools when saved review queues fail and leaves their error available to the page", async () => {
  const route = context();
  vi.mocked(reviewQueueApi.list).mockRejectedValue(new Error("Queues unavailable"));
  await expect(prepareToolsPage(route, {})).resolves.toBeUndefined();
  expect(libraryApi.toolsMedia).toHaveBeenCalledTimes(1);
  await waitFor(() => expect(route.queryClient.getQueryState(reviewQueuesQuery.queryKey)?.status).toBe("error"));
  expect(reviewQueueApi.create).not.toHaveBeenCalled();
});

it("keeps anonymous source lookup accessible when optional local media lookup fails", async () => {
  const route = context();
  vi.mocked(libraryApi.toolsMedia).mockRejectedValue(new Error("Local list unavailable"));
  await expect(prepareScraperPage(route, {})).resolves.toBeUndefined();
  expect(libraryApi.toolsDetail).not.toHaveBeenCalled();
  expect(reviewQueueApi.create).not.toHaveBeenCalled();
});

it.each(["missing_media", "unavailable_media"] as const)("restores a %s queue item without requesting its unavailable media", async (reason) => {
  const queue: ReviewQueue = {
    id: "11111111-1111-4111-8111-111111111111", revision: 1, current_media_id: "missing", context: {},
    items: [{ media_id: "missing", position: 0, state: "pending", checkpoint: null, preview_id: null, needs_recheck: true, reason }],
    created_at: "2026-11-06T00:00:00Z", updated_at: "2026-11-06T00:00:00Z",
  };
  vi.mocked(reviewQueueApi.get).mockResolvedValue(queue);
  await expect(prepareScraperPage(context(), { session: queue.id })).resolves.toBeUndefined();
  expect(libraryApi.toolsDetail).not.toHaveBeenCalled();
  expect(scraperOutputApi.defaultMode).not.toHaveBeenCalled();
  expect(reviewQueueApi.create).not.toHaveBeenCalled();
});

it("does not request a private route without a current grant", async () => {
  await prepareLibraryPage(context(), { kind: "adult_video" });
  expect(adultApi.roots).not.toHaveBeenCalled();
  expect(adultApi.media).not.toHaveBeenCalled();
});

it("does not restore cleared private query data when an older route response finishes", async () => {
  const route = context(granted);
  const response = deferred<Awaited<ReturnType<typeof adultApi.media>>>();
  vi.mocked(adultApi.media).mockReturnValue(response.promise);
  const pending = prepareDesktopRoute(new AbortController(), async () => {}, {}, (check) => prepareLibraryPage({ ...route, check }, { kind: "adult_video" }));
  const rejected = expect(pending).rejects.toBeDefined();
  await waitFor(() => expect(adultApi.media).toHaveBeenCalledTimes(1));
  cancelRoutePreparations();
  await route.queryClient.cancelQueries();
  route.queryClient.clear();
  response.resolve({ items: [], next_cursor: null });
  await rejected;
  expect(route.queryClient.getQueryCache().findAll()).toHaveLength(0);
});
