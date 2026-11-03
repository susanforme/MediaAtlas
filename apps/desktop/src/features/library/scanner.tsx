import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import { libraryApi } from "../../api/library";
import { Button } from "../../components/ui/button";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { ProviderTasks } from "../providers/pages";
import type { ScannerSearch } from "./scanner-navigation";
import { useLibraryDirectories } from "./use-library-directories";
import "./translations";
import { AutomaticScanResult } from "./automatic-scan-result";
import { isActiveScanTask } from "./library-model";
import { useScannerPreview } from "./use-scanner-preview";

export {
  type ScannerSearch,
  validateScannerSearch,
} from "./scanner-navigation";

import { Empty, ErrorNotice, Loading, PageHeading, useFormat } from "./shared";

export function TasksPage() {
  const { t } = useTranslation();
  const { number, date } = useFormat();
  const client = useQueryClient();
  const { roots, adultAvailable } = useLibraryDirectories();
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
    refetch: () =>
      Promise.all([
        ordinaryTasks.refetch(),
        ...(adultAvailable ? [privateTasks.refetch()] : []),
      ]),
  };
  const cancel = useMutation({
    mutationFn: (id: string) => libraryApi.cancel(id),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["tasks"] });
      if (adultAvailable)
        void client.invalidateQueries({ queryKey: ["adult", "scan-tasks"] });
    },
  });
  return (
    <>
      <PageHeading title={t("tasks")} />
      <h2 className="section-title">{t("scanTasks")}</h2>
      {cancel.isError && <ErrorNotice error={cancel.error} />}
      {tasks.isPending ? (
        <Loading />
      ) : tasks.isError ? (
        <ErrorNotice error={tasks.error} retry={() => void tasks.refetch()} />
      ) : !tasks.data.length ? (
        <Empty title={t("noTasks")} />
      ) : (
        <div className="space-y-4">
          {tasks.data.map((task) => (
            <article className="panel" key={task.id}>
              <div className="flex flex-wrap justify-between gap-4">
                <div>
                  <p className="badge">{t(`state_${task.state}`)}</p>
                  <p className="mt-2 break-all text-xs text-muted-foreground">
                    {roots.data?.find((root) => root.id === task.root_id)
                      ?.name ?? t("root")}
                  </p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {date(task.updated_at)}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button asChild variant="ghost">
                    <Link
                      to="/scanner"
                      search={{
                        root: task.root_id,
                        task: task.id,
                        ...(roots.data.some(
                          (root) =>
                            root.id === task.root_id &&
                            root.kind === "adult_video",
                        )
                          ? { kind: "adult_video" as const }
                          : {}),
                      }}
                      className="action-link"
                    >
                      {t(
                        task.mode === "automatic"
                          ? "autoScanDetails"
                          : "preview",
                      )}
                    </Link>
                  </Button>
                  {(task.mode === "automatic"
                    ? task.state === "queued" || task.state === "running"
                    : isActiveScanTask(task)) && (
                    <Button
                      variant="outline"
                      disabled={cancel.isPending}
                      onClick={() => cancel.mutate(task.id)}
                    >
                      {t("cancelTask")}
                    </Button>
                  )}
                </div>
              </div>
              <div className="mt-4 flex flex-wrap gap-5 text-sm">
                {(
                  [
                    ["visited", task.visited_files],
                    ["skipped", task.skipped_files],
                    ["candidates", task.candidate_count],
                    ["imported", task.imported_count],
                    ["warnings", task.warnings],
                  ] as const
                ).map(([label, value]) => (
                  <span key={label}>
                    {t(label)}: {number(value)}
                  </span>
                ))}
              </div>
              {task.error_code && (
                <p className="mt-3 text-sm" role="alert">
                  {t("scanError")}
                </p>
              )}
            </article>
          ))}
        </div>
      )}
      <ProviderTasks />
    </>
  );
}

export function ScannerPage({
  search,
  onSearch,
  adult = false,
  onAddRoot,
}: {
  adult?: boolean;
  onAddRoot?: () => void;
  onBrowse?: () => void;
  onTypeChange?: (adult: boolean) => void;
  search: ScannerSearch;
  onSearch: (value: ScannerSearch) => void;
}) {
  const { t } = useTranslation();
  const { number, date } = useFormat();
  const {
    roots,
    tasks,
    task,
    start,
    cancel,
    selectTask,
    selectRoot,
    rootId,
    selectedRoot,
    selectedAdult,
    availableRoots,
    matchingTasks,
    canStart,
  } = useScannerPreview(search, onSearch, adult);
  const librarySearch = {
    root: task?.root_id ?? rootId,
    ...(selectedAdult ? { kind: "adult_video" as const } : {}),
  };
  return (
    <>
      <PageHeading title={t("scanner")} />
      {roots.isSuccess && availableRoots.length === 0 && !search.task ? (
        <>
          <Empty
            title={t(roots.data.length ? "scanNoEnabledRoots" : "noRoots")}
          />
          <div className="mt-5">
            {onAddRoot ? (
              <Button onClick={onAddRoot}>{t("scanAddDirectory")}</Button>
            ) : (
              <Button asChild>
                <Link to="/roots" search={adult ? { kind: "adult_video" } : {}}>
                  {t("scanAddDirectory")}
                </Link>
              </Button>
            )}
          </div>
        </>
      ) : (
        <>
          <div className="panel mb-6">
            <div className="flex flex-wrap items-end gap-4">
              <Label className="filter-field min-w-0 flex-1">
                <span>{t("chooseRoot")}</span>
                <Select
                  value={rootId ?? ""}
                  disabled={start.isPending}
                  onChange={(event) => selectRoot(event.target.value)}
                >
                  <option value="">{t("chooseRoot")}</option>
                  {availableRoots.map((root) => (
                    <option key={root.id} value={root.id}>
                      {root.name}
                    </option>
                  ))}
                </Select>
              </Label>
              <Button
                disabled={!canStart || start.isPending}
                onClick={() => rootId && start.mutate(rootId)}
              >
                {t(start.isPending ? "rootScanning" : "autoRefreshLibrary")}
              </Button>
            </div>
            {roots.isError && (
              <ErrorNotice
                error={roots.error}
                retry={() => void roots.refetch()}
              />
            )}
            {rootId && roots.isSuccess && !selectedRoot && (
              <p className="mt-3 text-sm" role="status">
                {t("scanRootUnavailable")}
              </p>
            )}
            {start.isError && <ErrorNotice error={start.error} />}
          </div>
          <Label className="filter-field mb-6">
            <span>{t("scanChooseRecent")}</span>
            <Select
              value={task?.id ?? search.task ?? ""}
              disabled={cancel.isPending || matchingTasks.length === 0}
              onChange={(event) => selectTask(event.target.value)}
            >
              {matchingTasks.length === 0 && (
                <option value="">{t("noTasks")}</option>
              )}
              {search.task && !task && (
                <option value={search.task}>{t("taskNotAvailable")}</option>
              )}
              {matchingTasks.map((entry) => (
                <option key={entry.id} value={entry.id}>
                  {roots.data?.find((root) => root.id === entry.root_id)
                    ?.name ?? t("root")}{" "}
                  · {t(`state_${entry.state}`)} · {date(entry.created_at)}
                </option>
              ))}
            </Select>
          </Label>
          {tasks.isError && <ErrorNotice error={tasks.error} />}
          {!task && !search.task ? (
            tasks.isPending || roots.isPending ? (
              <Loading />
            ) : (
              <Empty title={t("autoRefreshReady")} />
            )
          ) : tasks.isPending ? (
            <Loading />
          ) : !task ? (
            <Empty title={t("taskNotAvailable")} />
          ) : (
            <>
              <div className="panel mb-5">
                <div className="flex flex-wrap justify-between gap-4">
                  <div>
                    <strong>
                      {t(
                        task.mode === "automatic" &&
                          (task.state === "queued" || task.state === "running")
                          ? `autoPhase_${task.phase ?? "scan"}`
                          : `state_${task.state}`,
                      )}
                    </strong>
                    <p className="mt-2 text-sm">
                      {t("visited")}: {number(task.visited_files)} ·{" "}
                      {t("candidates")}: {number(task.candidate_count)} ·{" "}
                      {t("warnings")}: {number(task.warnings)}
                    </p>
                  </div>
                  {(task.state === "queued" || task.state === "running") && (
                    <Button
                      variant="outline"
                      disabled={cancel.isPending}
                      onClick={() => cancel.mutate(task.id)}
                    >
                      {t("cancelTask")}
                    </Button>
                  )}
                </div>
                {task.error_code && (
                  <p role="alert" className="mt-3">
                    {t("scanError")}
                  </p>
                )}
              </div>
              {cancel.isError && <ErrorNotice error={cancel.error} />}
              {task.mode === "automatic" ? (
                <AutomaticScanResult task={task} adult={selectedAdult} />
              ) : (
                <div className="panel space-y-4">
                  <p role="status">
                    {t(
                      task.state === "queued" || task.state === "running"
                        ? "autoLegacyScanRunning"
                        : "autoLegacyScanReady",
                    )}
                  </p>
                  <Button asChild variant="outline">
                    <Link to="/library" search={librarySearch}>
                      {t("autoBrowseLibrary")}
                    </Link>
                  </Button>
                </div>
              )}
            </>
          )}
        </>
      )}
    </>
  );
}
