import type { RoutePreparation } from "../../lib/route-transition";
import { providersQuery } from "../providers/queries";
import { toolsDetailQuery } from "../tools/queries";
import { outputDefaultQuery, reviewCanWork, reviewMediaId, reviewQueueQuery, scraperHistoryQuery, scraperLocalQuery } from "./queries";
import { scrapeKind } from "./scraper-model";
import type { ScraperSearch } from "./search";

export async function prepareScraperPage(context: RoutePreparation, search: ScraperSearch) {
  context.check();
  void context.queryClient.prefetchQuery({ ...providersQuery(), staleTime: Infinity });
  void context.queryClient.prefetchQuery({ ...scraperHistoryQuery, staleTime: Infinity });
  const queue = search.session ? context.queryClient.ensureQueryData(reviewQueueQuery(search.session)) : undefined;
  const restored = await queue;
  const mediaId = search.session ? reviewMediaId(restored, search.media) : search.media;
  context.check();
  if (search.session && !reviewCanWork(restored, mediaId)) return;
  if (!mediaId) {
    void context.queryClient.prefetchQuery({ ...scraperLocalQuery("movie", ""), staleTime: Infinity });
  } else {
    const detail = await context.queryClient.ensureQueryData(toolsDetailQuery(mediaId));
    context.check();
    if (detail.media.id === mediaId && scrapeKind(detail.media.kind))
      void context.queryClient.prefetchQuery({ ...outputDefaultQuery(mediaId), staleTime: Infinity });
  }
  context.check();
}
