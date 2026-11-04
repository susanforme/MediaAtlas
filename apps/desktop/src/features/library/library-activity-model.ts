import type { ScanTask } from "../../api/library";

/** A newer refresh replaces the visible outcome of an older run for that folder. */
export function latestLibraryRefreshes(tasks: ScanTask[], rootId?: string) {
  const latest = new Map<string, ScanTask>();
  for (const task of [...tasks].sort((a, b) =>
    b.created_at.localeCompare(a.created_at),
  )) {
    if (
      task.mode !== "automatic" ||
      (rootId && task.root_id !== rootId) ||
      latest.has(task.root_id)
    )
      continue;
    latest.set(task.root_id, task);
  }
  return [...latest.values()];
}

export function refreshIsRunning(task: ScanTask) {
  return task.state === "queued" || task.state === "running";
}

export function refreshNeedsAttention(task: ScanTask) {
  return (
    !refreshIsRunning(task) &&
    (task.state === "failed" ||
      task.state === "interrupted" ||
      task.state === "cancelled" ||
      task.state === "waiting_for_user" ||
      (task.review_count ?? 0) > 0 ||
      (task.unconfigured_count ?? 0) > 0 ||
      (task.metadata_failed_count ?? 0) > 0)
  );
}

/** Source setup is an actionable prerequisite, not a failed movie match. */
export function refreshCounts(task: ScanTask) {
  const unconfigured = task.unconfigured_count ?? 0;
  return {
    unconfigured,
    review: Math.max(0, (task.review_count ?? 0) - unconfigured),
    failed: task.metadata_failed_count ?? 0,
  };
}

export function refreshResultTitle(task: ScanTask) {
  if (refreshIsRunning(task)) return "autoRefreshRunning";
  if (["failed", "cancelled", "interrupted"].includes(task.state))
    return `state_${task.state}`;
  if (refreshCounts(task).unconfigured > 0) return "libraryRefreshUnconfigured";
  if (refreshNeedsAttention(task)) return "libraryRefreshIncomplete";
  return (task.matched_count ?? 0) > 0
    ? "libraryRefreshCompleted"
    : "libraryRefreshLocalComplete";
}

export function refreshSnapshot(tasks: ScanTask[]) {
  return tasks
    .filter((task) => task.mode === "automatic")
    .map((task) =>
      [
        task.id,
        task.state,
        task.phase,
        task.updated_at,
        task.imported_count,
        task.matched_count,
        task.review_count,
        task.unconfigured_count,
        task.metadata_failed_count,
      ].join(":"),
    )
    .sort()
    .join("|");
}
