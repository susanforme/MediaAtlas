/** biome-ignore-all lint/suspicious/noArrayIndexKey: Candidate and field positions are immutable server snapshot indices. */
import { Alert } from "../../components/ui/alert";
import { Badge } from "../../components/ui/badge";
import { Card } from "../../components/ui/card";
import { Label } from "../../components/ui/label";
import "./styles.css";
import { Link, useNavigate } from "@tanstack/react-router";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import type { VideoKind } from "../../api/scraper";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Details, DetailsSummary } from "../../components/ui/details";
import { Input } from "../../components/ui/input";
import { Select } from "../../components/ui/select";
import {
  ErrorNotice,
  Loading,
  PageHeading,
  useFormat,
} from "../library/shared";
import "./translations";
import { validateProviderSetupSearch } from "../settings/setup-search";
import { MetadataMergeWorkspace } from "./merge-workspace";
import { PersistentQueueNavigation } from "./persistent-queue-navigation";
import { ScraperQueueNavigation } from "./queue-navigation";
import { ScraperRecovery } from "./recovery-actions";
import { providerChoices, providerNames, scrapeKind } from "./scraper-model";
import { validateScraperSearch } from "./search";
import "./merge-translations";
import { type ReviewSession, useReviewQueue } from "./use-review-queue";
import { useScraper } from "./use-scraper";

interface ScraperPageProps {
  mediaId?: string;
  queue?: string[];
  sessionId?: string;
  onSessionNavigate?: (sessionId: string, mediaId?: string) => void;
  onQueueNavigate?: (mediaId: string) => void;
}

export function ScraperPage({
  mediaId,
  queue,
  onQueueNavigate,
  sessionId,
  onSessionNavigate,
}: ScraperPageProps = {}) {
  const context = validateScraperSearch({
    media: mediaId,
    queue,
    session: sessionId,
  });
  if (context.session)
    return (
      <PersistentScraperPage
        key={context.session}
        mediaId={context.media}
        sessionId={context.session}
        onSessionNavigate={onSessionNavigate}
        onQueueNavigate={onQueueNavigate}
      />
    );
  return (
    <ScraperWorkspace
      key={context.media ?? "local-selection"}
      mediaId={context.media}
      queue={context.queue}
      onQueueNavigate={onQueueNavigate}
    />
  );
}

function PersistentScraperPage({
  mediaId,
  sessionId,
  onSessionNavigate,
  onQueueNavigate,
}: ScraperPageProps & { sessionId: string }) {
  const review = useReviewQueue(sessionId, mediaId);
  if (review.query.isPending) return <Loading />;
  if (review.query.isError)
    return <ErrorNotice error={review.query.error} retry={review.refresh} />;
  return (
    <ScraperWorkspace
      key={review.selectedId}
      mediaId={review.selectedId}
      sessionId={sessionId}
      review={review}
      onQueueNavigate={(id) =>
        onSessionNavigate
          ? onSessionNavigate(sessionId, id)
          : onQueueNavigate?.(id)
      }
    />
  );
}

function ScraperWorkspace({
  mediaId,
  queue,
  onQueueNavigate,
  sessionId,
  review,
}: ScraperPageProps & { review?: ReviewSession }) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const titleInput = useRef<HTMLInputElement>(null);
  const format = useFormat();
  const workspace = useScraper(
    mediaId,
    review
      ? { confirmPreview: review.confirm, failed: review.failed }
      : undefined,
  );
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
    setProviders,
    search,
    setSearch,
    metadata,
    candidateIndex,
    target,
    targetDetail,
    validTarget,
    setTarget,
    localSearch,
    setLocalSearch,
    setLocalQuery,
    preview,
    output,
    invalid,
    setInvalid,
    snapshot,
    history,
    local,
    run,
    restore,
    detail,
    confirm,
    discard,
    write,
    busy: workspaceBusy,
    error,
    available,
    providerState,
    selectedProviders,
    searchValid,
    selectCandidate,
    clearDetail,
  } = workspace;
  const busy = workspaceBusy || Boolean(review?.busy);
  const reviewNavigation = review ? (
    <PersistentQueueNavigation
      review={review}
      compact={Boolean(metadata && validTarget)}
      busy={busy}
      onNavigate={onQueueNavigate}
    />
  ) : null;
  const recovery = (
    <ScraperRecovery
      search={search}
      error={run.error ?? restore.error}
      busy={busy}
      onRetry={
        searchValid
          ? () => {
              void (async () => {
                if (
                  review?.selected &&
                  ["failed", "deferred"].includes(review.selected.state)
                )
                  await review.transition("retry", review.selected.media_id);
                clearDetail();
                run.mutate();
              })().catch(() => {});
            }
          : undefined
      }
      onEdit={
        !review || review.canWork
          ? () => titleInput.current?.focus()
          : undefined
      }
      onConfigure={(provider) =>
        void navigate({
          to: "/settings",
          search: validateProviderSetupSearch({
            tab: "auth",
            from: "scraper",
            media: target || undefined,
            queue,
            session: sessionId,
            provider: provider ?? (kind === "anime" ? "anilist" : "tmdb"),
          }),
        })
      }
    />
  );
  if (review && !review.canWork)
    return (
      <>
        <PageHeading title={t("scraper")} />
        {reviewNavigation}
        {recovery}
        {error && error !== run.error && error !== restore.error && (
          <ErrorNotice error={error} />
        )}
      </>
    );
  return (
    <>
      <PageHeading
        title={t(metadata ? "mergeTitle" : "scraper")}
        description={metadata ? t("mergeDescription") : undefined}
      />
      {reviewNavigation}
      {queue && mediaId && (
        <ScraperQueueNavigation
          mediaId={mediaId}
          queue={queue}
          busy={busy}
          confirmed={
            confirm.isSuccess || write.isSuccess || Boolean(preview?.confirmed)
          }
          onNavigate={onQueueNavigate}
        />
      )}
      <div hidden={Boolean(metadata && validTarget)}>
          <section className="mb-5 space-y-3 border-b border-border pb-4 [&_[data-slot=input]]:h-9 [&_[data-slot=select]]:h-9">
            {mediaId ? (
              targetDetail.isPending ? (
                <Loading />
              ) : targetDetail.error ? (
                <ErrorNotice error={targetDetail.error} />
              ) : validTarget ? (
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <h2 className="font-semibold">
                    {targetDetail.data?.media.title}
                  </h2>
                  <Badge variant="secondary">{t("scraperTargetLocked")}</Badge>
                </div>
              ) : null
            ) : (
              <>
                <form
                  className="flex max-w-2xl flex-wrap items-end gap-3"
                  onSubmit={(event) => {
                    event.preventDefault();
                    setLocalQuery(localSearch.trim());
                  }}
                >
                  <Label className="filter-field min-w-0 flex-1">
                    <span>{t("scraperTargetSearch")}</span>
                    <Input
                      type="search"
                      value={localSearch}
                      onChange={(e) => setLocalSearch(e.target.value)}
                    />
                  </Label>
                  <Button variant="outline" type="submit" size="sm">
                    {t("scraperLocalSearch")}
                  </Button>
                </form>
                <Label className="filter-field max-w-2xl">
                  <span>{t("scraperTarget")}</span>
                  <Select
                    value={target}
                    disabled={local.isPending}
                    onChange={(e) => setTarget(e.target.value)}
                  >
                    <option value="">{t("scraperAnonymous")}</option>
                    {local.data?.items
                      .filter((card) => scrapeKind(card.media.kind))
                      .map((card) => (
                        <option key={card.media.id} value={card.media.id}>
                          {card.media.title} ·{" "}
                          {card.media.year ?? t("unknownYear")}
                        </option>
                      ))}
                  </Select>
                </Label>
                {local.isPending && <Loading />}
                {local.error && <ErrorNotice error={local.error} />}
                {targetDetail.error && <ErrorNotice error={targetDetail.error} />}
              </>
            )}
            {target && targetDetail.data && !validTarget && (
              <Alert asChild variant="destructive">
                <p role="alert">
                  {t("scraperUnsupportedTarget")}
                </p>
              </Alert>
            )}
          </section>
        {snapshot.isPending && <Loading />}
        {snapshot.error && <ErrorNotice error={snapshot.error} />}
          <form
            className="scraper-search"
            noValidate
            onSubmit={(event) => {
              event.preventDefault();
              const valid = searchValid;
              setInvalid(!valid);
              if (valid) {
                clearDetail();
                restore.reset();
                setSearch(undefined);
                run.mutate();
              }
            }}>
            <div className="scraper-search-fields">
              <Label className="filter-field">
                <span>{t("scraperTitle")}</span>
                <Input
                  value={title}
                  ref={titleInput}
                  aria-describedby={
                    selectedProviders.includes("wikipedia.html")
                      ? "scraper-wikipedia-title-hint"
                      : undefined
                  }
                  maxLength={2048}
                  onChange={(e) => setTitle(e.target.value)}
                  disabled={busy}
                />
              </Label>
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
                  disabled={busy || Boolean(target)}
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
            </div>
            {selectedProviders.includes("wikipedia.html") && (
              <p
                id="scraper-wikipedia-title-hint"
                className="mt-3 text-sm text-muted-foreground"
              >
                {t("scraperWikipediaTitleHint")}
              </p>
            )}
            <Details className="mt-3 rounded-none border-0 p-0">
              <DetailsSummary>{t("scraperAdvanced")}</DetailsSummary>
              <div className="mt-3 grid max-w-2xl gap-3 sm:grid-cols-2">
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
                    maxLength={2048}
                    onChange={(e) => setExternalId(e.target.value)}
                    disabled={busy}
                  />
                </Label>
              </div>
            </Details>
            <fieldset className="my-4">
              <legend className="mb-2 font-medium">
                {t("scraperProviders")}
              </legend>
              <div className="flex flex-wrap gap-5">
                {providerChoices.map((id) => (
                  <Label key={id} className="flex items-center gap-2">
                    <Checkbox
                      checked={selectedProviders.includes(id)}
                      disabled={busy || !available(id)}
                      onChange={(e) =>
                        setProviders((old) =>
                          e.target.checked
                            ? [...old, id]
                            : old.filter((p) => p !== id),
                        )
                      }
                    />
                    <span>
                      {providerNames[id]}{" "}
                      {providerState(id) !== "ready" &&
                        `(${t(`scraperProviderState_${providerState(id)}`)})`}
                    </span>
                  </Label>
                ))}
              </div>
            </fieldset>
            {providerChoices.some((id) =>
              ["disabled", "credentials"].includes(providerState(id)),
            ) && (
              <div className="mb-4 flex flex-wrap gap-3">
                <Button asChild variant="outline" size="sm">
                  <Link
                    to="/settings"
                    search={{
                      tab: "providers",
                      from: "scraper",
                      media: target || undefined,
                      queue,
                      session: sessionId,
                    }}
                  >
                    {t("scraperConfigureProviders")}
                  </Link>
                </Button>
                <Button asChild variant="outline" size="sm">
                  <Link
                    to="/settings"
                    search={{
                      tab: "auth",
                      from: "scraper",
                      media: target || undefined,
                      queue,
                      session: sessionId,
                      provider: kind === "anime" ? "anilist" : "tmdb",
                    }}
                  >
                    {t("scraperConfigureAuth")}
                  </Link>
                </Button>
              </div>
            )}
            {invalid && (
              <Alert asChild variant="destructive">
                <p role="alert" className="mb-3">
                  {t("scraperInvalid")}
                </p>
              </Alert>
            )}
            <Button
              type="submit"
              disabled={busy || !snapshot.data || !searchValid}
            >
              {run.isPending ? t("scraperSearching") : t("scraperSearch")}
            </Button>
            {(run.isPending || restore.isPending) && (
              <Button
                className="ml-3"
                variant="outline"
                onClick={() => {
                  clearDetail();
                  restore.reset();
                  setSearch(undefined);
                }}
              >
                {t("scraperStopWaiting")}
              </Button>
            )}
          </form>
        {recovery}
        {error && error !== run.error && error !== restore.error && (
          <div className="mt-4">
            <ErrorNotice error={error} />
          </div>
        )}
        {search && (
          <section className="mt-7" aria-labelledby="scraper-results">
            <h2 id="scraper-results" className="text-lg font-semibold">
              {t("scraperResults")}
            </h2>
            <div className="scraper-candidates">
              {search.candidates.map((c, index) => (
                <Card asChild key={index}>
                  <article
                    className={candidateIndex === index ? "scraper-chosen" : ""}
                  >
                    <h3 className="font-semibold">
                      {c.candidate.data.title} ·{" "}
                      {c.candidate.data.year ?? t("unknownYear")}
                    </h3>
                    <p className="mt-1 text-sm text-muted-foreground">
                      {c.candidate.data.provider_id} ·{" "}
                      {c.candidate.data.remote_id}
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
                      <Badge variant="secondary" className="mt-3">{t("scraperReview")}</Badge>
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
                </Card>
              ))}
            </div>
          </section>
        )}
        {detail.isPending && <Loading />}
        {candidateIndex !== undefined && (
          <Button
            className="mt-4"
            variant="outline"
            onClick={clearDetail}
            disabled={confirm.isPending || write.isPending}
          >
            {t("scraperCancelSelection")}
          </Button>
        )}
      </div>
      {metadata && (
        <>
          {error && (
            <div className="mb-4">
              <ErrorNotice error={error} />
            </div>
          )}
          <MetadataMergeWorkspace
            workspace={workspace}
            busy={busy}
            onCancel={clearDetail}
          />
        </>
      )}
      {output && (
        <Card asChild>
          <section className="mt-7" aria-labelledby="scraper-output">
            <h2 id="scraper-output" className="text-lg font-semibold">
              {t("scraperOutputTitle")}
            </h2>
            <p className="mt-2 text-sm text-muted-foreground">
              {t(output.strategy === "update_nfo" ? "scraperOutputUpdateHelp" : "scraperOutputHelp")}
            </p>
            {output.files.map((file, index) => (
              <article key={index} className="scraper-output-file">
                <h3 className="font-medium break-all">{file.destination}</h3>
                {file.protected && (
                  <Badge variant="secondary">{t("scraperOutputProtected")}</Badge>
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
          </section>
        </Card>
      )}
      {!metadata && (
        <Card asChild>
          <section className="mt-7" aria-labelledby="scraper-history">
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
                    {t(`state_${entry.state}`, {
                      defaultValue: t("needsReview"),
                    })}{" "}
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
        </Card>
      )}
    </>
  );
}
