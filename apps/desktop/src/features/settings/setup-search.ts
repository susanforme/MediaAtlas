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
  "anilist",
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
export interface ProviderSetupSearch extends ScraperSearch {
  from?: "scraper" | "library";
  root?: string;
  kind?: SetupKind;
  provider?: SetupProvider;
  tab?: SettingsTab;
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
  return {
    ...from,
    ...(provider ? { provider } : {}),
    ...(tab ? { tab } : {}),
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
