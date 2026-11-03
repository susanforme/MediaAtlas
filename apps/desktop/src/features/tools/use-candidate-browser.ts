import { useQuery } from "@tanstack/react-query";
import { useLayoutEffect, useRef, useState } from "react";
import { libraryApi, type MediaCard } from "../../api/library";
import {
  changeCandidateSelection,
  ordinaryCandidates,
} from "./candidate-model";
import {
  changeToolsFilters,
  type ToolsFilters,
  type ToolsSearch,
  toolsBrowseQuery,
  toolsFilterKey,
  validateToolsSearch,
} from "./search";

export function useCandidateBrowser(
  initialSearch: ToolsSearch,
  onSearch?: (search: ToolsSearch, replace?: boolean) => void,
) {
  const [localSearch, setLocalSearch] = useState(initialSearch);
  const rawSearch = validateToolsSearch(
    onSearch
      ? (initialSearch as Record<string, unknown>)
      : (localSearch as Record<string, unknown>),
  );
  const scope = toolsFilterKey(rawSearch);
  const [selection, setSelection] = useState<{
    scope: string;
    cards: MediaCard[];
    limitReached: boolean;
  }>({ scope, cards: [], limitReached: false });
  const filterChanged = selection.scope !== scope;
  const search = filterChanged ? changeToolsFilters(rawSearch, {}) : rawSearch;
  const history = useRef<{ scope: string; cursors: (string | undefined)[] }>({
    scope,
    cursors: [undefined],
  });
  function navigate(next: ToolsSearch, replace = false) {
    if (onSearch) onSearch(next, replace);
    else setLocalSearch(next);
  }
  useLayoutEffect(() => {
    if (!filterChanged) return;
    setSelection({ scope, cards: [], limitReached: false });
    history.current = { scope, cursors: [undefined] };
    if (rawSearch.cursor || rawSearch.page || rawSearch.media)
      navigate(changeToolsFilters(rawSearch, {}), true);
  });
  const query = toolsBrowseQuery(search);
  const roots = useQuery({
    queryKey: ["roots", false],
    queryFn: () => libraryApi.roots(false),
  });
  const media = useQuery({
    queryKey: ["tools-media", query],
    queryFn: () => libraryApi.toolsMedia(query),
    select: (value) => ({ ...value, items: ordinaryCandidates(value.items) }),
  });
  const selected = filterChanged ? [] : selection.cards;
  const page = search.page ?? 1;
  function filter(patch: ToolsFilters) {
    setSelection({ scope, cards: [], limitReached: false });
    history.current = { scope, cursors: [undefined] };
    navigate(changeToolsFilters(search, patch));
  }
  function select(cards: MediaCard[], checked: boolean) {
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
      validateToolsSearch({
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
      navigate({});
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
      navigate(validateToolsSearch({ ...search, media: undefined }), true),
  };
}
