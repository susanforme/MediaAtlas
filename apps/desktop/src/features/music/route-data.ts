import type { RoutePreparation } from "../../lib/route-transition";
import { musicAlbumQuery, musicArtistQuery, musicCoverQuery, musicLibraryQuery, musicToolsLibraryQuery } from "./queries";

export async function prepareMusicPage(context: RoutePreparation, tools = false) {
  context.check();
  await context.queryClient.ensureQueryData(tools ? musicToolsLibraryQuery : musicLibraryQuery);
  context.check();
}

export async function prepareAlbumPage(context: RoutePreparation, releaseId: string) {
  context.check();
  void context.queryClient.prefetchQuery({ ...musicCoverQuery(releaseId), staleTime: Infinity });
  await context.queryClient.ensureQueryData(musicAlbumQuery(releaseId));
  context.check();
}

export async function prepareArtistPage(context: RoutePreparation, artistId: string) {
  context.check();
  await context.queryClient.ensureQueryData(musicArtistQuery(artistId));
  context.check();
}
