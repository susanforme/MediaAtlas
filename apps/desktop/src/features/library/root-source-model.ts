import type { MediaKind } from "../../api/library";
import type { AuthStatus } from "../../api/provider-auth";
import type { ProviderView } from "../../api/providers";

export type RootProviderId = "tmdb" | "anilist" | "dmm" | "wikidata";
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
