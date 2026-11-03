import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import {
  type FormEvent,
  type ReactNode,
  useEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import {
  type AdultBatchPreview,
  type AdultBatchResult,
  type AdultDetail,
  type AdultMergePreview,
  type AdultOutputPreview,
  type AdultSearchPreview,
  adultApi,
} from "../../api/adult";
import { defaultBrowse } from "../../api/library";
import { providerApi } from "../../api/providers";
import { Button } from "../../components/ui/button";
import { usePrivacy } from "../library/context";
import { RootsPage } from "../library/pages";
import { ScannerPage, type ScannerSearch } from "../library/scanner";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  Pager,
  useFormat,
} from "../library/shared";
import "./translations";
import "../scraper/navigation";

export function AdultGate({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { settings, includeAdult, loading, error, unlock } = usePrivacy();
  if (loading) return <Loading />;
  if (!settings.entry_visible) return <Empty title={t("privacyHidden")} />;
  if (!includeAdult)
    return (
      <section className="panel">
        <h1 className="section-title">{t("adultLocked")}</h1>
        <p className="mb-4">{t("adultGateHelp")}</p>
        <Button onClick={() => void unlock()}>{t("adultUnlock")}</Button>
        {error !== undefined && <ErrorNotice error={error} />}
      </section>
    );
  return children;
}
export function AdultPage() {
  return (
    <AdultGate>
      <AdultWorkspace />
    </AdultGate>
  );
}
function AdultWorkspace() {
  const { t } = useTranslation();
  const { lock } = usePrivacy();
  const [tab, setTab] = useState<
    "library" | "scraper" | "roots" | "scanner" | "tasks"
  >("library");
  const [scanSearch, setScanSearch] = useState<ScannerSearch>({});
  return (
    <>
      <PageHeading
        title={t("adultWorkspace")}
        description={t("adultSettingsHelp")}
      >
        <Button variant="outline" onClick={() => void lock()}>
          {t("adultLock")}
        </Button>
      </PageHeading>
      <div className="mb-6 flex flex-wrap gap-3">
        {(["library", "scraper", "roots", "scanner", "tasks"] as const).map(
          (value) => (
            <Button
              key={value}
              variant={tab === value ? "default" : "outline"}
              onClick={() => setTab(value)}
            >
              {t(
                value === "library"
                  ? "adultLibrary"
                  : value === "scraper"
                    ? "scraper"
                    : value === "roots"
                      ? "roots"
                      : value === "scanner"
                        ? "scanner"
                        : "tasks",
              )}
            </Button>
          ),
        )}
      </div>
      {tab === "library" ? (
        <AdultLibrary />
      ) : tab === "scraper" ? (
        <AdultScraper />
      ) : tab === "roots" ? (
        <RootsPage
          adult
          onScan={(rootId) => {
            setScanSearch({ root: rootId });
            setTab("scanner");
          }}
        />
      ) : tab === "scanner" ? (
        <ScannerPage adult search={scanSearch} onSearch={setScanSearch} />
      ) : (
        <AdultTasks />
      )}
    </>
  );
}
export function AdultMetadata({ detail }: { detail: AdultDetail }) {
  const { t } = useTranslation();
  const c = detail.identity.catalog_number;
  return (
    <section className="panel">
      <h2 className="section-title">
        {c.normalized} · {detail.identity.title}
        {c.is_primary && (
          <span className="badge ml-2">{t("adultPrimaryCatalog")}</span>
        )}
      </h2>
      <dl className="space-y-2 text-sm">
        {[
          ["adultCatalogRaw", c.raw],
          ["adultNamespace", c.namespace],
          ["adultPart", c.part ? `${c.part.kind} ${c.part.number}` : null],
          ["adultStudio", detail.identity.studio],
          ["adultLabel", detail.label],
          ["adultSeries", detail.series],
          ["adultRelease", detail.release_date],
          ["adultRuntime", detail.runtime_minutes],
          ["adultSource", detail.identity.provider_id],
        ].map(([key, value]) => (
          <div key={String(key)}>
            <dt className="font-medium">{t(String(key))}</dt>
            <dd>{value ?? t("none")}</dd>
          </div>
        ))}
      </dl>
      {c.aliases.length > 0 && (
        <p className="mt-3 text-sm">
          {t("adultCatalogAliases")}: {c.aliases.join(" / ")}
        </p>
      )}
      {(["performers", "directors"] as const).map((group) => (
        <div key={group} className="mt-4">
          <h3 className="font-medium">
            {t(group === "performers" ? "adultPerformers" : "adultDirectors")}
          </h3>
          <ul>
            {detail[group].map((person) => (
              <li
                key={`${person.provider_id}:${person.remote_id}`}
                className="py-1 text-sm"
              >
                {person.name}{" "}
                <span className="text-muted-foreground">
                  {person.provider_id} / {person.remote_id}
                  {person.aliases.length
                    ? ` · ${person.aliases.join(" / ")}`
                    : ""}
                </span>
              </li>
            ))}
          </ul>
        </div>
      ))}
      <p className="mt-3 text-xs text-muted-foreground">
        {t("adultPeopleHelp")}
      </p>
    </section>
  );
}
export function OutputDiff({ preview }: { preview: AdultOutputPreview }) {
  const { t } = useTranslation();
  return (
    <section className="panel">
      <h3 className="section-title">{t("adultOutputFiles")}</h3>
      <p className="notice mb-3">{t("adultPreserve")}</p>
      {preview.files.map((file) => (
        <div key={file.destination} className="border-t border-border py-3">
          <p className="break-all font-medium">
            {file.destination} · {file.kind}
          </p>
          {file.protected && (
            <span className="badge">{t("adultFieldProtected")}</span>
          )}
          {(file.old_text !== null || file.new_text !== null) && (
            <div className="mt-2 grid gap-3 md:grid-cols-2">
              <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">
                {file.old_text ?? "∅"}
              </pre>
              <pre className="max-h-64 overflow-auto whitespace-pre-wrap break-all text-xs">
                {file.new_text ?? "∅"}
              </pre>
            </div>
          )}
        </div>
      ))}
    </section>
  );
}
export function AdultOutput({ mediaId }: { mediaId: string }) {
  const { t } = useTranslation();
  const [preview, setPreview] = useState<AdultOutputPreview>();
  const current = useRef<AdultOutputPreview | undefined>(undefined);
  const active = useRef(true);
  useEffect(() => {
    current.current = preview;
  }, [preview]);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      const old = current.current;
      if (old) void adultApi.outputDiscard(old.preview_id).catch(() => {});
    };
  }, []);
  const plan = useMutation({
    mutationFn: () => adultApi.outputPreview(mediaId),
    onSuccess: (value) => {
      if (!active.current) {
        void adultApi.outputDiscard(value.preview_id).catch(() => {});
        return;
      }
      const old = current.current;
      if (old) void adultApi.outputDiscard(old.preview_id).catch(() => {});
      current.current = value;
      setPreview(value);
    },
  });
  const execute = useMutation({
    mutationFn: () => adultApi.outputExecute(preview?.preview_id ?? ""),
    onSuccess: () => setPreview(undefined),
  });
  return (
    <div className="space-y-4">
      <Button
        variant="outline"
        onClick={() => {
          execute.reset();
          plan.mutate();
        }}
        disabled={plan.isPending || execute.isPending}
      >
        {t("adultOutputPreview")}
      </Button>
      {plan.error && <ErrorNotice error={plan.error} />}{" "}
      {preview && (
        <>
          <OutputDiff preview={preview} />
          <Button onClick={() => execute.mutate()} disabled={execute.isPending}>
            {t("adultOutputExecute")}
          </Button>
        </>
      )}
      {execute.error && <ErrorNotice error={execute.error} />}{" "}
      {execute.isSuccess && <p role="status">{t("adultOutputDone")}</p>}
    </div>
  );
}
function AdultLibrary() {
  const { t } = useTranslation();
  const { settings } = usePrivacy();
  const [query, setQuery] = useState("");
  const [cursors, setCursors] = useState<(string | null)[]>([null]);
  const [page, setPage] = useState(0);
  const [selected, setSelected] = useState<string[]>([]);
  const [batch, setBatch] = useState<AdultBatchPreview>();
  const [result, setResult] = useState<AdultBatchResult>();
  const batchRef = useRef<AdultBatchPreview | undefined>(undefined);
  const active = useRef(true);
  useEffect(() => {
    batchRef.current = batch;
  }, [batch]);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      if (batchRef.current)
        void adultApi.batchCancel(batchRef.current.batch_id).catch(() => {});
    };
  }, []);
  const media = useQuery({
    queryKey: [
      "adult",
      "library",
      query,
      cursors[page],
      settings.covers_visible,
    ],
    queryFn: () =>
      adultApi.media({
        ...defaultBrowse(true),
        kind: "adult_video",
        query: query || null,
        cursor: cursors[page] ?? null,
      }),
  });
  const plan = useMutation({
    mutationFn: () => adultApi.batchPreview(selected),
    onSuccess: (value) => {
      if (!active.current) {
        void adultApi.batchCancel(value.batch_id).catch(() => {});
        return;
      }
      if (batchRef.current)
        void adultApi.batchCancel(batchRef.current.batch_id).catch(() => {});
      batchRef.current = value;
      setBatch(value);
      setResult(undefined);
    },
  });
  const execute = useMutation({
    mutationFn: () => adultApi.batchExecute(batch?.batch_id ?? ""),
    onSuccess: (value) => {
      setResult(value);
      setBatch(undefined);
    },
  });
  const cancel = useMutation({
    mutationFn: () => adultApi.batchCancel(batch?.batch_id ?? ""),
    onSuccess: (value) => {
      setResult(value);
      setBatch(undefined);
    },
  });
  return (
    <div className="space-y-5">
      <AdultRecent />
      <label className="filter-field">
        <span>{t("adultLocalSearch")}</span>
        <input
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setPage(0);
            setCursors([null]);
          }}
        />
      </label>
      {media.isPending ? (
        <Loading />
      ) : media.error ? (
        <ErrorNotice error={media.error} />
      ) : !media.data.items.length ? (
        <Empty title={t("emptyLibrary")} />
      ) : (
        <ul className="space-y-3">
          {media.data.items.map(({ media: item, poster_uri }) => (
            <li
              className="panel flex flex-wrap items-center gap-4"
              key={item.id}
            >
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  aria-label={`${t("adultBatchSelect")}: ${item.title}`}
                  checked={selected.includes(item.id)}
                  disabled={!!batch || execute.isPending}
                  onChange={(event) =>
                    setSelected((values) =>
                      event.target.checked
                        ? [...values, item.id].slice(0, 32)
                        : values.filter((v) => v !== item.id),
                    )
                  }
                />
                {t("adultBatchSelect")}
              </label>
              {settings.covers_visible &&
                poster_uri?.startsWith("data:image/") && (
                  <img src={poster_uri} alt="" className="max-h-24 max-w-16" />
                )}
              <Link
                className="text-primary"
                to="/adult/media/$mediaId"
                params={{ mediaId: item.id }}
              >
                {item.title}
              </Link>
            </li>
          ))}
        </ul>
      )}
      {media.data && (
        <Pager
          page={page + 1}
          hasNext={Boolean(media.data.next_cursor)}
          previous={() => setPage((v) => Math.max(0, v - 1))}
          next={() => {
            const next = media.data.next_cursor;
            if (next) {
              setCursors((v) => [...v.slice(0, page + 1), next]);
              setPage((v) => v + 1);
            }
          }}
        />
      )}
      <Button
        disabled={
          !selected.length || plan.isPending || execute.isPending || !!batch
        }
        onClick={() => plan.mutate()}
      >
        {t("adultBatchPreview")}
      </Button>
      {plan.error && <ErrorNotice error={plan.error} />}{" "}
      {batch && (
        <section className="space-y-4">
          {batch.items.map((item) => (
            <OutputDiff key={item.preview_id} preview={item} />
          ))}
          <div className="flex gap-3">
            <Button
              onClick={() => execute.mutate()}
              disabled={execute.isPending || cancel.isPending}
            >
              {t("adultBatchConfirm")}
            </Button>
            <Button
              variant="outline"
              onClick={() => cancel.mutate()}
              disabled={cancel.isPending}
            >
              {t("adultBatchCancel")}
            </Button>
          </div>
        </section>
      )}
      {execute.error && <ErrorNotice error={execute.error} />}{" "}
      {cancel.error && <ErrorNotice error={cancel.error} />}{" "}
      {result && (
        <section className="panel" role="status">
          <h2>
            {t("adultBatchResult")}:{" "}
            {t(`state_${result.state}`, { defaultValue: result.state })}
          </h2>
          <ul>
            {result.items.map((item) => (
              <li key={item.preview_id}>
                {item.media_item_id}:{" "}
                {t(`state_${item.state}`, { defaultValue: item.state })}
                {item.error_code &&
                  ` · ${t(`providerFailure_${item.error_code}`, { defaultValue: item.error_code })}`}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
export function AdultScraper() {
  const { t, i18n } = useTranslation();
  const client = useQueryClient();
  const { settings } = usePrivacy();
  const [catalog, setCatalog] = useState("");
  const [namespace, setNamespace] = useState("dmm");
  const [studio, setStudio] = useState("");
  const [selectedProviders, setSelectedProviders] = useState<string[]>([]);
  const [search, setSearch] = useState<AdultSearchPreview>();
  const [detail, setDetail] = useState<AdultDetail>();
  const [candidateIndex, setCandidateIndex] = useState<number>();
  const [target, setTarget] = useState("");
  const [fields, setFields] = useState<string[]>([
    "title",
    "catalog_numbers",
    "studio",
    "label",
    "series",
    "release_date",
    "runtime_minutes",
    "performers",
    "directors",
  ]);
  const [images, setImages] = useState<number[]>([]);
  const [merge, setMerge] = useState<AdultMergePreview>();
  const [thumbnail, setThumbnail] = useState<string>();
  const providers = useQuery({
    queryKey: ["adult", "providers"],
    queryFn: adultApi.providers,
  });
  const local = useQuery({
    queryKey: ["adult", "targets"],
    queryFn: () =>
      adultApi.media({ ...defaultBrowse(true), kind: "adult_video" }),
  });
  const enabled =
    providers.data?.providers
      .filter((p) => p.config.enabled)
      .map((p) => p.descriptor.id) ?? [];
  const activeProviders = selectedProviders.filter((p) => enabled.includes(p));
  const enable = useMutation({
    mutationFn: ({ id, value }: { id: string; value: boolean }) =>
      providerApi.setEnabled(id, value),
    onSuccess: () =>
      void client.invalidateQueries({ queryKey: ["adult", "providers"] }),
  });
  const searching = useMutation({
    mutationFn: () =>
      adultApi.search(
        {
          catalog_number: catalog,
          namespace,
          studio: studio.trim() || null,
          language: i18n.resolvedLanguage ?? "zh-CN",
        },
        activeProviders,
      ),
    onSuccess: (value) => {
      setSearch(value);
      setDetail(undefined);
      setMerge(undefined);
      setCandidateIndex(undefined);
      setImages([]);
      setThumbnail(undefined);
    },
  });
  const loadingDetail = useMutation({
    mutationFn: (index: number) => adultApi.detail(search?.run_id ?? "", index),
    onSuccess: (value, index) => {
      setDetail(value);
      setCandidateIndex(index);
      setMerge(undefined);
      setImages([]);
      setThumbnail(undefined);
    },
  });
  const preview = useMutation({
    mutationFn: () =>
      adultApi.preview({
        run_id: search?.run_id ?? "",
        candidate_index: candidateIndex ?? 0,
        media_item_id: target,
        selected_fields: fields,
        selected_artwork: images,
      }),
    onSuccess: setMerge,
  });
  const confirm = useMutation({
    mutationFn: () => adultApi.confirm(merge?.preview_id ?? ""),
    onSuccess: () => {
      setMerge(undefined);
      void client.invalidateQueries({ queryKey: ["adult"] });
    },
  });
  const image = useMutation({
    mutationFn: (index: number) =>
      adultApi.artworkPreview(search?.run_id ?? "", candidateIndex ?? 0, index),
    onSuccess: (value) => setThumbnail(value.thumbnail),
  });
  function submit(event: FormEvent) {
    event.preventDefault();
    confirm.reset();
    searching.mutate();
  }
  return (
    <div className="space-y-5">
      <p className="notice">{t("adultLivePending")}</p>
      <section className="panel">
        <h2 className="section-title">{t("adultProviders")}</h2>
        <Link className="action-link mb-4 inline-block" to="/settings">
          {t("providerAuthOpenSettings")}
        </Link>
        {providers.isPending ? (
          <Loading />
        ) : providers.error ? (
          <ErrorNotice error={providers.error} />
        ) : (
          providers.data.providers.map((p) => (
            <div key={p.descriptor.id} className="mb-4">
              <div className="flex flex-wrap items-center gap-3">
                <label className="flex gap-2">
                  <input
                    type="checkbox"
                    checked={selectedProviders.includes(p.descriptor.id)}
                    disabled={!p.config.enabled}
                    onChange={(event) =>
                      setSelectedProviders((ids) =>
                        event.target.checked
                          ? [...ids, p.descriptor.id]
                          : ids.filter((id) => id !== p.descriptor.id),
                      )
                    }
                  />
                  {p.descriptor.name}
                </label>
                <span className="badge">
                  {t(`providerHealth_${p.health}`, { defaultValue: p.health })}
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={enable.isPending}
                  onClick={() =>
                    enable.mutate({
                      id: p.descriptor.id,
                      value: !p.config.enabled,
                    })
                  }
                >
                  {t(
                    p.config.enabled
                      ? "adultProviderDisable"
                      : "adultProviderEnable",
                  )}
                </Button>
              </div>
              <p className="mt-2 text-xs text-muted-foreground">
                {p.descriptor.attribution} · {p.descriptor.rate_limit_policy}
              </p>
            </div>
          ))
        )}
        {enable.error && <ErrorNotice error={enable.error} />}
      </section>
      <form className="panel space-y-4" onSubmit={submit}>
        <label className="filter-field">
          <span>{t("adultCatalog")}</span>
          <input
            required
            value={catalog}
            onChange={(event) => setCatalog(event.target.value)}
            maxLength={128}
          />
        </label>
        <label className="filter-field">
          <span>{t("adultNamespace")}</span>
          <input
            required
            value={namespace}
            onChange={(event) => setNamespace(event.target.value)}
            maxLength={64}
          />
        </label>
        <label className="filter-field">
          <span>{t("adultStudio")}</span>
          <input
            value={studio}
            onChange={(event) => setStudio(event.target.value)}
            maxLength={256}
          />
        </label>
        <Button
          type="submit"
          disabled={
            !catalog.trim() ||
            !namespace.trim() ||
            !activeProviders.length ||
            searching.isPending
          }
        >
          {t("adultSearch")}
        </Button>
        {!enabled.length && <p>{t("adultNoProvider")}</p>}
      </form>
      {searching.error && <ErrorNotice error={searching.error} />}{" "}
      {search && (
        <section className="panel">
          <h2 className="section-title">{t("candidates")}</h2>
          {search.failures.map((f) => (
            <p className="notice" key={f.provider_id}>
              {f.provider_id}: {t(`providerFailure_${f.failure}`)}
            </p>
          ))}
          {!search.candidates.length && <p>{t("adultNoCandidates")}</p>}
          {search.candidates.map((entry, index) => (
            <article
              className="border-t border-border py-4"
              key={`${entry.candidate.provider_id}:${entry.candidate.remote_id}`}
            >
              <p className="font-semibold">
                {entry.candidate.catalog_number.normalized} ·{" "}
                {entry.candidate.title}
              </p>
              <p className="text-xs">
                {entry.candidate.catalog_number.namespace} ·{" "}
                {entry.candidate.studio ?? t("none")} ·{" "}
                {entry.candidate.provider_id}
              </p>
              {entry.needs_review && (
                <span className="badge">{t("adultReview")}</span>
              )}
              {entry.duplicate_of !== null && (
                <span className="badge">{t("adultDuplicate")}</span>
              )}
              <ul className="my-2 text-sm">
                {entry.reasons.map((reason) => (
                  <li key={reason}>
                    {t(`adultReason_${reason}`, {
                      defaultValue: t("adultNeedsReview"),
                    })}
                  </li>
                ))}
              </ul>
              <Button
                variant="outline"
                disabled={loadingDetail.isPending}
                onClick={() => loadingDetail.mutate(index)}
              >
                {t("adultChoose")}
              </Button>
            </article>
          ))}
        </section>
      )}
      {loadingDetail.error && <ErrorNotice error={loadingDetail.error} />}{" "}
      {detail && (
        <>
          <AdultMetadata detail={detail} />
          <section className="panel space-y-4">
            <label className="filter-field">
              <span>{t("adultTarget")}</span>
              <select
                value={target}
                onChange={(event) => {
                  setTarget(event.target.value);
                  setMerge(undefined);
                }}
              >
                <option value="">{t("adultTarget")}</option>
                {local.data?.items.map((item) => (
                  <option key={item.media.id} value={item.media.id}>
                    {item.media.title}
                  </option>
                ))}
              </select>
            </label>
            {local.error && <ErrorNotice error={local.error} />}
            <fieldset>
              <legend className="mb-3 font-medium">{t("adultFields")}</legend>
              <div className="flex flex-wrap gap-4">
                {[
                  "title",
                  "catalog_numbers",
                  "studio",
                  "label",
                  "series",
                  "release_date",
                  "runtime_minutes",
                  "performers",
                  "directors",
                ].map((key) => (
                  <label className="flex gap-2" key={key}>
                    <input
                      type="checkbox"
                      checked={fields.includes(key)}
                      onChange={(event) => {
                        setMerge(undefined);
                        setFields((values) =>
                          event.target.checked
                            ? [...values, key]
                            : values.filter((v) => v !== key),
                        );
                      }}
                    />
                    {fieldLabel(key, t)}
                  </label>
                ))}
              </div>
            </fieldset>
            <fieldset>
              <legend className="font-medium">{t("adultImages")}</legend>
              {!settings.covers_visible ? (
                <p>{t("adultHiddenImages")}</p>
              ) : (
                detail.artwork.map((art, index) => (
                  <div
                    key={`${art.kind}:${art.url}`}
                    className="mt-3 flex gap-3"
                  >
                    <label className="flex gap-2">
                      <input
                        type="checkbox"
                        checked={images.includes(index)}
                        onChange={(event) => {
                          setMerge(undefined);
                          setImages((values) =>
                            event.target.checked
                              ? [...values, index]
                              : values.filter((v) => v !== index),
                          );
                        }}
                      />
                      {t(`adultArtwork_${art.kind}`, {
                        defaultValue: t("artworks"),
                      })}{" "}
                      {index + 1}
                    </label>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={image.isPending}
                      onClick={() => image.mutate(index)}
                    >
                      {t("adultImagePreview")}
                    </Button>
                  </div>
                ))
              )}
              {settings.covers_visible && thumbnail && (
                <img
                  src={thumbnail}
                  alt={t("adultImages")}
                  className="mt-3 max-h-64"
                />
              )}
              {image.error && <ErrorNotice error={image.error} />}
            </fieldset>
            <Button
              onClick={() => {
                confirm.reset();
                preview.mutate();
              }}
              disabled={
                !target ||
                (!fields.length && !images.length) ||
                preview.isPending
              }
            >
              {t("adultMergePreview")}
            </Button>
          </section>
          {preview.error && <ErrorNotice error={preview.error} />}{" "}
          {merge && (
            <section className="panel space-y-4">
              <h2 className="section-title">{t("adultMergePreview")}</h2>
              <p>{t("adultNeedsReview")}</p>
              {merge.fields.map((field) => (
                <div
                  className="border-t border-border py-3"
                  key={`${field.key}:${field.language}`}
                >
                  <h3>
                    {fieldLabel(field.key, t)} · {field.provider_id}
                    {field.language ? ` · ${field.language}` : ""}
                    {field.protected && (
                      <span className="badge">{t("adultFieldProtected")}</span>
                    )}
                  </h3>
                  <div className="grid gap-3 text-sm md:grid-cols-2">
                    <pre className="overflow-auto whitespace-pre-wrap">
                      {JSON.stringify(field.old?.value ?? null)}
                    </pre>
                    <pre className="overflow-auto whitespace-pre-wrap">
                      {JSON.stringify(field.new.value)}
                    </pre>
                  </div>
                </div>
              ))}
              <Button
                disabled={confirm.isPending}
                onClick={() => confirm.mutate()}
              >
                {t("adultMergeConfirm")}
              </Button>
            </section>
          )}
          {confirm.error && <ErrorNotice error={confirm.error} />}{" "}
          {confirm.isSuccess && <p role="status">{t("adultMerged")}</p>}
          {target && <AdultOutput key={target} mediaId={target} />}
        </>
      )}
    </div>
  );
}
function fieldLabel(key: string, t: (key: string) => string) {
  const keys: Record<string, string> = {
    title: "titleLabel",
    catalog_numbers: "adultCatalog",
    studio: "adultStudio",
    label: "adultLabel",
    series: "adultSeries",
    release_date: "adultRelease",
    runtime_minutes: "adultRuntime",
    performers: "adultPerformers",
    directors: "adultDirectors",
  };
  return t(keys[key] ?? "metadata");
}
export function AdultMediaPage({ mediaId }: { mediaId: string }) {
  return (
    <AdultGate>
      <AdultMediaDetail mediaId={mediaId} />
    </AdultGate>
  );
}
function AdultMediaDetail({ mediaId }: { mediaId: string }) {
  const { t } = useTranslation();
  const { settings } = usePrivacy();
  const local = useQuery({
    queryKey: ["adult", "media", mediaId],
    queryFn: () => adultApi.getMedia(mediaId),
  });
  const source = useQuery({
    queryKey: ["adult", "source", mediaId],
    queryFn: () => adultApi.mediaDetail(mediaId),
  });
  return (
    <div className="space-y-5">
      <Link to="/adult" className="action-link">
        ← {t("adultBack")}
      </Link>
      {local.isPending ? (
        <Loading />
      ) : local.error ? (
        <ErrorNotice error={local.error} />
      ) : (
        <>
          <PageHeading title={local.data.media.title} />
          {settings.covers_visible &&
            local.data.poster_uri?.startsWith("data:image/") && (
              <img src={local.data.poster_uri} alt="" className="max-h-64" />
            )}
          <section className="panel">
            <h2 className="section-title">{t("files")}</h2>
            <ul>
              {local.data.files.map((file) => (
                <li key={file.id} className="break-all">
                  {file.relative_path}
                </li>
              ))}
            </ul>
          </section>
        </>
      )}
      {source.isPending ? (
        <Loading />
      ) : source.error ? (
        <ErrorNotice error={source.error} />
      ) : source.data ? (
        <AdultMetadata detail={source.data} />
      ) : (
        <Empty title={t("adultNoMetadata")} />
      )}
      {local.data && <AdultOutput mediaId={mediaId} />}
      <p className="text-xs text-muted-foreground">
        {t(
          settings.remember_recent
            ? "adultRecentEnabled"
            : "adultRecentDisabled",
        )}
      </p>
    </div>
  );
}

function AdultRecent() {
  const { t } = useTranslation();
  const { settings } = usePrivacy();
  const { date } = useFormat();
  const visits = useQuery({
    queryKey: ["adult", "recent"],
    queryFn: adultApi.recent,
    enabled: settings.remember_recent,
  });
  if (!settings.remember_recent)
    return (
      <p className="text-xs text-muted-foreground">
        {t("adultRecentDisabled")}
      </p>
    );
  return (
    <section className="panel">
      <h2 className="section-title">{t("adultRecentTitle")}</h2>
      {visits.error ? (
        <ErrorNotice error={visits.error} />
      ) : (
        <ul>
          {visits.data?.map((visit) => (
            <li key={visit.media_id} className="py-1">
              <Link
                to="/adult/media/$mediaId"
                params={{ mediaId: visit.media_id }}
                className="text-primary"
              >
                {visit.title}
              </Link>
              <span className="ml-3 text-xs text-muted-foreground">
                {date(visit.visited_at)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function AdultTasks() {
  const { t } = useTranslation();
  const client = useQueryClient();
  const snapshot = useQuery({
    queryKey: ["adult", "providers"],
    queryFn: adultApi.providers,
    refetchInterval: 2000,
  });
  const cancel = useMutation({
    mutationFn: (taskId: string) => providerApi.cancel(taskId),
    onSuccess: () => void client.invalidateQueries({ queryKey: ["adult"] }),
  });
  return (
    <section className="panel">
      <h2 className="section-title">{t("adultTaskTitle")}</h2>
      {snapshot.isPending ? (
        <Loading />
      ) : snapshot.error ? (
        <ErrorNotice error={snapshot.error} />
      ) : (
        <>
          <ul>
            {snapshot.data.tasks.map((task) => (
              <li key={task.id} className="border-t border-border py-3">
                <p>
                  {task.provider_id} · {task.query}
                </p>
                <p className="text-sm">{t(`state_${task.state}`)}</p>
                {task.failure && (
                  <p role="alert">{t(`providerFailure_${task.failure}`)}</p>
                )}
                {["queued", "running", "waiting_for_user"].includes(
                  task.state,
                ) && (
                  <Button
                    variant="outline"
                    disabled={cancel.isPending}
                    onClick={() => cancel.mutate(task.id)}
                  >
                    {t("cancelTask")}
                  </Button>
                )}
              </li>
            ))}
          </ul>
          <h3 className="my-3 font-medium">{t("networkInspector")}</h3>
          <ul>
            {snapshot.data.requests.map((record) => (
              <li
                key={`${record.task_id}:${record.provider_id}:${record.attempt}:${record.safe_url}:${record.duration_ms}`}
                className="break-all py-2 text-xs"
              >
                {record.provider_id} · {record.method} · {record.safe_url} ·{" "}
                {record.status ?? "—"} · {record.duration_ms} ms
                {record.failure &&
                  ` · ${t(`providerFailure_${record.failure}`)}`}
              </li>
            ))}
          </ul>
        </>
      )}
      {cancel.error && <ErrorNotice error={cancel.error} />}
    </section>
  );
}
