import type { RoutePreparation } from "../../lib/route-transition";
import { providersQuery } from "../providers/queries";
import type { LibrarySearch } from "./browse-model";
import { browseQuery, facetsQuery, mediaDetailQuery, mediaQuery, rootsQuery, scanTasksQuery } from "./queries";

function adultAllowed(context: RoutePreparation) {
  const settings = context.readSettings();
  return Boolean(settings?.entry_visible && settings.unlocked);
}

export async function prepareLibraryPage(context: RoutePreparation, search: LibrarySearch) {
  context.check();
  const adult = search.kind === "adult_video";
  if (adult && !adultAllowed(context)) return;
  const query = browseQuery(search, adult);
  if (!adult) {
    void context.queryClient.prefetchQuery({ ...facetsQuery(query), staleTime: Infinity });
    void context.queryClient.prefetchQuery({ ...scanTasksQuery(), staleTime: Infinity });
  }
  await Promise.all([
    context.queryClient.ensureQueryData(rootsQuery(adult)),
    context.queryClient.ensureQueryData(mediaQuery(query, adult, context.readSettings()?.covers_visible)),
  ]);
  context.check();
}

export async function prepareMediaPage(context: RoutePreparation, mediaId: string, adult: boolean) {
  context.check();
  if (adult && !adultAllowed(context)) return;
  await context.queryClient.ensureQueryData(mediaDetailQuery(mediaId, adult));
  context.check();
}

export async function prepareDirectoriesPage(context: RoutePreparation) {
  context.check();
  await Promise.all([
    context.queryClient.ensureQueryData(rootsQuery()),
    ...(adultAllowed(context) ? [context.queryClient.prefetchQuery({ ...rootsQuery(true), staleTime: Infinity })] : []),
  ]);
  context.check();
}

export async function prepareScannerPage(context: RoutePreparation) {
  context.check();
  if (adultAllowed(context)) void context.queryClient.prefetchQuery({ ...scanTasksQuery(true), staleTime: Infinity });
  await Promise.all([
    prepareDirectoriesPage(context),
    context.queryClient.ensureQueryData(scanTasksQuery()),
  ]);
  context.check();
}

export async function prepareTasksPage(context: RoutePreparation) {
  context.check();
  void context.queryClient.prefetchQuery({ ...providersQuery(), staleTime: Infinity });
  await prepareScannerPage(context);
  context.check();
}
