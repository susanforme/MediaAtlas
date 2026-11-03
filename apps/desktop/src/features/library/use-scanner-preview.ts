import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import { usePrivacy } from "./context";
import type { ScannerSearch } from "./scanner-navigation";

export function useScannerPreview(
  search: ScannerSearch,
  onSearch: (value: ScannerSearch) => void,
  adult: boolean,
) {
  const { includeAdult: unlockedAdult } = usePrivacy();
  const includeAdult = adult && unlockedAdult;
  const client = useQueryClient();
  const [selected, setSelected] = useState<string[]>([]);
  const [cursors, setCursors] = useState<(string | null)[]>([null]);
  const [page, setPage] = useState(0);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
    enabled: !adult || unlockedAdult,
  });
  const tasks = useQuery({
    queryKey: adult ? ["adult", "scan-tasks"] : ["tasks", false],
    queryFn: () => (adult ? adultApi.tasks() : libraryApi.tasks(false)),
    enabled: !adult || unlockedAdult,
    refetchInterval: 2000,
  });
  const task = tasks.data?.find((entry) => entry.id === search.task);
  const preview = useQuery({
    queryKey: adult
      ? ["adult", "scan-preview", search.task, cursors[page]]
      : ["preview", search.task, cursors[page], false],
    queryFn: () =>
      libraryApi.preview(
        search.task ?? "",
        cursors[page] ?? null,
        includeAdult,
      ),
    enabled: Boolean(task),
    refetchInterval:
      task?.state === "running" || task?.state === "queued" ? 2000 : false,
  });
  function selectTask(id: string) {
    setSelected([]);
    setCursors([null]);
    setPage(0);
    setConfirmOpen(false);
    onSearch({ ...search, task: id || undefined });
  }
  const start = useMutation({
    mutationFn: (id: string) => libraryApi.start(id),
    onSuccess: (result) => {
      selectTask(result.id);
      void client.invalidateQueries({
        queryKey: adult ? ["adult", "scan-tasks"] : ["tasks"],
      });
    },
  });
  const confirm = useMutation({
    mutationFn: () => libraryApi.confirm(search.task ?? "", selected),
    onSuccess: () => {
      setSelected([]);
      setConfirmOpen(false);
      void client.invalidateQueries();
    },
  });
  const cancel = useMutation({
    mutationFn: (id: string) => libraryApi.cancel(id),
    onSuccess: () => {
      setSelected([]);
      setConfirmOpen(false);
      void client.invalidateQueries();
    },
  });
  const canImport = task?.state === "waiting_for_user";
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
    canImport,
    pendingIds,
    selected,
    setSelected,
    confirmOpen,
    setConfirmOpen,
    page,
    setPage,
    setCursors,
    selectTask,
  };
}
