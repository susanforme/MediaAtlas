import { useQuery } from "@tanstack/react-query";
import { adultApi } from "../../api/adult";
import { libraryApi, type RootView, type ScanTask } from "../../api/library";

export function useRootActivity(roots: RootView[], adultAvailable: boolean) {
  const ordinary = useQuery({
    queryKey: ["tasks", false],
    queryFn: () => libraryApi.tasks(false),
    enabled: roots.some((root) => root.kind !== "adult_video"),
    refetchInterval: 2000,
  });
  const privateTasks = useQuery({
    queryKey: ["adult", "scan-tasks"],
    queryFn: adultApi.tasks,
    enabled:
      adultAvailable && roots.some((root) => root.kind === "adult_video"),
    refetchInterval: adultAvailable ? 2000 : false,
  });
  const latest = new Map<string, ScanTask>();
  for (const task of [
    ...(ordinary.data ?? []),
    ...(adultAvailable ? (privateTasks.data ?? []) : []),
  ]) {
    const previous = latest.get(task.root_id);
    if (!previous || task.updated_at > previous.updated_at)
      latest.set(task.root_id, task);
  }
  return latest;
}
