import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { type FormEvent, useEffect, useRef, useState } from "react";
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
import { Alert } from "../../components/ui/alert";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { toast } from "../../components/ui/sonner";
import "../library/library.css";
import { MediaDetailPage } from "../library/detail";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import { MediaToolsPage } from "../tools/page";
import "./translations";
import "../scraper/navigation";
import type { LibrarySearch } from "../library/browse-model";
import { AdultGate } from "./gate";
import { AdultLibraryPage } from "./library-page";

export { AdultGate } from "./gate";

export function AdultPage() {
  const [search, setSearch] = useState<LibrarySearch>({ kind: "adult_video" });
  return <AdultLibraryPage search={search} onSearch={setSearch} />;
}
export function AdultToolsPage({ mediaId }: { mediaId?: string } = {}) {
  return (
    <AdultGate>
      <MediaToolsPage search={{ kind: "adult_video", media: mediaId }} />
    </AdultGate>
  );
}
export function AdultMetadata({ detail }: { detail: AdultDetail }) {
  const { t } = useTranslation();
  const c = detail.identity.catalog_number;
  return (
    <Card asChild>
      <section>
        <h2 className="section-title">
          {c.normalized} · {detail.identity.title}
          {c.is_primary && (
            <Badge variant="secondary" className="ml-2">{t("adultPrimaryCatalog")}</Badge>
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
      </section>
    </Card>
  );
}
export function OutputDiff({ preview }: { preview: AdultOutputPreview }) {
  const { t } = useTranslation();
  return (
    <Card asChild>
      <section>
        <h3 className="section-title">{t("adultOutputFiles")}</h3>
        <Alert asChild>
          <p className="mb-3">{t("adultPreserve")}</p>
        </Alert>
        {preview.files.map((file) => (
          <div key={file.destination} className="border-t border-border py-3">
            <p className="break-all font-medium">
              {file.destination} · {file.kind}
            </p>
            {file.protected && (
              <Badge variant="secondary">{t("adultFieldProtected")}</Badge>
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
    </Card>
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
    onSuccess: () => {
      if (!active.current) return;
      setPreview(undefined);
      toast.success(t("adultOutputDone"));
    },
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
    </div>
  );
}
export function AdultBatchOutput({ mediaIds }: { mediaIds: string[] }) {
  const { t } = useTranslation();
  const selected = [...new Set(mediaIds)];
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
    <section className="space-y-4">
      <Button
        disabled={
          !selected.length ||
          selected.length > 32 ||
          plan.isPending ||
          execute.isPending ||
          !!batch
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
        <Card asChild>
          <section role="status">
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
        </Card>
      )}{" "}
    </section>
  );
}
export function AdultScraper({ mediaId }: { mediaId?: string } = {}) {
  return <CatalogScraper key={mediaId ?? "search"} mediaId={mediaId} />;
}
function CatalogScraper({ mediaId }: { mediaId?: string }) {
  const { t, i18n } = useTranslation();
  const client = useQueryClient();
  const active = useRef(false);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  const [catalog, setCatalog] = useState("");
  const [namespace, setNamespace] = useState("dmm");
  const [studio, setStudio] = useState("");
  const [selectedProviders, setSelectedProviders] = useState<string[]>([]);
  const [search, setSearch] = useState<AdultSearchPreview>();
  const [detail, setDetail] = useState<AdultDetail>();
  const [candidateIndex, setCandidateIndex] = useState<number>();
  const [selectedTarget, setTarget] = useState("");
  const target = mediaId ?? selectedTarget;
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
    enabled: !mediaId,
    queryFn: () =>
      adultApi.toolsMedia({ ...defaultBrowse(true), kind: "adult_video" }),
  });
  const localTarget = useQuery({
    queryKey: ["adult", "tools-detail", mediaId],
    queryFn: () => adultApi.toolsDetail(mediaId ?? ""),
    enabled: Boolean(mediaId),
  });
  const seeded = useRef(false);
  useEffect(() => {
    if (!localTarget.data || seeded.current) return;
    seeded.current = true;
    const parsedCatalog = localTarget.data.metadata_values.find(
      (field) =>
        field.field_key === "catalog_number" && typeof field.value === "string",
    );
    if (parsedCatalog) setCatalog(String(parsedCatalog.value));
  }, [localTarget.data]);
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
      if (!active.current) return;
      setMerge(undefined);
      void client.invalidateQueries({ queryKey: ["adult"] });
      toast.success(t("adultMerged"));
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
      <Card asChild>
        <section>
          <h2 className="section-title">{t("adultProviders")}</h2>
          <Button asChild variant="outline">
            <Link className="mb-4" to="/settings">
              {t("providerAuthOpenSettings")}
            </Link>
          </Button>
          {providers.isPending ? (
            <Loading />
          ) : providers.error ? (
            <ErrorNotice error={providers.error} />
          ) : (
            providers.data.providers.map((p) => (
              <div key={p.descriptor.id} className="mb-4">
                <div className="flex flex-wrap items-center gap-3">
                  <Label className="flex gap-2">
                    <Checkbox
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
                  </Label>
                  <Badge variant="secondary">
                    {t(`providerHealth_${p.health}`, { defaultValue: p.health })}
                  </Badge>
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
                  {p.descriptor.attribution}
                </p>
              </div>
            ))
          )}
          {enable.error && <ErrorNotice error={enable.error} />}
        </section>
      </Card>
      <Card asChild>
        <form className="space-y-4" onSubmit={submit}>
          <Label className="filter-field">
            <span>{t("adultCatalog")}</span>
            <Input
              required
              value={catalog}
              onChange={(event) => setCatalog(event.target.value)}
              maxLength={128}
            />
          </Label>
          <Label className="filter-field">
            <span>{t("adultNamespace")}</span>
            <Input
              required
              value={namespace}
              onChange={(event) => setNamespace(event.target.value)}
              maxLength={64}
            />
          </Label>
          <Label className="filter-field">
            <span>{t("adultStudio")}</span>
            <Input
              value={studio}
              onChange={(event) => setStudio(event.target.value)}
              maxLength={256}
            />
          </Label>
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
      </Card>
      {searching.error && <ErrorNotice error={searching.error} />}{" "}
      {search && (
        <Card asChild>
          <section>
            <h2 className="section-title">{t("candidates")}</h2>
            {search.failures.map((f) => (
              <Alert asChild role="note" key={f.provider_id}>
                <p>
                  {f.provider_id}: {t(`providerFailure_${f.failure}`)}
                </p>
              </Alert>
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
                  <Badge variant="secondary">{t("adultReview")}</Badge>
                )}
                {entry.duplicate_of !== null && (
                  <Badge variant="secondary">{t("adultDuplicate")}</Badge>
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
        </Card>
      )}
      {loadingDetail.error && <ErrorNotice error={loadingDetail.error} />}{" "}
      {detail && (
        <>
          <AdultMetadata detail={detail} />
          <Card asChild>
            <section className="space-y-4">
              {mediaId ? (
                localTarget.isPending ? (
                  <Loading />
                ) : localTarget.error ? (
                  <ErrorNotice error={localTarget.error} />
                ) : (
                  <h3 className="font-medium">{localTarget.data?.media.title}</h3>
                )
              ) : (
                <>
                  {" "}
                  <Label className="filter-field">
                    <span>{t("adultTarget")}</span>
                    <Select
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
                    </Select>
                  </Label>
                  {local.error && <ErrorNotice error={local.error} />}
                </>
              )}
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
                    <Label className="flex gap-2" key={key}>
                      <Checkbox
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
                    </Label>
                  ))}
                </div>
              </fieldset>
              <fieldset>
                <legend className="font-medium">{t("adultImages")}</legend>
                {detail.artwork.map((art, index) => (
                  <div key={`${art.kind}:${art.url}`} className="mt-3 flex gap-3">
                    <Label className="flex gap-2">
                      <Checkbox
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
                    </Label>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={image.isPending}
                      onClick={() => image.mutate(index)}
                    >
                      {t("adultImagePreview")}
                    </Button>
                  </div>
                ))}
                {thumbnail && (
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
          </Card>
          {preview.error && <ErrorNotice error={preview.error} />}{" "}
          {merge && (
            <Card asChild>
              <section className="space-y-4">
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
                        <Badge variant="secondary">{t("adultFieldProtected")}</Badge>
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
            </Card>
          )}
          {confirm.error && <ErrorNotice error={confirm.error} />}{" "}
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
      <MediaDetailPage
        mediaId={mediaId}
        adult
        sourceDetail={<AdultSourceDetail mediaId={mediaId} />}
      />
    </AdultGate>
  );
}
export function AdultSourceDetail({ mediaId }: { mediaId: string }) {
  const source = useQuery({
    queryKey: ["adult", "source", mediaId],
    queryFn: () => adultApi.mediaDetail(mediaId),
  });
  if (source.isPending) return <Loading />;
  if (source.error)
    return (
      <ErrorNotice error={source.error} retry={() => void source.refetch()} />
    );
  return source.data ? <AdultMetadata detail={source.data} /> : null;
}

export function AdultTasksPage() {
  const { t } = useTranslation();
  return (
    <AdultGate>
      <PageHeading title={t("tasks")} />
      <AdultTasks />
    </AdultGate>
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
    <Card asChild>
      <section>
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
    </Card>
  );
}
