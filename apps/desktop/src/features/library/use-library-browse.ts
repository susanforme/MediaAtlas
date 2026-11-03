import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { libraryApi } from "../../api/library";
import { type LibrarySearch, libraryBrowseQuery } from "./browse-model";

export function useLibraryBrowse(
  search: LibrarySearch,
  onSearch: (patch: LibrarySearch, replace?: boolean) => void,
  adult: boolean,
) {
  const [advanced, setAdvanced] = useState(false);
  const [history, setHistory] = useState<(string | undefined)[]>([undefined]);
  const access = false;
  const query = libraryBrowseQuery(search, adult);
  const allowed = !adult;
  const roots = useQuery({
    queryKey: ["roots", access],
    queryFn: () => libraryApi.roots(access),
    enabled: allowed,
  });
  const media = useQuery({
    queryKey: ["media", query],
    queryFn: () => libraryApi.media(query),
    select: (result) => ({
      ...result,
      items: result.items.filter((card) => card.media.kind !== "adult_video"),
    }),
    enabled: allowed,
  });
  const facets = useQuery({
    queryKey: ["facets", { ...query, cursor: null }],
    queryFn: () => libraryApi.facets({ ...query, cursor: null }),
    enabled: allowed,
  });
  function filter(patch: LibrarySearch) {
    setHistory([undefined]);
    onSearch({ ...search, ...patch, cursor: undefined, page: 1 });
  }
  return {
    advanced,
    setAdvanced,
    history,
    setHistory,
    media,
    roots,
    facets,
    filter,
    allowed,
  };
}
