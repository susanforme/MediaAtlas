import type { MediaDetail, MediaKind } from "../../api/library";
import {
  type ProviderSetupSearch,
  type SetupKind,
  suggestedProvider,
  validateProviderSetupSearch,
} from "../settings/setup-search";

function setupKind(kind: MediaKind): SetupKind | undefined {
  if (kind === "tv_episode" || kind === "tv_season") return "tv_series";
  if (
    kind === "movie" ||
    kind === "tv_series" ||
    kind === "anime" ||
    kind === "adult_video"
  )
    return kind;
  return undefined;
}

export type DetailRecovery =
  | { action: "pending" }
  | { action: "configure"; search: ProviderSetupSearch }
  | { action: "identify" | "correct" }
  | { action: "none" };

/** Recovery uses actual local files, never a guessed folder or provider query. */
export function detailRecovery(
  detail: Pick<MediaDetail, "media" | "files" | "health">,
): DetailRecovery {
  if (detail.health.includes("metadata_pending")) return { action: "pending" };
  if (detail.health.includes("metadata_unconfigured")) {
    const kind = setupKind(detail.media.kind);
    if (!kind) return { action: "none" };
    const roots = [...new Set(detail.files.map((file) => file.root_id))];
    return {
      action: "configure",
      search: validateProviderSetupSearch({
        from: "library",
        root: roots.length === 1 ? roots[0] : undefined,
        kind,
        provider: suggestedProvider(kind),
        tab: "auth",
      }),
    };
  }
  if (
    !["movie", "tv_series", "anime", "adult_video"].includes(detail.media.kind)
  )
    return { action: "none" };
  return {
    action: detail.health.includes("incomplete_metadata")
      ? "identify"
      : "correct",
  };
}
