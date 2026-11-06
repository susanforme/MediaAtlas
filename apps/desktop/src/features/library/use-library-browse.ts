import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import type { LibrarySearch } from "./browse-model";
import { usePrivacy } from "./context";
import { browseQuery, facetsQuery, mediaQuery, rootsQuery } from "./queries";
import { useLibraryActivity } from "./use-library-activity";

export function useLibraryBrowse(
  search: LibrarySearch,
  onSearch: (patch: LibrarySearch, replace?: boolean) => void,
  adult: boolean,
) {
  const [history, setHistory] = useState<(string | undefined)[]>([undefined]);
  const { settings, includeAdult } = usePrivacy();
  const query = browseQuery(search, adult);
  const allowed = !adult || (settings.entry_visible && includeAdult);
  const activity = useLibraryActivity(search.root, !adult);
  const roots = useQuery({
    ...rootsQuery(adult),
    enabled: allowed,
  });
  const media = useQuery({
    ...mediaQuery(query, adult, settings.covers_visible),
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
    ...facetsQuery(query),
    enabled: allowed && !adult,
  });
  function filter(patch: LibrarySearch) {
    setHistory([undefined]);
    onSearch({ ...search, ...patch, cursor: undefined, page: 1 });
  }
  return {
    history,
    setHistory,
    media,
    roots,
    facets,
    filter,
    allowed,
    reset: () => {
      setHistory([undefined]);
      onSearch({ root: search.root, kind: search.kind, view: search.view });
    },
    activity,
    adultAvailable: settings.entry_visible,
  };
}
