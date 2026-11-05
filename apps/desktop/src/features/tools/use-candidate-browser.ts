import { useQuery } from "@tanstack/react-query";
import { useLayoutEffect, useRef, useState } from "react";
import { adultApi } from "../../api/adult";
import { libraryApi, type MediaCard } from "../../api/library";
import { usePrivacy } from "../library/context";
import {
  adultCandidates,
  changeCandidateSelection,
  ordinaryCandidates,
} from "./candidate-model";
import {
  changeToolsRouteFilters,
  type ToolsRouteFilters,
  type ToolsRouteSearch,
  toolsRouteBrowseQuery,
  validateToolsRouteSearch,
} from "./search";

export function useCandidateBrowser(
  initialSearch: ToolsRouteSearch,
  onSearch?: (search: ToolsRouteSearch, replace?: boolean) => void,
) {
  const [localSearch, setLocalSearch] = useState(initialSearch);
  const rawSearch = validateToolsRouteSearch(
    onSearch
      ? (initialSearch as Record<string, unknown>)
      : (localSearch as Record<string, unknown>),
  );
  const { includeAdult, settings, loading, error } = usePrivacy();
  const adult = rawSearch.kind === "adult_video";
  const allowed = !adult || includeAdult;
  const scope = JSON.stringify([
    rawSearch.q,
    rawSearch.root,
    rawSearch.kind,
    rawSearch.health,
    rawSearch.sort,
  ]);
  const [selection, setSelection] = useState<{
    scope: string;
    cards: MediaCard[];
    limitReached: boolean;
  }>({ scope, cards: [], limitReached: false });
  const filterChanged = selection.scope !== scope;
  const search = filterChanged
    ? changeToolsRouteFilters(rawSearch, {})
    : rawSearch;
  const history = useRef<{ scope: string; cursors: (string | undefined)[] }>({
    scope,
    cursors: [undefined],
  });
  function navigate(next: ToolsRouteSearch, replace = false) {
    if (onSearch) onSearch(next, replace);
    else setLocalSearch(next);
  }
  useLayoutEffect(() => {
    if (!filterChanged) return;
    setSelection({ scope, cards: [], limitReached: false });
    history.current = { scope, cursors: [undefined] };
    if (rawSearch.cursor || rawSearch.page || rawSearch.media)
      navigate(changeToolsRouteFilters(rawSearch, {}), true);
  });
  const query = toolsRouteBrowseQuery(search);
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
    enabled: allowed,
  });
  const media = useQuery({
    queryKey: adult ? ["adult", "tools-media", query] : ["tools-media", query],
    queryFn: () =>
      adult ? adultApi.toolsMedia(query) : libraryApi.toolsMedia(query),
    enabled: allowed,
    select: (value) => ({
      ...value,
      items: adult
        ? adultCandidates(value.items)
        : ordinaryCandidates(value.items),
    }),
  });
  const selected = filterChanged ? [] : selection.cards;
  const page = search.page ?? 1;
  function filter(patch: ToolsRouteFilters) {
    setSelection({ scope, cards: [], limitReached: false });
    history.current = { scope, cursors: [undefined] };
    navigate(changeToolsRouteFilters(search, patch));
  }
  function select(cards: MediaCard[], checked: boolean) {
    if (adult || !allowed) return;
    // Callers supply records from this backend page; arbitrary IDs never enter the queue.
    const present = new Map(
      media.data?.items.map((card) => [card.media.id, card]),
    );
    const requested = cards.flatMap((card) => {
      const actual = present.get(card.media.id);
      return actual ? [actual] : [];
    });
    setSelection((old) => ({
      scope,
      ...changeCandidateSelection(
        old.scope === scope ? old.cards : [],
        requested,
        checked,
      ),
    }));
  }
  function previous() {
    if (page <= 1) return;
    const cursor =
      history.current.scope === scope
        ? history.current.cursors[page - 2]
        : undefined;
    navigate(
      validateToolsRouteSearch({
        ...search,
        cursor,
        page: cursor ? page - 1 : undefined,
      }),
    );
  }
  function next() {
    const cursor = media.data?.next_cursor;
    if (!cursor) return;
    if (history.current.scope !== scope)
      history.current = { scope, cursors: [undefined] };
    history.current.cursors[page - 1] = search.cursor;
    history.current.cursors[page] = cursor;
    navigate({ ...search, cursor, page: page + 1 });
  }
  return {
    adult,
    allowed,
    privacyLoading: loading,
    privacyError: error,
    adultEnabled: settings.entry_visible,
    search,
    roots,
    media,
    selected,
    page,
    filter,
    previous,
    next,
    select,
    limitReached: !filterChanged && selection.limitReached,
    clearSelection: () =>
      setSelection({ scope, cards: [], limitReached: false }),
    view: (id: string) => navigate({ ...search, media: id }, true),
    clearFilters: () => {
      setSelection({
        scope,
        cards: [],
        limitReached: false,
      });
      history.current = { scope, cursors: [undefined] };
      navigate(validateToolsRouteSearch({ kind: search.kind }));
    },
    removeSelected: (id: string) =>
      setSelection((old) => ({
        scope,
        cards:
          old.scope === scope
            ? old.cards.filter((card) => card.media.id !== id)
            : [],
        limitReached: false,
      })),
    closeDetail: () =>
      navigate(validateToolsRouteSearch({ ...search, media: undefined }), true),
  };
}
