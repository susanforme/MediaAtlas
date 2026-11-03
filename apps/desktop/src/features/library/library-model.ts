import type {
  MediaKind,
  RootInput,
  RootView,
  ScanTask,
} from "../../api/library";

export const ROOT_KINDS: MediaKind[] = [
  "movie",
  "tv_series",
  "anime",
  "music_release",
  "adult_video",
];
export const ALL_KINDS: MediaKind[] = [
  "movie",
  "tv_series",
  "tv_season",
  "tv_episode",
  "anime",
  "music_artist",
  "music_release_group",
  "music_release",
  "music_track",
  "music_recording",
  "music_video",
  "concert",
  "adult_video",
];
export const LIBRARY_CATEGORIES: MediaKind[] = [
  "movie",
  "tv_series",
  "anime",
  "music_release",
  "adult_video",
];
export const HEALTH = [
  "metadata_pending",
  "metadata_needs_review",
  "metadata_failed",
  "metadata_unconfigured",
  "missing_poster",
  "missing_backdrop",
  "missing_nfo",
  "incomplete_metadata",
  "duplicate",
];

/** Works on a Windows library path even when the frontend is running on POSIX. */
export function directoryName(path: string): string {
  const trimmed = path.trim().replace(/[\\/]+$/, "");
  return trimmed.split(/[\\/]/).filter(Boolean).at(-1) ?? path.trim();
}

export function newRootInput(adult: boolean): RootInput {
  return {
    id: null,
    name: "",
    path: "",
    kind: adult ? "adult_video" : "movie",
    enabled: true,
    exclude_patterns: [],
  };
}
export function rootInputFromView(root: RootView): RootInput {
  return {
    id: root.id,
    name: root.name,
    path: root.path,
    kind: root.kind,
    enabled: root.enabled,
    exclude_patterns: root.exclude_patterns,
  };
}
export function normalizeRootInput(
  form: RootInput,
  excludes: string,
): RootInput {
  return {
    ...form,
    name: form.name.trim(),
    path: form.path.trim(),
    exclude_patterns: excludes
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean),
  };
}
export function isActiveScanTask(task: ScanTask): boolean {
  return ["queued", "running", "waiting_for_user"].includes(task.state);
}
export function recentScanTasks(
  tasks: ScanTask[],
  rootId: string | undefined,
): ScanTask[] {
  return tasks
    .filter((task) => !rootId || task.root_id === rootId)
    .sort((a, b) => b.created_at.localeCompare(a.created_at));
}
