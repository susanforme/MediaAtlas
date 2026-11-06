import { queryOptions } from "@tanstack/react-query";
import { adultApi } from "../../api/adult";
import { libraryApi, type BrowseQuery } from "../../api/library";
import { type LibrarySearch, libraryBrowseQuery } from "./browse-model";

export const rootsQuery = (adult = false) => queryOptions({
  queryKey: adult ? ["adult", "roots"] : ["roots", false],
  queryFn: () => adult ? adultApi.roots() : libraryApi.roots(false),
});

export const scanTasksQuery = (adult = false) => queryOptions({
  queryKey: adult ? ["adult", "scan-tasks"] : ["tasks", false],
  queryFn: () => adult ? adultApi.tasks() : libraryApi.tasks(false),
});

export function browseQuery(search: LibrarySearch, adult: boolean) {
  return {
    ...libraryBrowseQuery(search, adult),
    ...(adult ? { include_adult: true, genre: null, person: null, studio: null, year: null } : {}),
  };
}

export const mediaQuery = (query: BrowseQuery, adult = false, coversVisible = false) => queryOptions({
  queryKey: adult ? ["adult", "library", query, coversVisible] : ["media", query],
  queryFn: () => adult ? adultApi.media(query) : libraryApi.media(query),
});

export const facetsQuery = (query: BrowseQuery) => queryOptions({
  queryKey: ["facets", { ...query, cursor: null }],
  queryFn: () => libraryApi.facets({ ...query, cursor: null }),
});

export const mediaDetailQuery = (mediaId: string, adult = false) => queryOptions({
  queryKey: adult ? ["adult", "media", mediaId] : ["detail", mediaId, false],
  queryFn: () => adult ? adultApi.getMedia(mediaId) : libraryApi.detail(mediaId, false),
});
