import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { adultApi } from "../../api/adult";
import { defaultBrowse, libraryApi } from "../../api/library";
import {
  maintenanceApi,
  type OrganizationPlan,
  type OrganizationStrategy,
  organizationApi,
} from "../../api/organization";
import { Button } from "../../components/ui/button";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import "./translations";

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
  const client = useQueryClient();
  const api = useMemo(() => organizationApi(adult), [adult]);
  const [target, setTarget] = useState("");
  const [template, setTemplate] = useState("{title} ({year})/{stem}.{ext}");
  const [strategy, setStrategy] = useState<OrganizationStrategy>("preserve");
  const [plan, setPlan] = useState<OrganizationPlan>();
  const [rollback, setRollback] = useState(false);
  const [historyId, setHistoryId] = useState("");
  const current = useRef<OrganizationPlan | undefined>(undefined);
  const active = useRef(true);
  const requestEpoch = useRef(0);
  const prefix = adult ? ["adult", "organization"] : ["organization"];
  const roots = useQuery({
    queryKey: adult ? ["adult", "roots"] : ["roots", false],
    queryFn: () => (adult ? adultApi.roots() : libraryApi.roots(false)),
  });
  const history = useQuery({
    queryKey: [...prefix, "history"],
    queryFn: api.history,
    refetchInterval: 2500,
  });
  const journal = useQuery({
    queryKey: [...prefix, "logs", historyId],
    queryFn: () => api.logs(historyId),
    refetchInterval: 2500,
    enabled: !!historyId,
  });
  const savedPlan = useQuery({
    queryKey: [...prefix, "plan", historyId],
    queryFn: () => api.plan(historyId),
    enabled: !!historyId,
  });
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      requestEpoch.current += 1;
      const old = current.current;
      if (old) void api.discard(old.id).catch(() => {});
    };
  }, [api]);
  function replace(
    value: OrganizationPlan,
    isRollback: boolean,
    epoch: number,
  ) {
    if (!active.current || epoch !== requestEpoch.current) {
      void api.discard(value.id).catch(() => {});
      return;
    }
    const old = current.current;
    if (old) void api.discard(old.id).catch(() => {});
    current.current = value;
    setPlan(value);
    setRollback(isRollback);
  }
  const preview = useMutation({
    mutationFn: async () => {
      discard();
      const epoch = requestEpoch.current;
      const value = await api.preview({
        media_item_id: mediaId,
        target_root_id: target,
        template,
        strategy,
      });
      return { value, epoch };
    },
    onSuccess: ({ value, epoch }) => replace(value, false, epoch),
  });
  const restore = useMutation({
    mutationFn: async (id: string) => {
      discard();
      const epoch = requestEpoch.current;
      const value = await api.rollbackPreview(id);
      return { value, epoch };
    },
    onSuccess: ({ value, epoch }) => replace(value, true, epoch),
  });
  const execute = useMutation({
    mutationFn: () =>
      rollback
        ? api.rollbackExecute(plan?.id ?? "")
        : api.execute(plan?.id ?? ""),
    onSuccess: () => {
      if (!active.current) return;
      requestEpoch.current += 1;
      current.current = undefined;
      setPlan(undefined);
      void client.invalidateQueries({ queryKey: prefix });
    },
  });
  const cancel = useMutation({
    mutationFn: () => api.cancel(plan?.id ?? ""),
    onSuccess: () => {
      if (!active.current) return;
      requestEpoch.current += 1;
      current.current = undefined;
      setPlan(undefined);
      void history.refetch();
    },
  });
  function discard() {
    requestEpoch.current += 1;
    const old = current.current;
    current.current = undefined;
    setPlan(undefined);
    if (old) void api.discard(old.id).catch(() => {});
  }
  return (
    <section className="panel space-y-4">
      <h2 className="section-title">{t("organize")}</h2>
      <div className="filter-grid">
        <label className="filter-field">
          <span>{t("organizeTarget")}</span>
          <select
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
          </select>
        </label>
        <label className="filter-field">
          <span>{t("organizeTemplate")}</span>
          <input
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
        </label>
        <label className="filter-field">
          <span>{t("organizeStrategy")}</span>
          <select
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
          </select>
        </label>
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
      {execute.data && (
        <p role="status">
          {t("organizeResult")}:{" "}
          {t(`state_${execute.data.state}`, {
            defaultValue: execute.data.state,
          })}
          {execute.data.error_code ? ` · ${execute.data.error_code}` : ""}
        </p>
      )}
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
  );
}
export function LocalCandidates({
  adult = false,
  renderExtra,
}: {
  adult?: boolean;
  renderExtra?: (id: string) => React.ReactNode;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [cursors, setCursors] = useState<(string | null)[]>([null]);
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState(
    () => new URLSearchParams(window.location.search).get("media") ?? "",
  );
  const media = useQuery({
    queryKey: adult
      ? ["adult", "tools", query, cursors[page]]
      : ["tools-media", query, cursors[page]],
    queryFn: () => {
      const q = {
        ...defaultBrowse(adult),
        query: query || null,
        cursor: cursors[page] ?? null,
      };
      return adult ? adultApi.toolsMedia(q) : libraryApi.toolsMedia(q);
    },
  });
  const detail = useQuery({
    queryKey: adult
      ? ["adult", "tools-detail", selected]
      : ["tools-detail", selected],
    queryFn: () =>
      adult ? adultApi.toolsDetail(selected) : libraryApi.toolsDetail(selected),
    enabled: !!selected,
  });
  return (
    <section className="space-y-5">
      <label className="filter-field">
        <span>{t("toolsSearch")}</span>
        <input
          type="search"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setPage(0);
            setCursors([null]);
          }}
        />
      </label>
      {media.isPending ? (
        <Loading />
      ) : media.error ? (
        <ErrorNotice error={media.error} />
      ) : (
        <>
          <label className="filter-field">
            <span>{t("toolsSelect")}</span>
            <select
              value={selected}
              onChange={(e) => setSelected(e.target.value)}
            >
              <option value="">{t("toolsSelect")}</option>
              {media.data.items.map((c) => (
                <option key={c.media.id} value={c.media.id}>
                  {c.media.title}
                </option>
              ))}
            </select>
          </label>
          <div className="flex gap-3">
            <Button
              variant="outline"
              disabled={page === 0}
              onClick={() => setPage((p) => p - 1)}
            >
              {t("toolsBack")}
            </Button>
            <Button
              variant="outline"
              disabled={!media.data.next_cursor}
              onClick={() => {
                const next = media.data.next_cursor;
                if (next) {
                  setCursors((v) => [...v.slice(0, page + 1), next]);
                  setPage((p) => p + 1);
                }
              }}
            >
              {t("toolsMore")}
            </Button>
          </div>
        </>
      )}
      {selected && (
        <>
          {detail.isPending ? (
            <Loading />
          ) : detail.error ? (
            <ErrorNotice error={detail.error} />
          ) : (
            <section className="panel">
              <h2>{detail.data.media.title}</h2>
              <ul>
                {detail.data.files.map((f) => (
                  <li className="break-all" key={f.id}>
                    {f.relative_path}
                  </li>
                ))}
              </ul>
            </section>
          )}
          <OrganizationPanel key={selected} mediaId={selected} adult={adult} />
          {renderExtra?.(selected)}
        </>
      )}
    </section>
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
      <section className="panel space-y-4">
        <h2>{t("watcherTitle")}</h2>
        <p>{t("watcherHelp")}</p>
        {watchers.data?.map((w) => (
          <div key={w.root_id}>
            <label className="flex gap-3">
              <input
                type="checkbox"
                checked={w.opted_in}
                disabled={configure.isPending}
                onChange={(e) =>
                  configure.mutate({ id: w.root_id, enabled: e.target.checked })
                }
              />
              {t("watcherEnable")}:{" "}
              {roots.data?.find((r) => r.id === w.root_id)?.name ?? w.root_id}
            </label>
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
      <section className="panel space-y-4">
        <h2>{t("healthIssues")}</h2>
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
              <a
                className="action-link"
                href={`/tools?media=${encodeURIComponent(i.media_id)}`}
              >
                {t("toolsRepair")}
              </a>
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
    </>
  );
}
export function MediaToolsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<"local" | "scraper" | "health">("local");
  return (
    <>
      <PageHeading title={t("videoTools")}>
        <Link to="/library" className="action-link">
          {t("library")}
        </Link>
      </PageHeading>
      <div className="flex gap-3 mb-6">
        {(["local", "scraper", "health"] as const).map((v) => (
          <Button
            key={v}
            variant={v === tab ? "default" : "outline"}
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
      </div>
      {tab === "local" ? (
        <LocalCandidates />
      ) : tab === "scraper" ? (
        <Suspense fallback={<Loading />}>
          <ScraperPage />
        </Suspense>
      ) : (
        <Maintenance />
      )}
    </>
  );
}
