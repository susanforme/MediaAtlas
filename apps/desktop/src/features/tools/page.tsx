import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { Activity, CheckCircle2, FileVideo, LoaderCircle } from "lucide-react";
import { lazy, Suspense, useState } from "react";
import { useTranslation } from "react-i18next";
import { DesktopError } from "../../api/desktop";
import {
  maintenanceApi,
  type OrganizationStrategy,
} from "../../api/organization";
import { Button } from "../../components/ui/button";
import { Badge } from "../../components/ui/badge";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Details, DetailsSummary } from "../../components/ui/details";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import { rootsQuery } from "../library/queries";
import { OrdinaryCandidates } from "./ordinary-candidates";
import { toolsDetailQuery, watchersQuery } from "./queries";
import { ToolsSectionNavigation } from "./section-navigation";
import { type ToolsRouteSearch, validateToolsRouteSearch } from "./search";
import { useOrganization } from "./use-organization";
import "./translations";
import "../scraper/translations";

const AdultScraper = lazy(() =>
  import("../adult/page").then((m) => ({ default: m.AdultScraper })),
);
const AdultOutput = lazy(() =>
  import("../adult/page").then((m) => ({ default: m.AdultOutput })),
);
export function OrganizationPanel({
  mediaId,
  adult = false,
}: {
  mediaId: string;
  adult?: boolean;
}) {
  const { t } = useTranslation();
  const {
    rollback,
    target,
    setTarget,
    template,
    setTemplate,
    strategy,
    setStrategy,
    plan,
    historyId,
    setHistoryId,
    roots,
    history,
    journal,
    savedPlan,
    preview,
    restore,
    execute,
    cancel,
    discard,
  } = useOrganization(mediaId, adult);
  return (
      <section className="space-y-5">
        <div className="grid gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1.6fr)_minmax(140px,0.8fr)]">
          <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
            <span>{t("organizeTarget")}</span>
            <Select
              disabled={
                preview.isPending ||
                restore.isPending ||
                execute.isPending ||
                cancel.isPending
              }
              value={target}
              onChange={(e) => {
                discard();
                setTarget(e.target.value);
              }}
            >
              <option value="">{t("organizeTarget")}</option>
              {roots.data
                ?.filter(
                  (r) =>
                    r.enabled &&
                    (adult ? r.kind === "adult_video" : r.kind !== "adult_video"),
                )
                .map((r) => (
                  <option value={r.id} key={r.id}>
                    {r.name}
                  </option>
                ))}
            </Select>
          </Label>
          <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
            <span>{t("organizeTemplate")}</span>
            <Input
              disabled={
                preview.isPending ||
                restore.isPending ||
                execute.isPending ||
                cancel.isPending
              }
              value={template}
              onChange={(e) => {
                discard();
                setTemplate(e.target.value);
              }}
            />
          </Label>
          <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
            <span>{t("organizeStrategy")}</span>
            <Select
              disabled={
                preview.isPending ||
                restore.isPending ||
                execute.isPending ||
                cancel.isPending
              }
              value={strategy}
              onChange={(e) => {
                discard();
                setStrategy(e.target.value as OrganizationStrategy);
              }}
            >
              {(["preserve", "move", "copy", "hardlink"] as const).map((s) => (
                <option value={s} key={s}>
                  {t(`mode_${s}`)}
                </option>
              ))}
            </Select>
          </Label>
        </div>
        <Button
          disabled={
            !target ||
            preview.isPending ||
            restore.isPending ||
            execute.isPending ||
            cancel.isPending
          }
          onClick={() => preview.mutate()}
        >
          {t("organizePreview")}
        </Button>
        {plan && (
          <section className="space-y-3">
            <p>
              {t("organizeSpace")}: {plan.estimated_bytes} · {t("organizeFiles")}:{" "}
              {plan.file_count}
            </p>
            <ul>
              {plan.warnings.map((w) => (
                <li key={w}>
                  {t(
                    w ===
                      "Hardlinks share bytes with the source; edits affect both names. Unsupported filesystems fail without fallback."
                      ? "organizeHardlinkWarning"
                      : w ===
                          "Quarantines are retained for explicit recovery; organization never overwrites existing files."
                        ? "organizeQuarantineWarning"
                        : w ===
                            "Recovery quarantines owned destinations and restores moved originals without replacing external files."
                          ? "organizeRecoveryWarning"
                          : w,
                    { defaultValue: w },
                  )}
                </li>
              ))}
            </ul>
            <ul>
              {plan.files.map((f) => (
                <li
                  className="break-all border-b py-2"
                  key={`${f.source}:${f.target}`}
                >
                  <p>
                    {f.source} → {f.target}
                  </p>
                  <p>
                    {t(`organizationRole_${f.role}`, { defaultValue: f.role })} ·{" "}
                    {f.bytes} B
                  </p>
                  {f.conflict && (
                    <p role="alert">
                      {t("organizeConflict")}: {f.conflict}
                    </p>
                  )}
                </li>
              ))}
            </ul>
            <div className="flex gap-3">
              <Button
                disabled={
                  execute.isPending || plan.files.some((f) => !!f.conflict)
                }
                onClick={() => execute.mutate()}
              >
                {t(rollback ? "organizeRestoreConfirm" : "organizeConfirm")}
              </Button>
              <Button
                variant="outline"
                onClick={() => cancel.mutate()}
                disabled={cancel.isPending}
              >
                {t("organizeCancel")}
              </Button>
              <Button variant="outline" onClick={discard}>
                {t("organizeDiscard")}
              </Button>
            </div>
          </section>
        )}
        {[
          preview.error,
          restore.error,
          execute.error,
          cancel.error,
          history.error,
          roots.error,
        ]
          .filter(Boolean)
          .map((e, index) => (
            <ErrorNotice key={String(index)} error={e} />
          ))}
        {Boolean(history.data?.length) && <section className="border-t border-border pt-4">
        <h3 className="mb-3 text-sm font-medium">{t("organizeHistory")}</h3>
        <ul className="divide-y divide-border text-sm">
          {history.data?.map((h) => (
            <li key={h.id} className="flex flex-wrap items-center gap-2 py-3">
              <p>
                {h.id} · {t(`state_${h.state}`, { defaultValue: h.state })} ·{" "}
                {h.completed}
                {h.error_code ? ` · ${h.error_code}` : ""}
              </p>
              <Button variant="ghost" size="sm" onClick={() => setHistoryId(h.id)}>
                {t("organizeLog")}
              </Button>
              {["succeeded", "failed", "interrupted", "cancelled"].includes(
                h.state,
              ) && (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={
                    preview.isPending ||
                    restore.isPending ||
                    execute.isPending ||
                    cancel.isPending
                  }
                  onClick={() => restore.mutate(h.id)}
                >
                  {t("organizeRestorePreview")}
                </Button>
              )}
            </li>
          ))}
        </ul>
        </section>}
        {historyId && (
          <section className="space-y-2">
            <h3>
              {t("organizeLog")}: {historyId}
            </h3>
            {journal.error && <ErrorNotice error={journal.error} />}
            {savedPlan.error && <ErrorNotice error={savedPlan.error} />}
            <ol>
              {journal.data?.map((step) => (
                <li key={`${step.file_index}:${step.stage}:${step.created_at}`}>
                  {step.created_at} · {step.file_index} ·{" "}
                  {t(`organizationStage_${step.stage}`, {
                    defaultValue: step.stage,
                  })}
                </li>
              ))}
            </ol>
            <ul>
              {savedPlan.data?.files.map((f) => (
                <li className="break-all" key={`${f.source}:${f.target}`}>
                  {f.source} → {f.target}
                </li>
              ))}
            </ul>
          </section>
        )}
      </section>
  );
}
export function LocalCandidates({
  adult = false,
  renderExtra,
  initialMediaId,
  search,
  onSearch,
}: {
  adult?: boolean;
  initialMediaId?: string;
  search?: ToolsRouteSearch;
  onSearch?: (search: ToolsRouteSearch, replace?: boolean) => void;
  renderExtra?: (id: string) => React.ReactNode;
}) {
  const context = validateToolsRouteSearch(
    search
      ? { ...search, ...(adult ? { kind: "adult_video" } : {}) }
      : { media: initialMediaId, ...(adult ? { kind: "adult_video" } : {}) },
  );
  return (
    <OrdinaryCandidates
      search={context}
      onSearch={onSearch}
      renderDetail={(mediaId, close, adult) => (
        <LocalCandidateDetail
          key={mediaId}
          mediaId={mediaId}
          adult={adult}
          onClose={close}
          renderExtra={renderExtra}
        />
      )}
    />
  );
}

function LocalCandidateDetail({
  mediaId,
  adult = false,
  renderExtra,
  onClose,
}: {
  mediaId: string;
  adult?: boolean;
  renderExtra?: (id: string) => React.ReactNode;
  onClose?: () => void;
}) {
  const { t } = useTranslation();
  const detail = useQuery(toolsDetailQuery(mediaId, adult));
  if (detail.isPending) return <Loading />;
  if (detail.isError)
    return (
      <ErrorNotice error={detail.error} retry={() => void detail.refetch()} />
    );
  if (
    detail.data.media.id !== mediaId ||
    (adult
      ? detail.data.media.kind !== "adult_video"
      : detail.data.media.kind === "adult_video")
  )
    return <ErrorNotice error={new DesktopError("invalid_response")} />;
  return (
    <>
      <Card asChild className="bg-muted/20 p-4 sm:p-5">
        <section>
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <h2 className="font-semibold">{detail.data.media.title}</h2>
            <div className="flex flex-wrap gap-2">
              {!adult &&
                ["movie", "tv_series", "anime"].includes(
                  detail.data.media.kind,
                ) && (
                  <Button asChild size="sm">
                    <Link to="/scraper" search={{ media: mediaId }}>
                      {t("scraperThisMedia")}
                    </Link>
                  </Button>
                )}
              {onClose && (
                <Button variant="ghost" size="sm" onClick={onClose}>
                  {t("toolsCloseDetail")}
                </Button>
              )}
            </div>
          </div>
          <ul className="divide-y divide-border text-sm">
            {detail.data.files.map((file) => (
              <li className="flex items-start gap-2.5 py-2 first:pt-0 last:pb-0" key={file.id}>
                <FileVideo size={16} className="mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />
                <span className="min-w-0 break-all">{file.relative_path}</span>
              </li>
            ))}
          </ul>
        </section>
      </Card>
      <Details className="bg-muted/20 p-4 sm:px-5">
        <DetailsSummary className="font-medium">{t("organize")}</DetailsSummary>
        <div className="mt-5 border-t border-border pt-5"><OrganizationPanel key={mediaId} mediaId={mediaId} adult={adult} /></div>
      </Details>
      {adult && (
        <Suspense fallback={<Loading />}>
          <AdultScraper key={mediaId} mediaId={mediaId} />
          <AdultOutput mediaId={mediaId} />
        </Suspense>
      )}
      {renderExtra?.(mediaId)}
    </>
  );
}
function Maintenance({ search }: { search: ToolsRouteSearch }) {
  const { t } = useTranslation();
  const [cursor, setCursor] = useState<string | null>(null);
  const watchers = useQuery({
    ...watchersQuery,
    refetchInterval: 2500,
  });
  const roots = useQuery(rootsQuery());
  const configure = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      maintenanceApi.configure(id, enabled),
    onSuccess: () => void watchers.refetch(),
  });
  const health = useMutation({
    mutationFn: (next: string | null) => maintenanceApi.health(next),
  });
  return (
    <div className="space-y-8">
        <section aria-labelledby="tools-watchers" className="border-t border-border pt-6">
          <h2 id="tools-watchers" className="text-lg font-semibold">{t("watcherTitle")}</h2>
          <p className="mt-2 max-w-3xl text-sm leading-relaxed text-muted-foreground">{t("watcherHelp")}</p>
          {watchers.isPending && <p role="status" className="py-6 text-sm text-muted-foreground">{t("loading")}</p>}
          <ul className="mt-4 divide-y divide-border">
          {watchers.data?.map((w) => (
            <li key={w.root_id} className="flex flex-wrap items-center justify-between gap-3 py-4">
              <Label className="flex min-w-0 items-center gap-3 text-sm font-medium">
                <Checkbox
                  checked={w.opted_in}
                  disabled={configure.isPending}
                  onChange={(e) =>
                    configure.mutate({ id: w.root_id, enabled: e.target.checked })
                  }
                />
                {roots.data?.find((r) => r.id === w.root_id)?.name ?? t("root")}
              </Label>
              <div className="flex flex-wrap gap-2">
                <Badge variant={w.active ? "secondary" : "outline"}>{t(w.active ? "watcherActive" : "watcherInactive")}</Badge>
                {w.pending_rescan && <Badge variant="secondary">{t("watcherPending")}</Badge>}
                {w.error_code && <Badge variant="destructive">{t("watcherError")}</Badge>}
              </div>
            </li>
          ))}
          </ul>
          {watchers.error && <ErrorNotice error={watchers.error} retry={() => void watchers.refetch()} />}
          {configure.error && <ErrorNotice error={configure.error} />}
          {roots.error && <ErrorNotice error={roots.error} retry={() => void roots.refetch()} />}
          {watchers.isSuccess && watchers.data.length === 0 && (
            <div className="flex flex-wrap items-center gap-4 py-4">
              <p className="text-sm text-muted-foreground">{t("toolsWatchersEmpty")}</p>
              <Button asChild variant="outline" size="sm"><Link to="/roots">{t("roots")}</Link></Button>
            </div>
          )}
        </section>
        <section aria-labelledby="tools-health" aria-busy={health.isPending} className="space-y-4 border-t border-border pt-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h2 id="tools-health" className="text-lg font-semibold">{t("healthIssues")}</h2>
            <Button
              size="sm"
              disabled={health.isPending}
              onClick={() => {
                setCursor(null);
                health.mutate(null);
              }}
            >
              {health.isPending && <LoaderCircle size={15} className="motion-safe:animate-spin" aria-hidden="true" />}
              {t("healthCheck")}
            </Button>
          </div>
          {health.error && <ErrorNotice error={health.error} retry={() => health.mutate(cursor)} />}
          {health.isIdle && <p role="status" className="flex items-center gap-3 py-4 text-sm text-muted-foreground"><Activity size={18} aria-hidden="true" />{t("toolsHealthIdle")}</p>}
          {health.isPending && <p role="status" className="py-4 text-sm text-muted-foreground">{t("toolsHealthChecking")}</p>}
          {health.isSuccess && health.data.items.length === 0 && <p role="status" className="flex items-center gap-3 py-4 text-sm text-muted-foreground"><CheckCircle2 size={18} aria-hidden="true" />{t("toolsHealthClear")}</p>}
          {health.isSuccess && <ul className="divide-y divide-border">
            {health.data.items.map((i) => (
              <li className="flex flex-wrap items-center justify-between gap-3 py-4" key={i.media_id}>
                <div className="min-w-0 space-y-2">
                  <p className="font-medium wrap-anywhere">{i.title}</p>
                  <div className="flex flex-wrap gap-2">
                    {i.issues.map((issue) => <Badge key={issue} variant="secondary">{t(`issue_${issue}`, { defaultValue: issue })}</Badge>)}
                  </div>
                </div>
                <Button asChild variant="outline" size="sm"><Link to="/tools" search={{ ...search, view: undefined, media: i.media_id }}>{t("toolsRepair")}</Link></Button>
              </li>
            ))}
          </ul>}
          {health.isSuccess && health.data.next_cursor && (
            <Button
              variant="outline"
              size="sm"
              disabled={health.isPending}
              onClick={() => {
                const next = health.data?.next_cursor ?? cursor;
                setCursor(next);
                health.mutate(next);
              }}
            >
              {t("healthMore")}
            </Button>
          )}
        </section>
    </div>
  );
}
export function MediaToolsPage({
  mediaId,
  search,
  onSearch,
}: {
  mediaId?: string;
  search?: ToolsRouteSearch;
  onSearch?: (search: ToolsRouteSearch, replace?: boolean) => void;
} = {}) {
  return (
    <MediaToolsWorkspace
      key={onSearch ? undefined : (mediaId ?? "tools")}
      mediaId={mediaId}
      search={search}
      onSearch={onSearch}
    />
  );
}

function MediaToolsWorkspace({
  mediaId,
  search,
  onSearch,
}: {
  mediaId?: string;
  search?: ToolsRouteSearch;
  onSearch?: (search: ToolsRouteSearch, replace?: boolean) => void;
}) {
  const { t } = useTranslation();
  const adult = search?.kind === "adult_video";
  const context = validateToolsRouteSearch({ ...search, media: search?.media ?? mediaId });

  return (
    <>
      <PageHeading title={t(!adult && context.view === "health" ? "toolsHealth" : "toolsLocal")}>
        <div className="flex flex-wrap items-center gap-2">
        <Button asChild variant="outline" size="sm">
          <Link to="/scanner">{t("scanner")}</Link>
        </Button>
        <Button asChild variant="outline" size="sm">
          <Link to="/library" search={{ kind: search?.kind }}>
            {t("library")}
          </Link>
        </Button>
        </div>
      </PageHeading>
      <ToolsSectionNavigation current={context.view === "health" ? "health" : "local"} search={context} />
      {adult || context.view !== "health" ? (
        <LocalCandidates
          initialMediaId={mediaId}
          search={search}
          onSearch={onSearch}
        />
      ) : (
        <Maintenance search={context} />
      )}
    </>
  );
}
