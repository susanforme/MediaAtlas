import { queryOptions } from "@tanstack/react-query";
import { adultApi } from "../../api/adult";
import { libraryApi, type BrowseQuery } from "../../api/library";
import { maintenanceApi } from "../../api/organization";

export const toolsMediaQuery = (query: BrowseQuery, adult = false) => queryOptions({
  queryKey: adult ? ["adult", "tools-media", query] : ["tools-media", query],
  queryFn: () => adult ? adultApi.toolsMedia(query) : libraryApi.toolsMedia(query),
});

export const toolsDetailQuery = (mediaId: string, adult = false) => queryOptions({
  queryKey: adult ? ["adult", "tools-detail", mediaId] : ["tools-detail", mediaId],
  queryFn: () => adult ? adultApi.toolsDetail(mediaId) : libraryApi.toolsDetail(mediaId),
});

export const watchersQuery = queryOptions({ queryKey: ["watchers"], queryFn: maintenanceApi.watchers });
