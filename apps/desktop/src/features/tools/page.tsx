import { useMutation, useQuery } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { lazy, Suspense, useState } from "react";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import { DesktopError } from "../../api/desktop";
import { libraryApi } from "../../api/library";
import {
  maintenanceApi,
  type OrganizationStrategy,
} from "../../api/organization";
import { Anchor, Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import { OrdinaryCandidates } from "./ordinary-candidates";
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
const ScraperPage = lazy(() =>
  import("../scraper/page").then((m) => ({ default: m.ScraperPage })),
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
    <Card asChild>
      <section className="space-y-4">
        <h2 className="mb-4 font-semibold">{t("organize")}</h2>
        <div className="grid grid-cols-[repeat(auto-fit,minmax(min(150px,100%),1fr))] gap-4">
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
        <h3>{t("organizeHistory")}</h3>
        <ul>
          {history.data?.map((h) => (
            <li key={h.id} className="py-2">
              <p>
                {h.id} · {t(`state_${h.state}`, { defaultValue: h.state })} ·{" "}
                {h.completed}
                {h.error_code ? ` · ${h.error_code}` : ""}
              </p>
              <Button variant="outline" onClick={() => setHistoryId(h.id)}>
                {t("organizeLog")}
              </Button>
              {["succeeded", "failed", "interrupted", "cancelled"].includes(
                h.state,
              ) && (
                <Button
                  variant="outline"
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
    </Card>
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
  const detail = useQuery({
    queryKey: adult
      ? ["adult", "tools-detail", mediaId]
      : ["tools-detail", mediaId],
    queryFn: () =>
      adult ? adultApi.toolsDetail(mediaId) : libraryApi.toolsDetail(mediaId),
  });
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
      <Card asChild>
        <section>
          <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
            <h2 className="font-semibold">{detail.data.media.title}</h2>
            <div className="flex flex-wrap gap-2">
              {!adult &&
                ["movie", "tv_series", "anime"].includes(
                  detail.data.media.kind,
                ) && (
                  <Button asChild>
                    <Link to="/scraper" search={{ media: mediaId }}>
                      {t("scraperThisMedia")}
                    </Link>
                  </Button>
                )}
              {onClose && (
                <Button variant="ghost" onClick={onClose}>
                  {t("toolsCloseDetail")}
                </Button>
              )}
            </div>
          </div>
          <ul>
            {detail.data.files.map((file) => (
              <li className="break-all" key={file.id}>
                {file.relative_path}
              </li>
            ))}
          </ul>
        </section>
      </Card>
      <OrganizationPanel key={mediaId} mediaId={mediaId} adult={adult} />
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
function Maintenance() {
  const { t } = useTranslation();
  const [cursor, setCursor] = useState<string | null>(null);
  const watchers = useQuery({
    queryKey: ["watchers"],
    queryFn: maintenanceApi.watchers,
    refetchInterval: 2500,
  });
  const roots = useQuery({
    queryKey: ["roots", false],
    queryFn: () => libraryApi.roots(false),
  });
  const configure = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      maintenanceApi.configure(id, enabled),
    onSuccess: () => void watchers.refetch(),
  });
  const health = useMutation({
    mutationFn: (next: string | null) => maintenanceApi.health(next),
  });
  return (
    <>
      <Card asChild>
        <section className="space-y-4">
          <h2 className="mb-4 font-semibold">{t("watcherTitle")}</h2>
          <p>{t("watcherHelp")}</p>
          {watchers.data?.map((w) => (
            <div key={w.root_id}>
              <Label className="flex gap-3">
                <Checkbox
                  checked={w.opted_in}
                  disabled={configure.isPending}
                  onChange={(e) =>
                    configure.mutate({ id: w.root_id, enabled: e.target.checked })
                  }
                />
                {t("watcherEnable")}:{" "}
                {roots.data?.find((r) => r.id === w.root_id)?.name ?? w.root_id}
              </Label>
              <p>
                {t(w.active ? "watcherActive" : "watcherInactive")}
                {w.pending_rescan ? ` · ${t("watcherPending")}` : ""}
                {w.error_code ? ` · ${w.error_code}` : ""}
              </p>
            </div>
          ))}
          {watchers.error && <ErrorNotice error={watchers.error} />}
          {configure.error && <ErrorNotice error={configure.error} />}
        </section>
      </Card>
      <Card asChild>
        <section className="space-y-4">
          <h2 className="mb-4 font-semibold">{t("healthIssues")}</h2>
          <Button
            disabled={health.isPending}
            onClick={() => {
              setCursor(null);
              health.mutate(null);
            }}
          >
            {t("healthCheck")}
          </Button>
          {health.error && <ErrorNotice error={health.error} />}
          <ul>
            {health.data?.items.map((i) => (
              <li className="py-3" key={i.media_id}>
                <p>
                  {i.title}:{" "}
                  {i.issues
                    .map((issue) => t(`issue_${issue}`, { defaultValue: issue }))
                    .join(" · ")}
                </p>
                <Anchor href={`/tools?media=${encodeURIComponent(i.media_id)}`}>
                  {t("toolsRepair")}
                </Anchor>
              </li>
            ))}
          </ul>
          {health.data?.next_cursor && (
            <Button
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
      </Card>
    </>
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
  const [tab, setTab] = useState<"local" | "scraper" | "health">("local");
  const adult = search?.kind === "adult_video";

  return (
    <>
      <PageHeading title={t("videoTools")}>
        <Button asChild variant="outline">
          <Link to="/scanner">{t("scanner")}</Link>
        </Button>
        <Button asChild variant="outline">
          <Link to="/library" search={{ kind: search?.kind }}>
            {t("library")}
          </Link>
        </Button>
      </PageHeading>
      {!adult && (
        <fieldset
          className="mb-7 flex flex-wrap gap-2"
          aria-label={t("videoTools")}
        >
          {(["local", "scraper", "health"] as const).map((v) => (
            <Button
              key={v}
              variant={v === tab ? "secondary" : "ghost"}
              aria-pressed={v === tab}
              onClick={() => setTab(v)}
            >
              {t(
                v === "local"
                  ? "toolsSelect"
                  : v === "scraper"
                    ? "scraper"
                    : "healthIssues",
              )}
            </Button>
          ))}
        </fieldset>
      )}
      {adult || tab === "local" ? (
        <LocalCandidates
          initialMediaId={mediaId}
          search={search}
          onSearch={onSearch}
        />
      ) : tab === "scraper" ? (
        <Suspense fallback={<Loading />}>
          <ScraperPage mediaId={search?.media ?? mediaId} />
        </Suspense>
      ) : (
        <Maintenance />
      )}
    </>
  );
}
