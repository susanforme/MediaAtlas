import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import {
  refreshIsRunning,
  refreshNeedsAttention,
  refreshResultTitle,
} from "./library-activity-model";
import { ScanRecovery } from "./scan-recovery";
import { Empty, ErrorNotice, PageHeading, useFormat } from "./shared";
import { scanLogCounts } from "./task-log-model";
import { useTaskLog } from "./use-task-log";
import "./translations";
import "./library-status-translations";
import "./task-log-translations";

export function TasksPage() {
  const { t } = useTranslation();
  const { date } = useFormat();
  const { entries, roots, providers, loading, errors, cancel } = useTaskLog();
  return (
    <>
      <PageHeading title={t("tasks")} />
      <div className="space-y-3">
        {errors.map(({ name, query }) => (
          <div key={name}>
            <p className="mb-2 text-sm font-medium">{t(name)}</p>
            <ErrorNotice error={query.error} retry={() => void query.refetch()} />
          </div>
        ))}
        {roots.isError && (
          <ErrorNotice error={roots.error} retry={() => void roots.refetch()} />
        )}
        {cancel.isError && <ErrorNotice error={cancel.error} />}
      </div>
      {loading && (
        <p role="status" className="py-3 text-sm text-muted-foreground">
          {t("loading")}
        </p>
      )}
      {!entries.length && !loading && !errors.length ? (
        <Empty title={t("taskLogEmpty")} />
      ) : entries.length > 0 ? (
        <ol
          aria-label={t("taskLog")}
          className="divide-y divide-border border-y border-border"
        >
          {entries.map((entry) => {
            const { task } = entry;
            const scan = entry.source === "scan" ? entry.task : null;
            const provider = entry.source === "provider" ? entry.task : null;
            const source = providers.find(
              (item) => item.descriptor.id === provider?.provider_id,
            );
            const providerName = source?.descriptor.fixture
              ? t("providerFixtureName")
              : source?.descriptor.name ?? t("taskLogSource");
            const title = scan
              ? t(scan.mode === "automatic" ? "taskLogRefresh" : "taskLogScan", {
                  name:
                    roots.data.find((root) => root.id === scan.root_id)?.name ??
                    t("root"),
                })
              : t("taskLogSearch", { name: providerName, query: provider?.query });
            const state =
              scan?.mode === "automatic" &&
              ["succeeded", "waiting_for_user"].includes(scan.state) &&
              refreshNeedsAttention(scan)
                ? "taskLogAttention"
                : `state_${task.state}`;
            const cancellable =
              ["queued", "running"].includes(task.state) ||
              (task.state === "waiting_for_user" && scan?.mode !== "automatic");
            const summary =
              scan?.mode === "automatic"
                ? scan.state === "running" && scan.phase
                  ? `autoPhase_${scan.phase}`
                  : !refreshIsRunning(scan)
                    ? refreshResultTitle(scan)
                    : null
                : null;
            return (
              <li
                key={`${entry.source}:${entry.source === "scan" && entry.adult ? "adult:" : ""}${task.id}`}
                className="grid min-w-0 grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 py-3 lg:grid-cols-[10rem_8rem_minmax(0,1fr)_auto] lg:items-start"
              >
                <time
                  dateTime={task.updated_at}
                  className="pt-1 text-xs tabular-nums text-muted-foreground"
                >
                  {date(task.updated_at)}
                </time>
                <div className="min-w-0 justify-self-end lg:justify-self-start">
                  <Badge
                    className="max-w-full whitespace-normal px-2 py-0.5"
                    variant={task.state === "failed" ? "destructive" : "secondary"}
                  >
                    {t(state)}
                  </Badge>
                </div>
                <div className="col-span-2 min-w-0 space-y-1 text-sm lg:col-span-1">
                  <p className="break-words font-medium">{title}</p>
                  {scan && (
                    <>
                      <p className="text-xs leading-5 text-muted-foreground">
                        {[
                          ...(summary ? [t(summary)] : []),
                          ...scanLogCounts(scan).map(([key, count]) =>
                            t(key, { count }),
                          ),
                        ].join(" · ")}
                      </p>
                      <ScanRecovery
                        task={scan}
                        adult={entry.source === "scan" && entry.adult}
                      />
                    </>
                  )}
                  {provider?.failure && (
                    <p role="status" className="text-xs text-destructive">
                      {t(`providerFailure_${provider.failure}`)}
                    </p>
                  )}
                  {provider?.result_summary && (
                    <p className="text-xs text-muted-foreground">
                      {t(`providerResult_${provider.result_summary}`, {
                        defaultValue: t("providerResultReady"),
                      })}
                    </p>
                  )}
                </div>
                <div className="col-span-2 flex flex-wrap gap-1 lg:col-span-1">
                  {scan && (
                    <Button asChild size="sm" variant="ghost">
                      <Link
                        to="/scanner"
                        search={{
                          root: scan.root_id,
                          task: scan.id,
                          ...(entry.source === "scan" && entry.adult
                            ? { kind: "adult_video" as const }
                            : {}),
                        }}
                      >
                        {t("details")}
                      </Link>
                    </Button>
                  )}
                  {cancellable && (
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={cancel.isPending}
                      onClick={() => cancel.mutate(entry)}
                    >
                      {t("cancelTask")}
                    </Button>
                  )}
                </div>
              </li>
            );
          })}
        </ol>
      ) : null}
    </>
  );
}
