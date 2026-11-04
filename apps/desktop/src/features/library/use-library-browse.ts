import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import { type LibrarySearch, libraryBrowseQuery } from "./browse-model";
import { usePrivacy } from "./context";
import { useLibraryActivity } from "./use-library-activity";

export function useLibraryBrowse(
  search: LibrarySearch,
  onSearch: (patch: LibrarySearch, replace?: boolean) => void,
  adult: boolean,
) {
  const [advanced, setAdvanced] = useState(false);
  const [history, setHistory] = useState<(string | undefined)[]>([undefined]);
  const { settings, includeAdult } = usePrivacy();
  const query = {
    ...libraryBrowseQuery(search, adult),
    ...(adult
      ? {
          include_adult: true,
          genre: null,
          person: null,
          studio: null,
          year: null,
        }
      : {}),
  };
  const allowed = !adult || (settings.entry_visible && includeAdult);
  const activity = useLibraryActivity(search.root, !adult);
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
    enabled: allowed,
  });
  const media = useQuery({
    queryKey: adult
      ? ["adult", "library", query, settings.covers_visible]
      : ["media", query],
    queryFn: () => (adult ? adultApi.media(query) : libraryApi.media(query)),
    select: (result) => ({
      ...result,
      items: result.items
        .filter((card) =>
          adult
            ? card.media.kind === "adult_video"
            : card.media.kind !== "adult_video",
        )
        .map((card) =>
          adult && !settings.covers_visible
            ? { ...card, poster_uri: null }
            : card,
        ),
    }),
    enabled: allowed,
  });
  const facets = useQuery({
    queryKey: ["facets", { ...query, cursor: null }],
    queryFn: () => libraryApi.facets({ ...query, cursor: null }),
    enabled: allowed && !adult,
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
    reset: () => {
      setHistory([undefined]);
      onSearch({ view: search.view });
    },
    activity,
    adultAvailable: settings.entry_visible,
  };
}
