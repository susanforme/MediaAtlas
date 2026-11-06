import { queryOptions } from "@tanstack/react-query";
import { defaultBrowse, libraryApi } from "../../api/library";
import { reviewQueueApi, type ReviewQueue } from "../../api/review-queue";
import { scraperOutputApi, type VideoKind } from "../../api/scraper";

export const scraperHistoryQuery = queryOptions({ queryKey: ["scraper-history"], queryFn: scraperOutputApi.history });
export const reviewQueuesQuery = queryOptions({ queryKey: ["review-queues"], queryFn: reviewQueueApi.list, retry: false });
export const reviewQueueQuery = (id: string) => queryOptions({ queryKey: ["review-queue", id], queryFn: () => reviewQueueApi.get(id), retry: false });
export const scraperLocalQuery = (kind: VideoKind, query: string) => queryOptions({
  queryKey: ["scraper-local", kind, query],
  queryFn: () => libraryApi.toolsMedia({ ...defaultBrowse(false), kind: kind === "tv" ? "tv_series" : kind, query: query || null, limit: 48 }),
});
export const outputDefaultQuery = (target: string) => queryOptions({
  queryKey: ["roots", "nfo-export-mode", target],
  queryFn: () => scraperOutputApi.defaultMode(target),
  retry: false,
  refetchOnWindowFocus: false,
});

export function reviewMediaId(queue: ReviewQueue | undefined, requested?: string) {
  return requested && queue?.items.some((item) => item.media_id === requested)
    ? requested
    : queue?.current_media_id ?? queue?.items[0]?.media_id;
}

export function reviewCanWork(queue: ReviewQueue | undefined, mediaId: string | undefined, confirmedHere?: string) {
  const selected = queue?.items.find((item) => item.media_id === mediaId);
  return Boolean(selected && !selected.needs_recheck &&
    selected.reason !== "missing_media" && selected.reason !== "unavailable_media" &&
    ((selected.state === "pending" && queue?.current_media_id === mediaId) ||
      (selected.state === "confirmed" && confirmedHere === mediaId)));
}
