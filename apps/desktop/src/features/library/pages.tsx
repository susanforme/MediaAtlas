import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { type FormEvent, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  defaultBrowse,
  libraryApi,
  type MediaKind,
  type RootInput,
  type RootView,
} from "../../api/library";
import { Button } from "../../components/ui/button";
import { usePrivacy } from "./context";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  ROOT_KINDS,
  useFormat,
} from "./shared";

export function DashboardPage() {
  const { t } = useTranslation();
  const { number } = useFormat();
  // Dashboard is always ordinary media, even while the separate adult section is enabled.
  const summary = useQuery({
    queryKey: ["summary", false],
    queryFn: () => libraryApi.summary(defaultBrowse(false)),
  });
  return (
    <>
      <PageHeading title={t("dashboard")} description={t("firstSteps")}>
        <Link to="/roots" className="action-link">
          {t("addRoot")}
        </Link>
      </PageHeading>
      {summary.isPending ? (
        <Loading />
      ) : summary.isError ? (
        <ErrorNotice
          error={summary.error}
          retry={() => void summary.refetch()}
        />
      ) : (
        <>
          <div className="stats-grid">
            {(
              [
                ["totalMedia", summary.data.total_media],
                ["totalRoots", summary.data.total_roots],
                ["pendingTasks", summary.data.pending_tasks],
                ["failedTasks", summary.data.failed_tasks],
              ] as const
            ).map(([label, value]) => (
              <div className="panel" key={label}>
                <p className="text-sm text-muted-foreground">{t(label)}</p>
                <p className="mt-3 text-3xl font-semibold">{number(value)}</p>
              </div>
            ))}
          </div>
          <section className="panel mt-6">
            <h2 className="section-title">{t("healthOverview")}</h2>
            <div className="flex flex-wrap gap-6">
              {(
                [
                  ["missingPoster", summary.data.missing_poster],
                  ["missingNfo", summary.data.missing_nfo],
                  ["incompleteMetadata", summary.data.incomplete_metadata],
                ] as const
              ).map(([label, value]) => (
                <p key={label}>
                  {t(label)} <strong>{number(value)}</strong>
                </p>
              ))}
            </div>
          </section>
          <Link className="action-link mt-6 inline-block" to="/library">
            {t("browseLibrary")}
          </Link>
        </>
      )}
    </>
  );
}

const newRoot = (): RootInput => ({
  id: null,
  name: "",
  path: "",
  kind: "movie",
  enabled: true,
  exclude_patterns: [],
});
export function RootsPage() {
  const { t } = useTranslation();
  const { date } = useFormat();
  const { includeAdult } = usePrivacy();
  const client = useQueryClient();
  const [form, setForm] = useState<RootInput | null>(null);
  const [excludes, setExcludes] = useState("");
  const [remove, setRemove] = useState<RootView | null>(null);
  const [validation, setValidation] = useState(false);
  const roots = useQuery({
    queryKey: ["roots", includeAdult],
    queryFn: () => libraryApi.roots(includeAdult),
  });
  const save = useMutation({
    mutationFn: (input: RootInput) => libraryApi.saveRoot(input),
    onSuccess: () => {
      setForm(null);
      void client.invalidateQueries();
    },
  });
  const deletion = useMutation({
    mutationFn: (id: string) => libraryApi.removeRoot(id),
    onSuccess: () => {
      setRemove(null);
      void client.invalidateQueries();
    },
  });
  function edit(root?: RootView) {
    setValidation(false);
    save.reset();
    const value = root
      ? {
          id: root.id,
          name: root.name,
          path: root.path,
          kind: root.kind,
          enabled: root.enabled,
          exclude_patterns: root.exclude_patterns,
        }
      : newRoot();
    setForm(value);
    setExcludes(value.exclude_patterns.join("\n"));
  }
  function submit(event: FormEvent) {
    event.preventDefault();
    if (!form?.name.trim() || !form.path.trim()) {
      setValidation(true);
      return;
    }
    save.mutate({
      ...form,
      exclude_patterns: excludes
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean),
    });
  }
  return (
    <>
      <PageHeading title={t("roots")} description={t("pathHelp")}>
        <Button onClick={() => edit()}>{t("addRoot")}</Button>
      </PageHeading>
      {form && (
        <form className="panel mb-6 space-y-4" onSubmit={submit} noValidate>
          <h2 className="section-title">{t(form.id ? "edit" : "addRoot")}</h2>
          <label className="filter-field">
            <span>{t("rootName")}</span>
            <input
              value={form.name}
              onChange={(event) =>
                setForm({ ...form, name: event.target.value })
              }
              required
            />
          </label>
          <label className="filter-field">
            <span>{t("rootPath")}</span>
            <input
              value={form.path}
              onChange={(event) =>
                setForm({ ...form, path: event.target.value })
              }
              required
            />
          </label>
          <label className="filter-field">
            <span>{t("rootType")}</span>
            <select
              value={form.kind ?? "mixed"}
              onChange={(event) =>
                setForm({
                  ...form,
                  kind:
                    event.target.value === "mixed"
                      ? null
                      : (event.target.value as MediaKind),
                })
              }
            >
              {ROOT_KINDS.filter(
                (kind) => includeAdult || kind !== "adult_video",
              ).map((kind) => (
                <option value={kind} key={kind}>
                  {t(`kind_${kind}`)}
                </option>
              ))}
              <option value="mixed">{t("mixed")}</option>
            </select>
          </label>
          <label className="filter-field">
            <span>{t("excludePatterns")}</span>
            <textarea
              value={excludes}
              onChange={(event) => setExcludes(event.target.value)}
              placeholder={t("excludeHelp")}
              rows={3}
            />
          </label>
          <label className="flex gap-2">
            <input
              type="checkbox"
              checked={form.enabled}
              onChange={(event) =>
                setForm({ ...form, enabled: event.target.checked })
              }
            />
            {t("enabled")}
          </label>
          {validation && <p role="alert">{t("invalidRoot")}</p>}
          {save.isError && <ErrorNotice error={save.error} />}
          <div className="flex gap-3">
            <Button type="submit" disabled={save.isPending}>
              {t(save.isPending ? "saving" : "save")}
            </Button>
            <Button variant="outline" onClick={() => setForm(null)}>
              {t("cancel")}
            </Button>
          </div>
        </form>
      )}
      {remove && (
        <section
          className="panel mb-6"
          role="dialog"
          aria-label={t("removeRoot")}
        >
          <p>{t("removeConfirm", { name: remove.name })}</p>
          {deletion.isError && <ErrorNotice error={deletion.error} />}
          <div className="mt-4 flex gap-3">
            <Button
              disabled={deletion.isPending}
              onClick={() => deletion.mutate(remove.id)}
            >
              {t("confirmRemove")}
            </Button>
            <Button variant="outline" onClick={() => setRemove(null)}>
              {t("cancel")}
            </Button>
          </div>
        </section>
      )}
      {roots.isPending ? (
        <Loading />
      ) : roots.isError ? (
        <ErrorNotice error={roots.error} retry={() => void roots.refetch()} />
      ) : !roots.data.length ? (
        <Empty title={t("noRoots")} body={t("firstSteps")} />
      ) : (
        <div className="space-y-4">
          {roots.data.map((root) => (
            <article className="panel" key={root.id}>
              <div className="flex flex-wrap justify-between gap-4">
                <div>
                  <h2 className="font-semibold">
                    {root.name}{" "}
                    <span className="badge">
                      {t(root.enabled ? "enabled" : "disabled")}
                    </span>
                  </h2>
                  <p className="mt-2 break-all text-sm">{root.path}</p>
                  <p className="mt-2 text-xs text-muted-foreground">
                    {root.kind ? t(`kind_${root.kind}`) : t("mixed")} ·{" "}
                    {t("lastScan")}:{" "}
                    {root.last_scan_at
                      ? date(root.last_scan_at)
                      : t("neverScanned")}
                  </p>
                </div>
                <div className="flex flex-wrap gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => edit(root)}
                  >
                    {t("edit")}
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => {
                      deletion.reset();
                      setRemove(root);
                    }}
                  >
                    {t("remove")}
                  </Button>
                  <Link
                    className="action-link"
                    to="/scanner"
                    search={{ root: root.id }}
                  >
                    {t("scanRoot")}
                  </Link>
                </div>
              </div>
            </article>
          ))}
        </div>
      )}
    </>
  );
}

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const { includeAdult, setIncludeAdult } = usePrivacy();
  const navigate = useNavigate();
  return (
    <>
      <PageHeading title={t("settings")} />
      <div className="space-y-6">
        <section className="panel">
          <h2 className="section-title">{t("languageSettings")}</h2>
          <p className="mb-4 text-sm text-muted-foreground">
            {t("languageHelp")}
          </p>
          <label className="filter-field">
            <span>{t("language")}</span>
            <select
              value={i18n.resolvedLanguage ?? "zh-CN"}
              onChange={(event) => void i18n.changeLanguage(event.target.value)}
            >
              <option value="zh-CN">简体中文</option>
              <option value="en-US">English (US)</option>
            </select>
          </label>
        </section>
        <section className="panel">
          <h2 className="section-title">{t("adultSettings")}</h2>
          <p className="mb-4 text-sm text-muted-foreground">{t("adultHelp")}</p>
          <label className="flex items-center gap-3">
            <input
              type="checkbox"
              checked={includeAdult}
              onChange={(event) => {
                setIncludeAdult(event.target.checked);
                if (!event.target.checked) void navigate({ to: "/" });
              }}
            />
            {t("adultEnabled")}
          </label>
        </section>
      </div>
    </>
  );
}
