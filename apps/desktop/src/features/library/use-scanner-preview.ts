import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef } from "react";
import { adultApi } from "../../api/adult";
import { libraryApi, type ScanTask } from "../../api/library";
import { recentScanTasks } from "./library-model";
import type { ScannerSearch } from "./scanner-navigation";
import { useLibraryDirectories } from "./use-library-directories";

/** A unified directory workflow with separate authorized query/cache lanes. */
export function useScannerPreview(
  search: ScannerSearch,
  onSearch: (value: ScannerSearch) => void,
  _adult: boolean,
) {
  const { roots, adultAvailable } = useLibraryDirectories();
  const client = useQueryClient();
  const active = useRef(true);
  const startLock = useRef(false);
  const autoNavigation = useRef("");
  const privateAccess = useRef(adultAvailable);
  privateAccess.current = adultAvailable;
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const ordinaryTasks = useQuery({
    queryKey: ["tasks", false],
    queryFn: () => libraryApi.tasks(false),
    refetchInterval: 2000,
  });
  const privateTasks = useQuery({
    queryKey: ["adult", "scan-tasks"],
    queryFn: adultApi.tasks,
    enabled: adultAvailable,
    refetchInterval: adultAvailable ? 2000 : false,
  });
  const tasks = {
    ...ordinaryTasks,
    data: [
      ...(ordinaryTasks.data ?? []),
      ...(adultAvailable ? (privateTasks.data ?? []) : []),
    ],
    isPending:
      ordinaryTasks.isPending || (adultAvailable && privateTasks.isPending),
    isError: ordinaryTasks.isError || (adultAvailable && privateTasks.isError),
    error: ordinaryTasks.error ?? (adultAvailable ? privateTasks.error : null),
  };
  const availableRoots = roots.data.filter((root) => root.enabled);
  const explicitTask = tasks.data.find((entry) => entry.id === search.task);
  const rootId =
    explicitTask?.root_id ??
    search.root ??
    (availableRoots.length === 1 ? availableRoots[0].id : undefined);
  const selectedRoot = availableRoots.find((root) => root.id === rootId);
  const matchingTasks = recentScanTasks(tasks.data, rootId);
  const task =
    explicitTask ?? (!search.task && rootId ? matchingTasks[0] : undefined);
  const selectedAdult =
    selectedRoot?.kind === "adult_video" ||
    Boolean(
      adultAvailable &&
        privateTasks.data?.some((entry) => entry.id === task?.id),
    );
  const context = useRef({ rootId });
  context.current = { rootId };
  const running = matchingTasks.some(
    (entry) => entry.state === "queued" || entry.state === "running",
  );
  function routeFor(id: string | undefined, taskId?: string): ScannerSearch {
    const isAdult =
      roots.data.some(
        (root) => root.id === id && root.kind === "adult_video",
      ) ||
      Boolean(
        adultAvailable &&
          privateTasks.data?.some((entry) => entry.root_id === id),
      );
    return {
      root: id,
      ...(taskId ? { task: taskId } : {}),
      ...(isAdult ? { kind: "adult_video" as const } : {}),
    };
  }
  useEffect(() => {
    if (!rootId || (search.root === rootId && (search.task || !task))) return;
    const next = {
      root: rootId,
      task: search.task ?? task?.id,
      ...(selectedAdult ? { kind: "adult_video" as const } : {}),
    };
    const key = JSON.stringify(next);
    if (key === autoNavigation.current) return;
    autoNavigation.current = key;
    onSearch(next);
  }, [rootId, search.root, search.task, task, onSearch, selectedAdult]);
  function selectTask(id: string) {
    const selected = tasks.data.find((entry) => entry.id === id);
    if (!selected) return;
    onSearch(routeFor(selected.root_id, id));
  }
  function selectRoot(id: string) {
    if (id && !availableRoots.some((root) => root.id === id)) return;
    onSearch(routeFor(id || undefined));
  }
  const startMutation = useMutation({
    mutationFn: (input: { id: string; adult: boolean }) =>
      libraryApi.sync(input.id),
    onSuccess: (result, input) => {
      if (input.adult && !privateAccess.current) return;
      const key = input.adult ? ["adult", "scan-tasks"] : ["tasks", false];
      client.setQueryData<ScanTask[]>(key, (old) => [
        result,
        ...(old ?? []).filter((entry) => entry.id !== result.id),
      ]);
      void client.invalidateQueries({ queryKey: key });
      if (!active.current || context.current.rootId !== input.id) return;
      onSearch({
        root: result.root_id,
        task: result.id,
        ...(input.adult ? { kind: "adult_video" as const } : {}),
      });
    },
    onSettled: () => {
      startLock.current = false;
    },
  });
  const start = {
    ...startMutation,
    isError: startMutation.isError && startMutation.variables?.id === rootId,
    error: startMutation.variables?.id === rootId ? startMutation.error : null,
    mutate: (id: string) => {
      const root = availableRoots.find((entry) => entry.id === id);
      if (
        startLock.current ||
        !root ||
        (root.kind === "adult_video" && !privateAccess.current) ||
        running
      )
        return;
      startLock.current = true;
      startMutation.mutate({ id, adult: root.kind === "adult_video" });
    },
  };
  const cancelMutation = useMutation({
    mutationFn: (id: string) => libraryApi.cancel(id),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["tasks"] });
      if (privateAccess.current)
        void client.invalidateQueries({ queryKey: ["adult", "scan-tasks"] });
    },
  });
  const cancel = {
    ...cancelMutation,
    mutate: (id: string) => {
      if (
        cancelMutation.isPending ||
        (privateTasks.data?.some((entry) => entry.id === id) &&
          !privateAccess.current) ||
        !matchingTasks.some((entry) => entry.id === id)
      )
        return;
      cancelMutation.mutate(id);
    },
  };
  return {
    roots,
    tasks,
    task,
    start,
    cancel,
    availableRoots,
    selectedRoot,
    selectedAdult,
    rootId,
    matchingTasks,
    selectRoot,
    selectTask,
    canStart: Boolean(selectedRoot) && !running,
  };
}
