import type { RoutePreparation } from "../../lib/route-transition";
import { rootsQuery } from "../library/queries";
import { reviewQueuesQuery } from "../scraper/queries";
import { toolsDetailQuery, toolsMediaQuery, watchersQuery } from "./queries";
import { type ToolsRouteSearch, toolsRouteBrowseQuery } from "./search";

export async function prepareToolsPage(context: RoutePreparation, search: ToolsRouteSearch) {
  context.check();
  const adult = search.kind === "adult_video";
  const settings = context.readSettings();
  if (adult && !(settings?.entry_visible && settings.unlocked)) return;
  if (!adult && search.view !== "health") void context.queryClient.prefetchQuery({ ...reviewQueuesQuery, staleTime: Infinity });
  await Promise.all([
    context.queryClient.ensureQueryData(rootsQuery(adult)),
    ...(!adult && search.view === "health" ? [context.queryClient.ensureQueryData(watchersQuery)] : [
      context.queryClient.ensureQueryData(toolsMediaQuery(toolsRouteBrowseQuery(search), adult)),
      ...(search.media ? [context.queryClient.ensureQueryData(toolsDetailQuery(search.media, adult))] : []),
    ]),
  ]);
  context.check();
}
