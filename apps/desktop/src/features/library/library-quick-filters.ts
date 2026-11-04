/** Existing server health predicates, never counts or filters derived from the loaded page. */
const QUICK_FILTERS = [
  { health: undefined, label: "libraryQuickAll" },
  { health: "incomplete_metadata", label: "libraryQuickIncomplete" },
  { health: "metadata_needs_review", label: "libraryQuickReview" },
  { health: "metadata_unconfigured", label: "libraryQuickUnconfigured" },
  { health: "metadata_failed", label: "libraryQuickFailed" },
  { health: "missing_poster", label: "libraryQuickMissingPoster" },
] as const;

export function libraryQuickFilters(adult: boolean) {
  return QUICK_FILTERS.filter(
    (filter) => !adult || !filter.health?.startsWith("metadata_"),
  );
}
