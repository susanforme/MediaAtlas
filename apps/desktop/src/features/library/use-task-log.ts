import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { libraryApi } from "../../api/library";
import { providerApi } from "../../api/providers";
import { providersQuery } from "../providers/queries";
import { scanTasksQuery } from "./queries";
import { taskLogEntries, type TaskLogEntry } from "./task-log-model";
import { useLibraryDirectories } from "./use-library-directories";

export function useTaskLog() {
  const client = useQueryClient();
  const { roots, adultAvailable } = useLibraryDirectories();
  const ordinary = useQuery({
    ...scanTasksQuery(),
    refetchInterval: 2000,
  });
  const privateTasks = useQuery({
    ...scanTasksQuery(true),
    enabled: adultAvailable,
    refetchInterval: adultAvailable ? 2000 : false,
  });
  const providers = useQuery({
    ...providersQuery(),
    refetchInterval: 2000,
  });
  const cancel = useMutation({
    mutationFn: async (entry: TaskLogEntry) =>
      entry.source === "provider"
        ? providerApi.cancel(entry.task.id)
        : libraryApi.cancel(entry.task.id),
    onSuccess: (_result, entry) => {
      void client.invalidateQueries({
        queryKey:
          entry.source === "provider"
            ? ["providers"]
            : entry.adult
              ? ["adult", "scan-tasks"]
              : ["tasks"],
      });
    },
  });
  const queries = [
    { name: "taskLogLibrary", query: ordinary },
    ...(adultAvailable ? [{ name: "kind_adult_video", query: privateTasks }] : []),
    { name: "providers", query: providers },
  ];
  return {
    roots,
    providers: providers.data?.providers ?? [],
    entries: taskLogEntries(
      ordinary.data ?? [],
      adultAvailable ? (privateTasks.data ?? []) : [],
      providers.data?.tasks ?? [],
    ),
    loading: queries.some(({ query }) => query.isPending),
    errors: queries.filter(({ query }) => query.isError),
    cancel,
  };
}
