import type { ScanTask } from "../../api/library";
import type { ProviderTask } from "../../api/providers";
import { refreshCounts } from "./library-activity-model";

export type TaskLogEntry =
  | { source: "scan"; task: ScanTask; adult: boolean }
  | { source: "provider"; task: ProviderTask };

// shortcut: tasks expose only their latest state; add event history when the backend persists it.
export function taskLogEntries(
  ordinary: ScanTask[],
  adult: ScanTask[],
  providers: ProviderTask[],
): TaskLogEntry[] {
  const entries: TaskLogEntry[] = [
    ...ordinary.map((task) => ({ source: "scan" as const, task, adult: false })),
    ...adult.map((task) => ({ source: "scan" as const, task, adult: true })),
    ...providers.map((task) => ({ source: "provider" as const, task })),
  ];
  return entries.sort(
    (a, b) => Date.parse(b.task.updated_at) - Date.parse(a.task.updated_at),
  );
}

export function scanLogCounts(task: ScanTask) {
  const counts = refreshCounts(task);
  const values: [string, number][] = [
    ["taskLogChecked", task.visited_files],
    ["libraryRefreshAdded", task.imported_count],
  ];
  if (task.mode === "automatic") {
    values.push(
      ["libraryRefreshMatched", task.matched_count ?? 0],
      ["libraryRefreshReview", counts.review],
      ["libraryRefreshUnconfiguredCount", counts.unconfigured],
      ["libraryRefreshFailed", counts.failed],
    );
  } else {
    values.push(["taskLogCandidates", task.candidate_count]);
  }
  values.push(
    ["taskLogSkipped", task.skipped_files],
    ["taskLogWarnings", task.warnings],
  );
  return values.filter(
    ([key, count]) =>
      key === "taskLogChecked" ||
      key === "libraryRefreshAdded" ||
      key === "libraryRefreshMatched" ||
      count > 0,
  );
}
