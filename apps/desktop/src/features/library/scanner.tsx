import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import type { ScannerSearch } from "./scanner-navigation";
import "./translations";
import { AutomaticScanResult } from "./automatic-scan-result";
import { refreshIsRunning, refreshResultTitle } from "./library-activity-model";
import "./library-status-translations";
import { ScanRecovery } from "./scan-recovery";
import { useScannerPreview } from "./use-scanner-preview";

export {
  type ScannerSearch,
  validateScannerSearch,
} from "./scanner-navigation";

import { Empty, ErrorNotice, Loading, PageHeading, useFormat } from "./shared";

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
          <section className="mb-5 space-y-3 [&_[data-slot=select]]:h-9">
              <div className="flex flex-wrap items-center gap-3">
                <Label className="block w-full min-w-0 sm:w-60">
                  <span className="sr-only">{t("chooseRoot")}</span>
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
                <Label className="block w-full min-w-0 sm:min-w-56 sm:flex-1 sm:max-w-md">
                  <span className="sr-only">{t("scanChooseRecent")}</span>
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
                        {roots.data?.find((root) => root.id === entry.root_id)?.name ?? t("root")}
                        {" · "}
                        {t(entry.mode === "automatic" && !refreshIsRunning(entry)
                          ? refreshResultTitle(entry)
                          : `state_${entry.state}`)}
                        {" · "}{date(entry.created_at)}
                      </option>
                    ))}
                  </Select>
                </Label>
                <Button
                  size="sm"
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
          </section>
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
              <Card asChild>
                <div className="mb-5">
                  <div className="flex flex-wrap justify-between gap-4">
                    <div>
                      <strong>
                        {t(
                          task.mode === "automatic" &&
                            (task.state === "queued" || task.state === "running")
                            ? `autoPhase_${task.phase ?? "scan"}`
                            : task.mode === "automatic"
                              ? refreshResultTitle(task)
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
                  <ScanRecovery
                    task={task}
                    adult={selectedAdult}
                    onRetry={() => rootId && start.mutate(rootId)}
                    retryDisabled={!canStart || start.isPending}
                  />
                </div>
              </Card>
              {cancel.isError && <ErrorNotice error={cancel.error} />}
              {task.mode === "automatic" ? (
                <AutomaticScanResult task={task} adult={selectedAdult} />
              ) : (
                <Card asChild>
                  <div className="space-y-4">
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
                </Card>
              )}
            </>
          )}
        </>
      )}
    </>
  );
}
