import { type LibrarySearch, validateLibrarySearch } from "../library/browse-model";
import {
  type ScraperSearch,
  validateScraperReturnSearch,
} from "../scraper/search";

export const SETUP_KINDS = [
  "movie",
  "tv_series",
  "anime",
  "music_release",
  "adult_video",
] as const;
export type SetupKind = (typeof SETUP_KINDS)[number];
export const SETUP_PROVIDERS = [
  "tmdb",
  "omdb",
  "douban",
  "bangumi",
  "anilist",
  "tvmaze",
  "wikipedia.html",
  "dmm",
  "wikidata",
  "musicbrainz",
  "coverartarchive",
] as const;
export type SetupProvider = (typeof SETUP_PROVIDERS)[number];
export const SETTINGS_TABS = [
  "general",
  "auth",
  "providers",
  "network",
  "diagnostics",
] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];
export type SetupReturn =
  | { to: "library"; search: LibrarySearch }
  | { to: "roots" }
  | { to: "media"; media: string };
export interface ProviderSetupSearch extends ScraperSearch {
  from?: "scraper" | "library";
  root?: string;
  kind?: SetupKind;
  provider?: SetupProvider;
  tab?: SettingsTab;
  return?: SetupReturn;
}
function id(value: unknown): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= 128 &&
    value.trim() === value &&
    [...value].every(
      (character) =>
        character.charCodeAt(0) > 32 && character.charCodeAt(0) !== 127,
    ) &&
    !/[/\\]/.test(value)
  );
}
export function validateProviderSetupSearch(
  value: Record<string, unknown>,
): ProviderSetupSearch {
  const provider = SETUP_PROVIDERS.includes(value.provider as SetupProvider)
    ? (value.provider as SetupProvider)
    : undefined;
  const from =
    value.from === "library"
      ? {
          from: "library" as const,
          ...(id(value.root) ? { root: value.root } : {}),
          ...(SETUP_KINDS.includes(value.kind as SetupKind)
            ? { kind: value.kind as SetupKind }
            : {}),
        }
      : validateScraperReturnSearch(value);
  const tab = SETTINGS_TABS.includes(value.tab as SettingsTab)
    ? (value.tab as SettingsTab)
    : undefined;
  const destination = value.return;
  let origin: SetupReturn | undefined;
  if (
    value.from === "library" &&
    destination &&
    typeof destination === "object" &&
    !Array.isArray(destination)
  ) {
    const candidate = destination as Record<string, unknown>;
    if (candidate.to === "roots") origin = { to: "roots" };
    if (candidate.to === "media" && id(candidate.media))
      origin = { to: "media", media: candidate.media };
    if (
      candidate.to === "library" &&
      candidate.search &&
      typeof candidate.search === "object" &&
      !Array.isArray(candidate.search)
    ) {
      const search = validateLibrarySearch(
        candidate.search as Record<string, unknown>,
      );
      if ((search.kind === "adult_video") === (value.kind === "adult_video"))
        origin = { to: "library", search };
    }
  }
  return {
    ...from,
    ...(provider ? { provider } : {}),
    ...(tab ? { tab } : {}),
    ...(origin ? { return: origin } : {}),
  };
}

/** Only fixed app routes can resume a source setup workflow. */
export function setupReturnLocation(setup: ProviderSetupSearch) {
  if (setup.from === "scraper")
    return {
      to: "/scraper" as const,
      search: { media: setup.media, queue: setup.queue, session: setup.session },
    };
  if (setup.return?.to === "roots")
    return { to: "/roots" as const, search: { kind: setup.kind } };
  if (setup.return?.to === "media")
    return {
      to: "/media/$mediaId" as const,
      params: { mediaId: setup.return.media },
      search:
        setup.kind === "adult_video" ? { kind: "adult_video" as const } : {},
    };
  return {
    to: "/library" as const,
    search:
      setup.return?.to === "library"
        ? setup.return.search
        : { root: setup.root, kind: setup.kind },
  };
}
export function suggestedProvider(kind?: string | null): SetupProvider {
  if (kind === "anime") return "anilist";
  if (kind === "adult_video") return "dmm";
  if (kind?.startsWith("music_")) return "musicbrainz";
  return "tmdb";
}
export type SetupComplete = (
  rootId?: string,
  taskId?: string,
  kind?: SetupKind,
) => void;
