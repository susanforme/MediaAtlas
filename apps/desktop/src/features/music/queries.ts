import { queryOptions } from "@tanstack/react-query";
import { musicApi } from "../../api/music";

export const musicLibraryQuery = queryOptions({ queryKey: ["music-library"], queryFn: musicApi.library });
export const musicToolsLibraryQuery = queryOptions({ queryKey: ["music-tools-library"], queryFn: musicApi.toolsLibrary });
export const musicAlbumQuery = (id: string) => queryOptions({ queryKey: ["music-album", id], queryFn: () => musicApi.album(id) });
export const musicArtistQuery = (id: string) => queryOptions({ queryKey: ["music-artist", id], queryFn: () => musicApi.artist(id) });
export const musicCoverQuery = (id: string) => queryOptions({ queryKey: ["music-cached-cover", id], queryFn: () => musicApi.cachedCover(id) });
