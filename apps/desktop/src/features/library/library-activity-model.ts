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
      (task.review_count ?? 0) > 0 ||
      (task.metadata_failed_count ?? 0) > 0)
  );
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
        task.metadata_failed_count,
      ].join(":"),
    )
    .sort()
    .join("|");
}
