import type { MediaKind } from "../../api/library";
import type { AuthStatus } from "../../api/provider-auth";
import type { ProviderView } from "../../api/providers";

export type RootProviderId = "tmdb" | "anilist" | "tvmaze" | "dmm" | "wikidata";
const rootProviderIds: RootProviderId[] = ["tmdb", "anilist", "tvmaze", "dmm", "wikidata"];

export function directorySourceChoices(providers: ProviderView[], kind: MediaKind | null) {
  return providers.filter(({ descriptor }) =>
    !descriptor.fixture &&
    rootProviderIds.includes(descriptor.id as RootProviderId) &&
    (kind === "adult_video"
      ? descriptor.media_kinds.includes("adult_video")
      : !descriptor.media_kinds.includes("adult_video") &&
        (kind === null
          ? descriptor.media_kinds.some((value) => ["movie", "tv_series", "anime"].includes(value))
          : descriptor.media_kinds.includes(kind))),
  );
}

export function moveRootSource(sources: string[], index: number, direction: -1 | 1): string[] {
  const target = index + direction;
  if (index < 0 || index >= sources.length || target < 0 || target >= sources.length) return sources;
  const ordered = [...sources];
  [ordered[index], ordered[target]] = [ordered[target], ordered[index]];
  return ordered;
}
export type RootSourceState =
  | "checking"
  | "configured"
  | "disabled"
  | "credentials"
  | "unavailable"
  | "error";

/** Mirrors the backend's automatic source selection, not every installed scraper. */
export function automaticRootSources(kind: MediaKind | null): RootProviderId[] {
  if (kind === "movie" || kind === "tv_series") return ["tmdb"];
  if (kind === "anime") return ["anilist"];
  if (kind === "adult_video") return ["dmm", "wikidata"];
  if (kind === null) return ["tmdb", "anilist"];
  return [];
}

export function rootSourceState(
  provider: ProviderView | undefined,
  auth: AuthStatus | undefined,
  loading: boolean,
  failed: boolean,
): RootSourceState {
  if (failed) return "error";
  if (loading) return "checking";
  if (!provider || provider.descriptor.fixture) return "unavailable";
  if (!provider.config.enabled) return "disabled";
  // This is configuration readiness; it does not assert a successful live request.
  if (
    (provider.descriptor.requires_api_key ||
      provider.descriptor.requires_login) &&
    !auth?.configured
  )
    return "credentials";
  return "configured";
}
