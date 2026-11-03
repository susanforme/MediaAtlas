import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { libraryApi } from "../../api/library";
import {
  latestLibraryRefreshes,
  refreshIsRunning,
  refreshNeedsAttention,
  refreshSnapshot,
} from "./library-activity-model";

/** Ordinary library views observe the backend worker; mounting never starts work. */
export function useLibraryActivity(rootId?: string, enabled = true) {
  const client = useQueryClient();
  const previousSnapshot = useRef<string | undefined>(undefined);
  const tasks = useQuery({
    queryKey: ["tasks", false],
    queryFn: () => libraryApi.tasks(false),
    enabled,
    refetchInterval: (query) =>
      query.state.data?.some(
        (task) => task.mode === "automatic" && refreshIsRunning(task),
      )
        ? 2000
        : false,
  });
  const snapshot = tasks.data ? refreshSnapshot(tasks.data) : undefined;
  useEffect(() => {
    if (!enabled || snapshot === undefined) return;
    const previous = previousSnapshot.current;
    previousSnapshot.current = snapshot;
    if (previous === undefined || previous === snapshot) return;
    // Include the terminal transition: the last items must appear when polling stops.
    for (const queryKey of [
      ["media"],
      ["detail"],
      ["video-source-detail"],
      ["summary", false],
      ["facets"],
    ])
      void client.invalidateQueries({ queryKey });
  }, [client, enabled, snapshot]);
  const latest = latestLibraryRefreshes(tasks.data ?? [], rootId);
  return {
    tasks,
    active: latest.filter(refreshIsRunning),
    attention: latest.filter(refreshNeedsAttention),
  };
}
