/** biome-ignore-all lint/suspicious/noArrayIndexKey: Candidate and field positions are immutable server snapshot indices. */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { defaultBrowse, libraryApi } from "../../api/library";
import { providerApi } from "../../api/providers";
import {
  type MergePreview,
  type OutputPreview,
  type SearchPreview,
  scraperApi,
  scraperOutputApi,
  type VideoKind,
  type VideoMetadata,
} from "../../api/scraper";
import { Button } from "../../components/ui/button";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  useFormat,
} from "../library/shared";
import "./translations";
import { VideoSourceDetails } from "./source-detail";

function value(v: unknown): string {
  return typeof v === "string" ? v : (JSON.stringify(v) ?? "");
}
const providerChoices = ["tmdb", "anilist", "wikipedia.html"];
export function ScraperPage() {
  const { t, i18n } = useTranslation();
  const format = useFormat();
  const client = useQueryClient();
  const [kind, setKind] = useState<VideoKind>("movie");
  const [title, setTitle] = useState("");
  const [year, setYear] = useState("");
  const [namespace, setNamespace] = useState("");
  const [externalId, setExternalId] = useState("");
  const [providers, setProviders] = useState<string[]>(["tmdb"]);
  const [search, setSearch] = useState<SearchPreview>();
  const [metadata, setMetadata] = useState<VideoMetadata>();
  const [candidateIndex, setCandidateIndex] = useState<number>();
  const [selectedFields, setSelectedFields] = useState<string[]>([]);
  const [selectedArtwork, setSelectedArtwork] = useState<number[]>([]);
  const [target, setTarget] = useState("");
  const [localSearch, setLocalSearch] = useState("");
  const [localQuery, setLocalQuery] = useState("");
  const [preview, setPreview] = useState<MergePreview>();
  const [output, setOutput] = useState<OutputPreview>();
  const [invalid, setInvalid] = useState(false);
  const [thumbnails, setThumbnails] = useState<Record<number, string>>({});
  useEffect(() => {
    const id = output?.preview_id;
    if (!id) return;
    return () => {
      // Replacing or leaving a preview releases backend leases and never writes.
      void Promise.resolve(scraperOutputApi.discard(id)).catch(() => {
        // Backend leases expire independently if the desktop connection is gone.
      });
    };
  }, [output?.preview_id]);
  const snapshot = useQuery({
    queryKey: ["providers"],
    queryFn: providerApi.snapshot,
  });
  const history = useQuery({
    queryKey: ["scraper-history"],
    queryFn: scraperOutputApi.history,
  });
  const local = useQuery({
    queryKey: ["scraper-local", metadata?.kind, localQuery],
    enabled: Boolean(metadata),
    queryFn: () =>
      libraryApi.toolsMedia({
        ...defaultBrowse(false),
        kind: metadata?.kind === "tv" ? "tv_series" : (metadata?.kind ?? null),
        query: localQuery || null,
        limit: 48,
      }),
  });
  function clearDetail() {
    setMetadata(undefined);
    setCandidateIndex(undefined);
    setPreview(undefined);
    setOutput(undefined);
    setTarget("");
    setThumbnails({});
    run.reset();
    detail.reset();
    artworkLoad.reset();
    merge.reset();
    outputPlan.reset();
    confirm.reset();
    write.reset();
  }
  const run = useMutation({
    mutationFn: () =>
      scraperApi.search({
        query: {
          kind,
          title: title.trim(),
          year: year === "" ? null : Number(year),
          external_ids: externalId.trim()
            ? [{ namespace: namespace.trim(), value: externalId.trim() }]
            : [],
          language: i18n.resolvedLanguage === "en-US" ? "en-US" : "zh-CN",
        },
        providers,
      }),
    onSuccess: (result) => {
      setSearch(result);
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
  });
  const restore = useMutation({
    mutationFn: scraperApi.searchPreview,
    onSuccess: (result, runId) => {
      clearDetail();
      setSearch(result);
      const saved = history.data?.find((entry) => entry.id === runId);
      if (saved) {
        setKind(saved.query.kind);
        setTitle(saved.query.title);
        setYear(saved.query.year === null ? "" : String(saved.query.year));
        setNamespace(saved.query.external_ids[0]?.namespace ?? "");
        setExternalId(saved.query.external_ids[0]?.value ?? "");
        setProviders([
          ...new Set(
            result.candidates.map(
              (candidate) => candidate.candidate.data.provider_id,
            ),
          ),
        ]);
      }
    },
  });
  const detail = useMutation({
    mutationFn: ({ runId, index }: { runId: string; index: number }) =>
      scraperApi.detail(runId, index),
    onSuccess: (result) => {
      setMetadata(result);
      setSelectedFields(result.data.fields.map((f) => f.key));
      setSelectedArtwork([]);
    },
  });
  const artworkLoad = useMutation({
    mutationFn: (index: number) =>
      scraperApi.artworkPreview(
        search?.run_id ?? "",
        candidateIndex ?? 0,
        index,
      ),
    onSuccess: (result, index) =>
      setThumbnails((old) => ({ ...old, [index]: result.thumbnail })),
  });
  const merge = useMutation({
    mutationFn: () =>
      scraperApi.preview({
        run_id: search?.run_id ?? "",
        candidate_index: candidateIndex ?? 0,
        media_item_id: target,
        selected_fields: selectedFields,
        selected_artwork: selectedArtwork,
      }),
    onSuccess: setPreview,
  });
  const confirm = useMutation({
    mutationFn: () => scraperApi.confirm(preview?.preview_id ?? ""),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["media"] });
      void client.invalidateQueries({ queryKey: ["detail"] });
      void client.invalidateQueries({ queryKey: ["video-source-detail"] });
      void client.invalidateQueries({ queryKey: ["scraper-history"] });
    },
  });
  const outputPlan = useMutation({
    mutationFn: () => scraperOutputApi.preview(target),
    onSuccess: setOutput,
  });
  const discard = useMutation({
    mutationFn: () => scraperOutputApi.discard(output?.preview_id ?? ""),
    onSuccess: () => {
      setOutput(undefined);
      write.reset();
    },
  });
  const write = useMutation({
    mutationFn: () => scraperOutputApi.execute(output?.preview_id ?? ""),
    onSuccess: () => {
      void client.invalidateQueries({ queryKey: ["media"] });
      void client.invalidateQueries({ queryKey: ["detail"] });
      void client.invalidateQueries({ queryKey: ["video-source-detail"] });
    },
  });
  const busy =
    discard.isPending ||
    artworkLoad.isPending ||
    restore.isPending ||
    run.isPending ||
    detail.isPending ||
    merge.isPending ||
    confirm.isPending ||
    outputPlan.isPending ||
    write.isPending;
  const error =
    discard.error ??
    restore.error ??
    artworkLoad.error ??
    run.error ??
    detail.error ??
    merge.error ??
    confirm.error ??
    outputPlan.error ??
    write.error;
  const available = (id: string) =>
    snapshot.data?.providers.some(
      (p) => p.descriptor.id === id && p.config.enabled,
    );
  const supported = (id: string) =>
    snapshot.data?.providers.some(
      (provider) =>
        provider.descriptor.id === id &&
        provider.descriptor.media_kinds.includes(
          kind === "tv" ? "tv_series" : kind,
        ),
    );
  const selectCandidate = (index: number) => {
    if (!search) return;
    clearDetail();
    setCandidateIndex(index);
    detail.mutate({ runId: search.run_id, index });
  };
  return (
    <>
      <PageHeading title={t("scraper")} />
      {snapshot.isPending && <Loading />}
      {snapshot.error && <ErrorNotice error={snapshot.error} />}
      <form
        className="panel scraper-search"
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          const valid =
            (title.trim() || (namespace.trim() && externalId.trim())) &&
            (year === "" ||
              (Number.isInteger(Number(year)) &&
                Number(year) >= 1800 &&
                Number(year) <= 2200)) &&
            providers.length > 0 &&
            providers.every((id) => available(id) && supported(id));
          setInvalid(!valid);
          if (valid) {
            clearDetail();
            restore.reset();
            setSearch(undefined);
            run.mutate();
          }
        }}
      >
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          <label className="filter-field">
            <span>{t("type")}</span>
            <select
              value={kind}
              onChange={(e) => {
                setKind(e.target.value as VideoKind);
                setProviders(
                  e.target.value === "anime" ? ["anilist"] : ["tmdb"],
                );
                clearDetail();
                setSearch(undefined);
              }}
              disabled={busy}
            >
              {(["movie", "tv", "anime"] as const).map((k) => (
                <option key={k} value={k}>
                  {t(
                    k === "movie"
                      ? "scraperMovie"
                      : k === "tv"
                        ? "scraperTv"
                        : "scraperAnime",
                  )}
                </option>
              ))}
            </select>
          </label>
          <label className="filter-field">
            <span>{t("scraperTitle")}</span>
            <input
              value={title}
              maxLength={256}
              onChange={(e) => setTitle(e.target.value)}
              disabled={busy}
            />
          </label>
          <label className="filter-field">
            <span>{t("scraperYear")}</span>
            <input
              type="number"
              min={1800}
              max={2200}
              value={year}
              onChange={(e) => setYear(e.target.value)}
              disabled={busy}
            />
          </label>
          <label className="filter-field">
            <span>{t("scraperExternalNamespace")}</span>
            <input
              value={namespace}
              maxLength={32}
              onChange={(e) => setNamespace(e.target.value)}
              disabled={busy}
            />
          </label>
          <label className="filter-field">
            <span>{t("scraperExternalId")}</span>
            <input
              value={externalId}
              maxLength={128}
              onChange={(e) => setExternalId(e.target.value)}
              disabled={busy}
            />
          </label>
        </div>
        <fieldset className="my-4">
          <legend className="mb-2 font-medium">{t("scraperProviders")}</legend>
          <div className="flex flex-wrap gap-5">
            {providerChoices.map((id) => (
              <label key={id} className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={providers.includes(id)}
                  disabled={busy || !available(id) || !supported(id)}
                  onChange={(e) =>
                    setProviders((old) =>
                      e.target.checked
                        ? [...old, id]
                        : old.filter((p) => p !== id),
                    )
                  }
                />
                <span>
                  {id} {!available(id) && `(${t("disabled")})`}
                </span>
              </label>
            ))}
          </div>
        </fieldset>
        {invalid && (
          <p role="alert" className="notice error-notice mb-3">
            {t("scraperInvalid")}
          </p>
        )}
        <Button type="submit" disabled={busy || !snapshot.data}>
          {run.isPending ? t("scraperSearching") : t("scraperSearch")}
        </Button>
      </form>
      {error && (
        <div className="mt-4">
          <ErrorNotice error={error} />
        </div>
      )}
      {restore.isSuccess && (
        <p role="status" className="notice mt-4">
          {t("scraperRestored")}
        </p>
      )}
      {search && (
        <section className="mt-7" aria-labelledby="scraper-results">
          <h2 id="scraper-results" className="text-lg font-semibold">
            {t("scraperResults")}
          </h2>
          {search.failures.map((f, index) => (
            <p role="status" className="notice mt-3" key={index}>
              {f.provider_id}:{" "}
              {t(`providerFailure_${f.failure}`, {
                defaultValue: t("operationFailed"),
              })}
            </p>
          ))}
          {search.candidates.length === 0 && (
            <Empty title={t("scraperNoResults")} />
          )}
          <div className="scraper-candidates">
            {search.candidates.map((c, index) => (
              <article
                className={`panel ${candidateIndex === index ? "scraper-chosen" : ""}`}
                key={index}
              >
                <h3 className="font-semibold">
                  {c.candidate.data.title} ·{" "}
                  {c.candidate.data.year ?? t("unknownYear")}
                </h3>
                <p className="mt-1 text-sm text-muted-foreground">
                  {c.candidate.data.provider_id} · {c.candidate.data.remote_id}
                </p>
                <ul className="mt-3 list-disc pl-5 text-sm">
                  {c.reasons.map((reason, i) => (
                    <li key={i}>
                      {t(`scraperReason_${reason}`, {
                        defaultValue: t("scraperReasonFallback"),
                      })}
                    </li>
                  ))}
                </ul>
                {c.needs_review && (
                  <p className="mt-3 badge">{t("scraperReview")}</p>
                )}
                {c.duplicate_of !== null && (
                  <p className="mt-2 text-sm">
                    {t("scraperDuplicate", { index: c.duplicate_of + 1 })}
                  </p>
                )}
                <Button
                  className="mt-4"
                  variant="outline"
                  disabled={busy}
                  onClick={() => selectCandidate(index)}
                >
                  {t("scraperSelect")}
                </Button>
              </article>
            ))}
          </div>
        </section>
      )}
      {detail.isPending && <Loading />}
      {metadata && (
        <section className="panel mt-7" aria-labelledby="scraper-detail">
          <h2 id="scraper-detail" className="text-lg font-semibold">
            {t("scraperDetail")}: {metadata.data.identity.title}
          </h2>
          <VideoSourceDetails metadata={metadata} />
          <fieldset className="mt-6">
            <legend className="font-medium">{t("scraperFields")}</legend>
            {metadata.data.fields.map((field, index) => (
              <label className="scraper-field-choice" key={index}>
                <input
                  type="checkbox"
                  checked={selectedFields.includes(field.key)}
                  disabled={busy}
                  onChange={(e) => {
                    setSelectedFields((old) =>
                      e.target.checked
                        ? [...old, field.key]
                        : old.filter((k) => k !== field.key),
                    );
                    setPreview(undefined);
                    setOutput(undefined);
                    write.reset();
                    confirm.reset();
                  }}
                />
                <span>
                  <strong>
                    {t(`scraperField_${field.key}`, {
                      defaultValue: field.key,
                    })}
                  </strong>{" "}
                  <span className="text-muted-foreground">
                    {field.language ?? t("none")}
                  </span>
                  <span className="block whitespace-pre-wrap break-words">
                    {value(field.value)}
                  </span>
                </span>
              </label>
            ))}
          </fieldset>
          <fieldset className="mt-6">
            <legend className="font-medium">{t("scraperArtwork")}</legend>
            <p className="mt-2 text-sm text-muted-foreground">
              {t("scraperArtworkHelp")}
            </p>
            <div className="scraper-artwork-grid">
              {metadata.data.artwork.map((art, index) => (
                <label className="scraper-artwork-choice" key={index}>
                  <input
                    type="checkbox"
                    checked={selectedArtwork.includes(index)}
                    disabled={busy}
                    onChange={(e) => {
                      setSelectedArtwork((old) =>
                        e.target.checked
                          ? [...old, index]
                          : old.filter((i) => i !== index),
                      );
                      setPreview(undefined);
                      setOutput(undefined);
                      write.reset();
                      confirm.reset();
                    }}
                  />
                  <span>
                    <strong>
                      {t("scraperArtworkItem", {
                        kind: t(`scraperArtwork_${art.kind}`, {
                          defaultValue: art.kind,
                        }),
                        provider: art.source_provider,
                        index: index + 1,
                      })}
                    </strong>
                    <span className="block break-all text-xs">{art.url}</span>
                    <span>{art.language ?? t("none")}</span>
                    {thumbnails[index] && (
                      <img
                        src={thumbnails[index]}
                        alt={t("scraperArtworkAlt", { kind: art.kind })}
                        className="scraper-thumbnail"
                      />
                    )}
                    <Button
                      type="button"
                      className="mt-2"
                      size="sm"
                      variant="outline"
                      disabled={
                        busy ||
                        discard.isPending ||
                        artworkLoad.isPending ||
                        Boolean(thumbnails[index])
                      }
                      onClick={(event) => {
                        event.preventDefault();
                        artworkLoad.mutate(index);
                      }}
                    >
                      {t("scraperLoadArtwork")}
                    </Button>
                  </span>
                </label>
              ))}
            </div>
          </fieldset>
          <div className="mt-6 border-t border-border pt-5">
            <label className="filter-field">
              <span>{t("scraperTargetSearch")}</span>
              <input
                value={localSearch}
                onChange={(e) => setLocalSearch(e.target.value)}
                disabled={busy}
              />
            </label>
            <Button
              className="mt-2"
              variant="outline"
              disabled={busy}
              onClick={() => setLocalQuery(localSearch.trim())}
            >
              {t("scraperLocalSearch")}
            </Button>
            <label className="filter-field mt-4">
              <span>{t("scraperTarget")}</span>
              <select
                value={target}
                disabled={busy || local.isPending}
                onChange={(e) => {
                  setTarget(e.target.value);
                  setPreview(undefined);
                  setOutput(undefined);
                  confirm.reset();
                  write.reset();
                }}
              >
                <option value="">{t("scraperChooseTarget")}</option>
                {local.data?.items.map((card) => (
                  <option key={card.media.id} value={card.media.id}>
                    {card.media.title} · {card.media.year ?? t("unknownYear")}
                  </option>
                ))}
              </select>
            </label>
            {local.error && <ErrorNotice error={local.error} />}
            <Button
              className="mt-4"
              disabled={
                !target ||
                busy ||
                (!selectedFields.length && !selectedArtwork.length)
              }
              onClick={() => {
                confirm.reset();
                setOutput(undefined);
                merge.mutate();
              }}
            >
              {t("scraperPreview")}
            </Button>
          </div>
        </section>
      )}
      {preview && (
        <section className="panel mt-7" aria-labelledby="scraper-preview">
          <h2 id="scraper-preview" className="text-lg font-semibold">
            {t("scraperPreviewTitle")}
          </h2>
          <div className="mt-4 overflow-x-auto">
            <table className="data-table scraper-diff">
              <thead>
                <tr>
                  <th>{t("scraperField")}</th>
                  <th>{t("scraperOld")}</th>
                  <th>{t("scraperNew")}</th>
                  <th>{t("scraperSource")}</th>
                  <th>{t("state")}</th>
                </tr>
              </thead>
              <tbody>
                {preview.fields.map((field, index) => (
                  <tr key={index}>
                    <td>
                      {t(`scraperField_${field.key}`, {
                        defaultValue: field.key,
                      })}
                      <span className="block text-xs">
                        {field.language ?? t("none")}
                      </span>
                    </td>
                    <td>
                      {field.old ? value(field.old.value) : t("none")}
                      {field.old?.is_locked && (
                        <span className="badge">{t("locked")}</span>
                      )}
                      {field.old?.is_user_modified && (
                        <span className="badge">{t("userEdited")}</span>
                      )}
                    </td>
                    <td>{value(field.new.value)}</td>
                    <td>
                      {field.provider_id}
                      <span className="block break-all text-xs">
                        {field.source_url}
                      </span>
                    </td>
                    <td>
                      {field.protected
                        ? t("scraperProtected")
                        : field.selected
                          ? t("scraperSelected")
                          : t("scraperNotSelected")}
                      {field.needs_review && !field.protected && (
                        <span className="block text-xs">
                          {t("scraperConflict")}
                        </span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <Button
            className="mt-5"
            disabled={
              busy ||
              confirm.isSuccess ||
              preview.confirmed ||
              (!preview.fields.some((f) => f.selected && !f.protected) &&
                !preview.artwork.length)
            }
            onClick={() => confirm.mutate()}
          >
            {t("scraperConfirm")}
          </Button>
          {confirm.data && (
            <p role="status" className="notice mt-4">
              {confirm.data.already_applied
                ? t("scraperAlready")
                : t("scraperConfirmed", { count: confirm.data.applied_fields })}
            </p>
          )}
          {(confirm.isSuccess || preview.confirmed) && (
            <Button
              className="mt-4 ml-3"
              variant="outline"
              disabled={busy}
              onClick={() => {
                write.reset();
                outputPlan.mutate();
              }}
            >
              {t("scraperOutputPreview")}
            </Button>
          )}
        </section>
      )}
      {output && (
        <section className="panel mt-7" aria-labelledby="scraper-output">
          <h2 id="scraper-output" className="text-lg font-semibold">
            {t("scraperOutputTitle")}
          </h2>
          <p className="mt-2 text-sm text-muted-foreground">
            {t("scraperOutputHelp")}
          </p>
          {output.files.map((file, index) => (
            <article key={index} className="scraper-output-file">
              <h3 className="font-medium break-all">{file.destination}</h3>
              {file.protected && (
                <p className="badge">{t("scraperOutputProtected")}</p>
              )}
              <div className="scraper-output-text">
                <div>
                  <h4>{t("scraperOld")}</h4>
                  <pre>{file.old_text ?? t("none")}</pre>
                </div>
                <div>
                  <h4>{t("scraperNew")}</h4>
                  <pre>{file.new_text ?? file.content_hash}</pre>
                </div>
              </div>
            </article>
          ))}
          <Button
            className="mt-4"
            disabled={busy || write.isSuccess || output.files.length === 0}
            onClick={() => write.mutate()}
          >
            {t("scraperOutputExecute")}
          </Button>
          <Button
            className="mt-4 ml-3"
            variant="outline"
            disabled={busy}
            onClick={() => discard.mutate()}
          >
            {t("scraperOutputDiscard")}
          </Button>
          {write.data && (
            <p role="status" className="notice mt-4">
              {t("scraperOutputDone", {
                written: write.data.written,
                preserved: write.data.preserved,
              })}
            </p>
          )}
        </section>
      )}
      <section className="panel mt-7" aria-labelledby="scraper-history">
        <h2 id="scraper-history" className="text-lg font-semibold">
          {t("scraperHistory")}
        </h2>
        {history.error && <ErrorNotice error={history.error} />}
        <ul className="mt-3 space-y-2">
          {history.data?.map((entry) => (
            <li
              key={entry.id}
              className="flex flex-wrap justify-between gap-2 text-sm"
            >
              <span>
                {entry.query.title ||
                  entry.query.external_ids
                    .map((id) => `${id.namespace}:${id.value}`)
                    .join(" · ")}{" "}
                · {format.date(entry.created_at)}
              </span>
              <span>
                {t(`state_${entry.state}`, { defaultValue: t("needsReview") })}{" "}
                · {format.number(entry.candidate_count)} {t("candidates")}
              </span>
              <Button
                variant="outline"
                size="sm"
                disabled={busy || entry.candidate_count === 0}
                onClick={() => restore.mutate(entry.id)}
              >
                {t("scraperRestore")}
              </Button>
            </li>
          ))}
        </ul>
        {history.data?.length === 0 && (
          <p className="mt-3 text-muted-foreground">
            {t("scraperHistoryEmpty")}
          </p>
        )}
      </section>
    </>
  );
}
