import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { libraryApi, type ScanTask } from "../../api/library";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "../../components/ui/dialog";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { ProviderTasks } from "../providers/pages";
import type { ScannerSearch } from "./scanner-navigation";
import { useScannerPreview } from "./use-scanner-preview";

export {
  type ScannerSearch,
  validateScannerSearch,
} from "./scanner-navigation";

import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  Pager,
  useFormat,
} from "./shared";

function active(task: ScanTask) {
  return ["queued", "running", "waiting_for_user"].includes(task.state);
}
export function TasksPage() {
  const { t } = useTranslation();
  const { number, date } = useFormat();
  const client = useQueryClient();
  const tasks = useQuery({
    queryKey: ["tasks", false],
    queryFn: () => libraryApi.tasks(false),
    refetchInterval: 2000,
  });
  const cancel = useMutation({
    mutationFn: (id: string) => libraryApi.cancel(id),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["tasks"] });
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
                    {t("taskId")}: {task.id}
                  </p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {date(task.updated_at)}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button asChild variant="ghost">
                    <Link
                      to="/scanner"
                      search={{ task: task.id }}
                      className="action-link"
                    >
                      {t("preview")}
                    </Link>
                  </Button>
                  {active(task) && (
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
}: {
  adult?: boolean;
  search: ScannerSearch;
  onSearch: (value: ScannerSearch) => void;
}) {
  const { t } = useTranslation();
  const { number } = useFormat();
  const confirmTrigger = useRef<HTMLButtonElement>(null);
  const {
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
  } = useScannerPreview(search, onSearch, adult);
  return (
    <>
      <PageHeading title={t("scanner")} />
      <div className="panel mb-6">
        <div className="flex flex-wrap items-end gap-4">
          <Label className="filter-field min-w-64 flex-1">
            <span>{t("chooseRoot")}</span>
            <Select
              value={search.root ?? ""}
              onChange={(event) =>
                onSearch({ root: event.target.value || undefined })
              }
            >
              <option value="">{t("chooseRoot")}</option>
              {roots.data
                ?.filter((root) => root.enabled)
                .map((root) => (
                  <option key={root.id} value={root.id}>
                    {root.name}
                  </option>
                ))}
            </Select>
          </Label>
          <Button
            disabled={!search.root || start.isPending}
            onClick={() => search.root && start.mutate(search.root)}
          >
            {t("startScan")}
          </Button>
        </div>
        {roots.isError && <ErrorNotice error={roots.error} />}
        {start.isError && <ErrorNotice error={start.error} />}
      </div>
      <Label className="filter-field mb-6">
        <span>{t("chooseTask")}</span>
        <Select
          value={search.task ?? ""}
          onChange={(event) => selectTask(event.target.value)}
        >
          <option value="">{t("chooseTask")}</option>
          {tasks.data?.map((entry) => (
            <option key={entry.id} value={entry.id}>
              {t(`state_${entry.state}`)} · {entry.id}
            </option>
          ))}
        </Select>
      </Label>
      {tasks.isError && <ErrorNotice error={tasks.error} />}
      {!search.task ? (
        <Empty title={t("chooseTask")} />
      ) : tasks.isPending ? (
        <Loading />
      ) : !task ? (
        <Empty title={t("taskNotAvailable")} />
      ) : (
        <>
          <div className="panel mb-5">
            <div className="flex flex-wrap justify-between gap-4">
              <div>
                <strong>{t(`state_${task.state}`)}</strong>
                <p className="mt-2 text-sm">
                  {t("visited")}: {number(task.visited_files)} ·{" "}
                  {t("candidates")}: {number(task.candidate_count)} ·{" "}
                  {t("warnings")}: {number(task.warnings)}
                </p>
              </div>
              {active(task) && (
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
          {confirm.isError && <ErrorNotice error={confirm.error} />}
          {confirm.isSuccess && (
            <p role="status" className="notice mb-4">
              {t("importResult", {
                imported: confirm.data.imported,
                unchanged: confirm.data.unchanged,
              })}
            </p>
          )}
          {preview.isPending ? (
            <Loading />
          ) : preview.isError ? (
            <ErrorNotice
              error={preview.error}
              retry={() => void preview.refetch()}
            />
          ) : !preview.data.items.length ? (
            <Empty title={t("noCandidates")} />
          ) : (
            <>
              <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
                <Label className="flex items-center gap-2">
                  <Checkbox
                    aria-label={t("selectPage")}
                    disabled={!canImport || !pendingIds.length}
                    checked={
                      pendingIds.length > 0 &&
                      pendingIds.every((id) => selected.includes(id))
                    }
                    onChange={(event) =>
                      setSelected(event.target.checked ? pendingIds : [])
                    }
                  />
                  {t("selectPage")}
                </Label>
                <span>{t("selectedCount", { count: selected.length })}</span>
                <Button
                  ref={confirmTrigger}
                  disabled={!canImport || !selected.length || confirm.isPending}
                  onClick={() => setConfirmOpen(true)}
                >
                  {t("confirmImport")}
                </Button>
              </div>
              {preview.data.items.some(
                (candidate) => candidate.needs_review,
              ) && <p className="notice mb-4">{t("reviewNotice")}</p>}
              <div className="table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>{t("confirmImport")}</th>
                      <th>{t("titleLabel")}</th>
                      <th>{t("path")}</th>
                      <th>{t("state")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {preview.data.items.map((candidate) => (
                      <tr key={candidate.id}>
                        <td>
                          <Checkbox
                            aria-label={`${t("confirmImport")}: ${candidate.title}`}
                            checked={selected.includes(candidate.id)}
                            disabled={
                              !canImport || candidate.status !== "pending"
                            }
                            onChange={(event) =>
                              setSelected((old) =>
                                event.target.checked
                                  ? [...old, candidate.id]
                                  : old.filter((id) => id !== candidate.id),
                              )
                            }
                          />
                        </td>
                        <td>
                          <p className="font-medium">{candidate.title}</p>
                          <p className="mt-1 text-xs text-muted-foreground">
                            {t(`kind_${candidate.kind}`)}{" "}
                            {candidate.year ? `· ${candidate.year}` : ""}
                            {candidate.season_number !== null &&
                              ` · ${t("season")} ${number(candidate.season_number)}`}
                            {candidate.episode_number !== null &&
                              ` · ${t("episode")} ${number(candidate.episode_number)}`}
                            {candidate.catalog_number &&
                              ` · ${candidate.catalog_number}`}
                          </p>
                          {candidate.needs_review && (
                            <p className="mt-2 text-xs">{t("needsReview")}</p>
                          )}
                          {candidate.warnings.length > 0 && (
                            <p className="text-xs text-muted-foreground">
                              {t("warningGeneric")}
                            </p>
                          )}
                        </td>
                        <td className="break-all text-xs">
                          {candidate.relative_path}
                        </td>
                        <td>
                          <span className="badge">{t(candidate.status)}</span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <Pager
                page={page + 1}
                hasNext={Boolean(preview.data.next_cursor)}
                previous={() => {
                  setPage((old) => Math.max(0, old - 1));
                  setSelected([]);
                }}
                next={() => {
                  const cursor = preview.data?.next_cursor;
                  if (!cursor) return;
                  setCursors((old) => [...old.slice(0, page + 1), cursor]);
                  setPage((old) => old + 1);
                  setSelected([]);
                }}
              />
            </>
          )}
          {confirmOpen && (
            <Dialog
              open
              onOpenChange={(open) => {
                if (!confirm.isPending) setConfirmOpen(open);
              }}
            >
              <DialogContent
                onCloseAutoFocus={(event) => {
                  event.preventDefault();
                  confirmTrigger.current?.focus();
                }}
              >
                <DialogTitle className="section-title">
                  {t("confirmStep")}
                </DialogTitle>
                <DialogDescription className="mb-4">
                  {t("confirmExplanation", { count: selected.length })}
                </DialogDescription>
                <div className="flex flex-wrap gap-3">
                  <Button
                    disabled={confirm.isPending || !canImport}
                    onClick={() => confirm.mutate()}
                  >
                    {t(confirm.isPending ? "importing" : "applyImport")}
                  </Button>
                  <Button
                    variant="outline"
                    onClick={() => setConfirmOpen(false)}
                  >
                    {t("cancel")}
                  </Button>
                </div>
              </DialogContent>
            </Dialog>
          )}
        </>
      )}
    </>
  );
}
