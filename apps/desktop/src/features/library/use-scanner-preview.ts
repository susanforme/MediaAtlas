import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  type Dispatch,
  type SetStateAction,
  useEffect,
  useRef,
  useState,
} from "react";
import { adultApi } from "../../api/adult";
import { libraryApi, type ScanTask } from "../../api/library";
import { usePrivacy } from "./context";
import { isActiveScanTask, recentScanTasks } from "./library-model";
import type { ScannerSearch } from "./scanner-navigation";

type ReviewState = {
  taskId: string;
  selected: string[];
  cursors: (string | null)[];
  page: number;
  confirmOpen: boolean;
};
const emptyReview = (taskId: string): ReviewState => ({
  taskId,
  selected: [],
  cursors: [null],
  page: 0,
  confirmOpen: false,
});

export function useScannerPreview(
  search: ScannerSearch,
  onSearch: (value: ScannerSearch) => void,
  adult: boolean,
) {
  const { includeAdult: unlockedAdult } = usePrivacy();
  const includeAdult = adult && unlockedAdult;
  const client = useQueryClient();
  const active = useRef(true);
  const startLock = useRef(false);
  const confirmLock = useRef(false);
  const autoNavigation = useRef("");
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
    enabled: !adult || unlockedAdult,
  });
  const tasksKey = adult ? ["adult", "scan-tasks"] : ["tasks", false];
  const tasks = useQuery({
    queryKey: tasksKey,
    queryFn: () => (adult ? adultApi.tasks() : libraryApi.tasks(false)),
    enabled: !adult || unlockedAdult,
    refetchInterval: 2000,
  });
  const availableRoots =
    roots.data?.filter(
      (root) => root.enabled && (adult || root.kind !== "adult_video"),
    ) ?? [];
  const explicitTask = tasks.data?.find((entry) => entry.id === search.task);
  const rootId =
    explicitTask?.root_id ??
    search.root ??
    (availableRoots.length === 1 ? availableRoots[0].id : undefined);
  const selectedRoot = availableRoots.find((root) => root.id === rootId);
  const matchingTasks = recentScanTasks(tasks.data ?? [], rootId);
  const task =
    explicitTask ?? (!search.task && rootId ? matchingTasks[0] : undefined);
  const taskId = task?.id ?? search.task ?? "";
  const currentContext = useRef({ rootId, taskId });
  currentContext.current = { rootId, taskId };
  useEffect(() => {
    if (!rootId || (search.root === rootId && (search.task || !task))) return;
    const next = { root: rootId, task: search.task ?? task?.id };
    const key = JSON.stringify(next);
    if (key === autoNavigation.current) return;
    autoNavigation.current = key;
    onSearch(next);
  }, [rootId, search.root, search.task, task, onSearch]);
  const [review, setReview] = useState(() => emptyReview(taskId));
  const currentReview = review.taskId === taskId ? review : emptyReview(taskId);
  function updateReview(update: (value: ReviewState) => ReviewState) {
    setReview((old) =>
      update(old.taskId === taskId ? old : emptyReview(taskId)),
    );
  }
  const setSelected: Dispatch<SetStateAction<string[]>> = (value) =>
    updateReview((old) => ({
      ...old,
      selected: typeof value === "function" ? value(old.selected) : value,
    }));
  const setCursors: Dispatch<SetStateAction<(string | null)[]>> = (value) =>
    updateReview((old) => ({
      ...old,
      cursors: typeof value === "function" ? value(old.cursors) : value,
    }));
  const setPage: Dispatch<SetStateAction<number>> = (value) =>
    updateReview((old) => ({
      ...old,
      page: typeof value === "function" ? value(old.page) : value,
    }));
  const setConfirmOpen: Dispatch<SetStateAction<boolean>> = (value) =>
    updateReview((old) => ({
      ...old,
      confirmOpen: typeof value === "function" ? value(old.confirmOpen) : value,
    }));
  const preview = useQuery({
    queryKey: adult
      ? [
          "adult",
          "scan-preview",
          taskId,
          currentReview.cursors[currentReview.page],
        ]
      : ["preview", taskId, currentReview.cursors[currentReview.page], false],
    queryFn: () =>
      libraryApi.preview(
        taskId,
        currentReview.cursors[currentReview.page] ?? null,
        includeAdult,
      ),
    enabled: Boolean(task) && (!adult || unlockedAdult),
    refetchInterval:
      task?.state === "running" || task?.state === "queued" ? 2000 : false,
  });
  function selectTask(id: string) {
    const selectedTask = tasks.data?.find((entry) => entry.id === id);
    setReview(emptyReview(id));
    onSearch({ root: selectedTask?.root_id ?? rootId, task: id || undefined });
  }
  function selectRoot(id: string) {
    setReview(emptyReview(""));
    onSearch({ root: id || undefined });
  }
  const startMutation = useMutation({
    mutationFn: (id: string) => libraryApi.start(id),
    onSuccess: (result, startedRoot) => {
      client.setQueryData<ScanTask[]>(tasksKey, (old) => [
        result,
        ...(old ?? []).filter((entry) => entry.id !== result.id),
      ]);
      void client.invalidateQueries({ queryKey: tasksKey });
      if (!active.current || currentContext.current.rootId !== startedRoot)
        return;
      setReview(emptyReview(result.id));
      onSearch({ root: result.root_id, task: result.id });
    },
    onSettled: () => {
      startLock.current = false;
    },
  });
  const start = {
    ...startMutation,
    isError: startMutation.isError && startMutation.variables === rootId,
    error: startMutation.variables === rootId ? startMutation.error : null,
    mutate: (id: string) => {
      if (
        startLock.current ||
        confirmLock.current ||
        (adult && !unlockedAdult) ||
        !availableRoots.some((root) => root.id === id) ||
        matchingTasks.some(isActiveScanTask)
      )
        return;
      startLock.current = true;
      startMutation.mutate(id);
    },
  };
  const confirmMutation = useMutation({
    mutationFn: (input: { taskId: string; ids: string[] }) =>
      libraryApi.confirm(input.taskId, input.ids),
    onSuccess: (_result, input) => {
      void client.invalidateQueries();
      if (!active.current || currentContext.current.taskId !== input.taskId)
        return;
      setReview(emptyReview(input.taskId));
    },
    onSettled: () => {
      confirmLock.current = false;
    },
  });
  const confirm = {
    ...confirmMutation,
    isError:
      confirmMutation.isError && confirmMutation.variables?.taskId === taskId,
    error:
      confirmMutation.variables?.taskId === taskId
        ? confirmMutation.error
        : null,
    isSuccess:
      confirmMutation.isSuccess && confirmMutation.variables?.taskId === taskId,
    mutate: () => {
      if (
        confirmLock.current ||
        !task ||
        (adult && !unlockedAdult) ||
        task.state !== "waiting_for_user" ||
        !currentReview.selected.length ||
        !currentReview.confirmOpen
      )
        return;
      confirmLock.current = true;
      confirmMutation.mutate({ taskId, ids: currentReview.selected });
    },
  };
  const cancel = useMutation({
    mutationFn: (id: string) => libraryApi.cancel(id),
    onSuccess: (_result, id) => {
      void client.invalidateQueries({ queryKey: tasksKey });
      if (active.current && currentContext.current.taskId === id)
        setReview(emptyReview(id));
    },
  });
  const pendingIds =
    preview.data?.items
      .filter((candidate) => candidate.status === "pending")
      .map((candidate) => candidate.id) ?? [];
  return {
    roots,
    tasks,
    task,
    preview,
    start,
    confirm,
    cancel,
    availableRoots,
    selectedRoot,
    rootId,
    matchingTasks,
    selectRoot,
    canStart: Boolean(selectedRoot) && !matchingTasks.some(isActiveScanTask),
    canImport: task?.state === "waiting_for_user" && (!adult || unlockedAdult),
    pendingIds,
    selected: currentReview.selected,
    setSelected,
    confirmOpen: currentReview.confirmOpen,
    setConfirmOpen,
    page: currentReview.page,
    setPage,
    setCursors,
    selectTask,
  };
}
