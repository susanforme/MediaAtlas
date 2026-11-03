/** biome-ignore-all lint/suspicious/noArrayIndexKey: Candidate and field positions are immutable server snapshot indices. */
import { Label } from "../../components/ui/label";
import "./styles.css";
import { useTranslation } from "react-i18next";
import type { VideoKind } from "../../api/scraper";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Select } from "../../components/ui/select";
import {
  Empty,
  ErrorNotice,
  Loading,
  PageHeading,
  useFormat,
} from "../library/shared";
import "./translations";
import { providerChoices, value } from "./scraper-model";
import { VideoSourceDetails } from "./source-detail";
import { useScraper } from "./use-scraper";

export function ScraperPage() {
  const { t } = useTranslation();
  const format = useFormat();
  const {
    kind,
    setKind,
    title,
    setTitle,
    year,
    setYear,
    namespace,
    setNamespace,
    externalId,
    setExternalId,
    providers,
    setProviders,
    search,
    setSearch,
    metadata,
    candidateIndex,
    selectedFields,
    setSelectedFields,
    selectedArtwork,
    setSelectedArtwork,
    target,
    setTarget,
    localSearch,
    setLocalSearch,
    setLocalQuery,
    preview,
    setPreview,
    output,
    setOutput,
    invalid,
    setInvalid,
    thumbnails,
    snapshot,
    history,
    local,
    run,
    restore,
    detail,
    artworkLoad,
    merge,
    confirm,
    outputPlan,
    discard,
    write,
    busy,
    error,
    available,
    supported,
    selectCandidate,
    clearDetail,
  } = useScraper();
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
          <Label className="filter-field">
            <span>{t("type")}</span>
            <Select
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
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("scraperTitle")}</span>
            <Input
              value={title}
              maxLength={256}
              onChange={(e) => setTitle(e.target.value)}
              disabled={busy}
            />
          </Label>
          <Label className="filter-field">
            <span>{t("scraperYear")}</span>
            <Input
              type="number"
              min={1800}
              max={2200}
              value={year}
              onChange={(e) => setYear(e.target.value)}
              disabled={busy}
            />
          </Label>
          <Label className="filter-field">
            <span>{t("scraperExternalNamespace")}</span>
            <Input
              value={namespace}
              maxLength={32}
              onChange={(e) => setNamespace(e.target.value)}
              disabled={busy}
            />
          </Label>
          <Label className="filter-field">
            <span>{t("scraperExternalId")}</span>
            <Input
              value={externalId}
              maxLength={128}
              onChange={(e) => setExternalId(e.target.value)}
              disabled={busy}
            />
          </Label>
        </div>
        <fieldset className="my-4">
          <legend className="mb-2 font-medium">{t("scraperProviders")}</legend>
          <div className="flex flex-wrap gap-5">
            {providerChoices.map((id) => (
              <Label key={id} className="flex items-center gap-2">
                <Checkbox
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
              </Label>
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
              <Label className="scraper-field-choice" key={index}>
                <Checkbox
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
              </Label>
            ))}
          </fieldset>
          <fieldset className="mt-6">
            <legend className="font-medium">{t("scraperArtwork")}</legend>
            <p className="mt-2 text-sm text-muted-foreground">
              {t("scraperArtworkHelp")}
            </p>
            <div className="scraper-artwork-grid">
              {metadata.data.artwork.map((art, index) => (
                <Label className="scraper-artwork-choice" key={index}>
                  <Checkbox
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
                </Label>
              ))}
            </div>
          </fieldset>
          <div className="mt-6 border-t border-border pt-5">
            <Label className="filter-field">
              <span>{t("scraperTargetSearch")}</span>
              <Input
                value={localSearch}
                onChange={(e) => setLocalSearch(e.target.value)}
                disabled={busy}
              />
            </Label>
            <Button
              className="mt-2"
              variant="outline"
              disabled={busy}
              onClick={() => setLocalQuery(localSearch.trim())}
            >
              {t("scraperLocalSearch")}
            </Button>
            <Label className="filter-field mt-4">
              <span>{t("scraperTarget")}</span>
              <Select
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
              </Select>
            </Label>
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
