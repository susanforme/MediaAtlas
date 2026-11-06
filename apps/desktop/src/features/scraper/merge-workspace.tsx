import { ImageOff } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { NfoExportMode } from "../../api/scraper";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Details, DetailsSummary } from "../../components/ui/details";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "../../components/ui/table";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "../../components/ui/tabs";
import { ErrorNotice } from "../library/shared";
import type { MergePreviewRow } from "./merge-preview-model";
import { value } from "./scraper-model";
import { VideoSourceDetails } from "./source-detail";
import type { useScraper } from "./use-scraper";
import "./merge-translations";

type Workspace = ReturnType<typeof useScraper>;

function PreviewPoster({ uri, title }: { uri?: string | null; title: string }) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState<string>();
  return (
    <div className="flex aspect-[2/3] w-24 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-border bg-muted sm:w-28">
      {uri?.startsWith("data:image/") && failed !== uri ? (
        <img
          src={uri}
          alt={title}
          className="size-full object-cover"
          onError={() => setFailed(uri)}
        />
      ) : (
        <div className="flex flex-col items-center gap-2 p-3 text-center text-xs text-muted-foreground">
          <ImageOff size={24} aria-hidden="true" />
          {t("noPoster")}
        </div>
      )}
    </div>
  );
}

function FieldChoice({
  row,
  workspace,
  busy,
}: {
  row: MergePreviewRow;
  workspace: Workspace;
  busy: boolean;
}) {
  const { t } = useTranslation();
  return (
    <Label className="flex min-w-0 items-start gap-3 rounded-lg p-2 hover:bg-accent">
      <Checkbox
        className="mt-1"
        checked={row.selected}
        disabled={busy || row.mandatory || !row.eligible}
        onChange={(event) =>
          workspace.chooseFields(
            event.target.checked
              ? [...workspace.selectedFields, row.key]
              : workspace.selectedFields.filter((key) => key !== row.key),
          )
        }
      />
      <span className="min-w-0 flex-1">
        <span className="flex flex-wrap items-center gap-2">
          <span className="font-medium">
            {t(`scraperField_${row.key}`, { defaultValue: row.key })}
          </span>
          {row.language && (
            <span className="text-xs text-muted-foreground">
              {row.language}
            </span>
          )}
          {row.protected && <Badge variant="secondary">{t("locked")}</Badge>}
          {row.mandatory && !row.protected && (
            <Badge variant="outline">{t("mergeRequiredSource")}</Badge>
          )}
        </span>
        <span className="mt-1 line-clamp-2 block break-words text-xs text-muted-foreground">
          {row.result === "removed" ? t("scraperIdRemoved") : value(row.new.value) || t("none")}
        </span>
      </span>
    </Label>
  );
}

export function MetadataMergeWorkspace({
  workspace,
  busy,
  onCancel,
}: {
  workspace: Workspace;
  busy: boolean;
  onCancel: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [tab, setTab] = useState("merged");
  const { metadata, projection, targetDetail, preview, confirm } = workspace;
  if (!metadata || !projection) return null;
  const current = targetDetail.data;
  const primary = projection.rows.filter(
    (row) => row.mandatory || ["title", "overview", "plot"].includes(row.key),
  );
  const more = projection.rows.filter((row) => !primary.includes(row));
  const done = confirm.isSuccess || Boolean(preview?.confirmed);
  const posterIndex = projection.effectiveArtwork.find((index) =>
    ["poster", "cover"].includes(metadata.data.artwork[index]?.kind),
  );
  const poster =
    posterIndex === undefined
      ? current?.poster_uri
      : workspace.thumbnails[posterIndex];
  const overviewChange = projection.rows.find(
    (row) => row.selected && row.result !== "removed" && ["overview", "plot"].includes(row.key),
  );
  const overviews = projection.fields.filter((field) =>
    ["overview", "plot"].includes(field.key),
  );
  const overview = overviewChange
    ? { ...overviewChange.new, value: overviewChange.value }
    : (overviews.find(
        (field) =>
          field.language?.toLowerCase() ===
          i18n.resolvedLanguage?.toLowerCase(),
      ) ?? overviews[0]);
  const overviewRow =
    overview &&
    projection.rows.find(
      (row) =>
        row.key === overview.key &&
        row.language?.toLowerCase() === overview.language?.toLowerCase(),
    );
  const identityChanges = workspace.identityChanges;
  const count = {
    ...projection.counts,
    added: projection.counts.added + identityChanges.filter((id) => !id.old.length).length,
    overwritten: projection.counts.overwritten + identityChanges.filter((id) => id.old.length).length,
    total: projection.counts.total + identityChanges.length,
  };
  const canExportSaved = workspace.validTarget && metadata.kind === workspace.targetKind &&
    count.total === 0 && !workspace.mergePreviewPending && !workspace.mergePreviewError &&
    (!preview || workspace.mergeSelectionValid);
  const exportAvailable = done || canExportSaved;
  return (
    <Card asChild className="overflow-clip rounded-2xl bg-surface/50 p-0 sm:p-0">
      <section aria-label={t("mergeTitle")}>
        <div className="grid min-w-0 lg:grid-cols-[minmax(16rem,0.8fr)_minmax(0,1.7fr)]">
          <div className="min-w-0 border-b border-border lg:border-r lg:border-b-0">
            <section
              className="space-y-3 border-b border-border p-4"
              aria-labelledby="merge-sources"
            >
              <h2
                id="merge-sources"
                className="font-semibold"
              >
                {t("mergeSources")}
              </h2>
              <div className="space-y-1 border-l-2 border-border pl-3 text-sm">
                <h3 className="text-muted-foreground">{t("mergeBase")}</h3>
                <p className="break-words font-medium">
                  {current?.media.title ?? t("scraperAnonymous")}
                </p>
                {current?.metadata_values.length ? (
                  <p className="text-xs text-muted-foreground">
                    {[
                      ...new Set(
                        current.metadata_values
                          .map((field) => field.provider_id)
                          .filter(Boolean),
                      ),
                    ].join(" · ")}
                  </p>
                ) : null}
              </div>
              <div className="space-y-1 border-l-2 border-primary pl-3 text-sm">
                <h3 className="text-muted-foreground">{t("mergeSupplement")}</h3>
                <p className="font-medium">
                  {metadata.data.identity.provider_id}
                </p>
                <p className="break-all text-xs text-muted-foreground">
                  {metadata.data.identity.source_url}
                </p>
              </div>
              <Details className="border-0 p-0">
                <DetailsSummary className="text-xs text-muted-foreground">
                  {t("mergeSourceDetails")}
                </DetailsSummary>
                <VideoSourceDetails metadata={metadata} />
              </Details>
            </section>
          <section
            className="space-y-3 border-b border-border p-4"
            aria-labelledby="merge-fields"
          >
            <h2
              id="merge-fields"
              className="font-semibold"
            >
              {t("mergeFields")}
            </h2>
            <div className="flex flex-wrap gap-2">
              {(["all", "none", "missing"] as const).map((preset) => (
                <Button
                  key={preset}
                  variant="outline"
                  size="sm"
                  disabled={
                    busy ||
                    done ||
                    (preset === "missing" && !workspace.validTarget)
                  }
                  onClick={() => workspace.applyFieldPreset(preset)}
                >
                  {t(`scraperFields_${preset}`)}
                </Button>
              ))}
            </div>
            <div className="space-y-1">
              {primary.map((row) => (
                <FieldChoice
                  key={`${row.key}:${row.language}`}
                  row={row}
                  workspace={workspace}
                  busy={busy || done}
                />
              ))}
            </div>
            {metadata.data.artwork.map((art, index) => (
              <div key={`${art.kind}:${art.url}`} className="space-y-2">
                <Label className="flex items-center gap-3 rounded-lg p-2">
                  <Checkbox
                    checked={projection.effectiveArtwork.includes(index)}
                    disabled={
                      busy ||
                      done ||
                      (workspace.onlyMissing &&
                        Boolean(
                          current?.artworks.some(
                            (item) => item.kind === art.kind,
                          ) ||
                            (current?.poster_uri &&
                              ["poster", "cover"].includes(art.kind)),
                        ))
                    }
                    onChange={(event) => {
                      workspace.chooseArtwork(
                        event.target.checked
                          ? [...workspace.selectedArtwork, index]
                          : workspace.selectedArtwork.filter(
                              (item) => item !== index,
                            ),
                      );
                      if (event.target.checked && !workspace.thumbnails[index])
                        workspace.artworkLoad.mutate(index);
                    }}
                  />
                  <span className="min-w-0 flex-1 break-words text-sm">
                    {t("scraperArtworkItem", {
                      kind: t(`scraperArtwork_${art.kind}`, {
                        defaultValue: art.kind,
                      }),
                      provider: art.source_provider,
                      index: index + 1,
                    })}
                  </span>
                </Label>
                {workspace.thumbnails[index] && (
                  <div className="pl-2">
                    <PreviewPoster
                      uri={workspace.thumbnails[index]}
                      title={t("scraperArtworkAlt", { kind: art.kind })}
                    />
                  </div>
                )}
                {!workspace.thumbnails[index] && (
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy || done}
                    onClick={() => workspace.artworkLoad.mutate(index)}
                  >
                    {t("scraperLoadArtwork")}
                  </Button>
                )}
              </div>
            ))}
            {more.length > 0 && (
              <Details className="border-0 p-0">
                <DetailsSummary>
                  {t("mergeMoreFields", { count: more.length })}
                </DetailsSummary>
                <div className="mt-3 space-y-1">
                  {more.map((row) => (
                    <FieldChoice
                      key={`${row.key}:${row.language}`}
                      row={row}
                      workspace={workspace}
                      busy={busy || done}
                    />
                  ))}
                </div>
              </Details>
            )}
          </section>
          <div className="p-4">
            <Label className="flex items-center gap-3 text-sm">
              <Checkbox
                checked={workspace.onlyMissing}
                disabled={busy || done || !workspace.validTarget}
                onChange={(event) =>
                  workspace.setOnlyMissing(event.target.checked)
                }
              />
              {t(
                projection.rows.some((row) => row.mandatory)
                  ? "mergeOnlyMissingLicensed"
                  : "mergeOnlyMissing",
              )}
            </Label>
          </div>
        </div>
        <div className="min-w-0 p-4 lg:p-5">
          <Tabs value={tab} onValueChange={setTab}>
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <h2 className="font-semibold">{t("mergeLivePreview")}</h2>
              <TabsList aria-label={t("mergePreviewMode")}>
                <TabsTrigger value="merged">{t("mergeAfter")}</TabsTrigger>
                <TabsTrigger value="compare">{t("mergeCompare")}</TabsTrigger>
              </TabsList>
            </div>
            <TabsContent value="merged">
              <div className="flex min-w-0 gap-5">
                <PreviewPoster uri={poster} title={projection.media.title} />
                <div className="min-w-0 space-y-3 py-2">
                  <h3 className="break-words text-xl font-semibold">
                    {projection.media.title}
                  </h3>
                  {projection.media.original_title && (
                    <p className="break-words text-sm text-muted-foreground">
                      {projection.media.original_title}
                    </p>
                  )}
                  <p className="flex flex-wrap gap-x-3 gap-y-1 text-sm text-muted-foreground">
                    <span>
                      {t(
                        metadata.kind === "movie"
                          ? "scraperMovie"
                          : metadata.kind === "tv"
                            ? "scraperTv"
                            : "scraperAnime",
                      )}
                    </span>
                    {projection.media.year !== null && (
                      <span>{projection.media.year}</span>
                    )}
                    <span>{metadata.data.identity.provider_id}</span>
                  </p>
                  {posterIndex !== undefined && !poster && (
                    <p className="text-xs text-muted-foreground">
                      {t("mergePosterNotLoaded")}
                    </p>
                  )}
                </div>
              </div>
              {overview && (
                <Card asChild className="mt-6 rounded-xl p-4 sm:p-4">
                  <article>
                    <div className="mb-3 flex items-center justify-between gap-2">
                      <h3 className="text-sm font-medium">
                        {t(`scraperField_${overview.key}`)}
                      </h3>
                      {overviewRow && (
                        <Badge variant="outline">
                          {t(`mergeResult_${overviewRow.result}`)}
                        </Badge>
                      )}
                    </div>
                    <p className="whitespace-pre-wrap break-words text-sm leading-7 text-muted-foreground">
                      {value(overview.value)}
                    </p>
                  </article>
                </Card>
              )}
              <div className="mt-5 grid gap-4 sm:grid-cols-2">
                {projection.fields
                  .filter(
                    (field) =>
                      ![
                        "title",
                        "original_title",
                        "overview",
                        "plot",
                        "year",
                      ].includes(field.key),
                  )
                  .map((field) => (
                    <div
                      className="min-w-0"
                      key={`${field.key}:${field.language}`}
                    >
                      <h3 className="text-xs text-muted-foreground">
                        {t(`scraperField_${field.key}`, {
                          defaultValue: field.key,
                        })}
                      </h3>
                      <p className="mt-1 whitespace-pre-wrap break-words text-sm">
                        {value(field.value)}
                      </p>
                    </div>
                  ))}
              </div>
            </TabsContent>
            <TabsContent value="compare">
              <div className="grid grid-cols-1 gap-5 sm:grid-cols-2">
                <div className="min-w-0 space-y-3">
                  <h3 className="text-sm text-muted-foreground">
                    {t("scraperOld")}
                  </h3>
                  <p className="break-words text-xl font-semibold">
                    {current?.media.title ?? t("none")}
                  </p>
                  {projection.rows.map((row) => (
                    <div className="min-w-0" key={`${row.key}:${row.language}`}>
                      <p className="text-xs text-muted-foreground">
                        {t(`scraperField_${row.key}`, {
                          defaultValue: row.key,
                        })}
                      </p>
                      <p className="mt-1 whitespace-pre-wrap break-words text-sm">
                        {row.old ? value(row.old.value) : t("none")}
                      </p>
                    </div>
                  ))}
                </div>
                <div className="min-w-0 space-y-3">
                  <h3 className="text-sm text-muted-foreground">
                    {t("mergeAfter")}
                  </h3>
                  <p className="break-words text-xl font-semibold">
                    {projection.media.title}
                  </p>
                  {projection.rows.map((row) => (
                    <div className="min-w-0" key={`${row.key}:${row.language}`}>
                      <p className="text-xs text-muted-foreground">
                        {t(`scraperField_${row.key}`, {
                          defaultValue: row.key,
                        })}
                      </p>
                      <p className="mt-1 whitespace-pre-wrap break-words text-sm">
                        {row.result === "removed" ? t("none") : value(row.value) || t("none")}
                      </p>
                    </div>
                  ))}
                </div>
              </div>
            </TabsContent>
          </Tabs>
          <section
            className="mt-6 border-t border-border pt-5"
            aria-labelledby="merge-changes"
          >
            <h2 id="merge-changes" className="mb-3 font-semibold">
              {t("mergeChanges")}
            </h2>
            {identityChanges.some((id) => id.old.length > 0) && (
              <p className="mb-3 text-sm text-muted-foreground">{t("scraperIdentityCorrection")}</p>
            )}
            <div className="overflow-x-auto">
              <Table className="w-full min-w-[540px] table-fixed text-left text-sm">
                <TableHeader className="text-xs text-muted-foreground">
                  <TableRow>
                    <TableHead className="w-1/5 py-3 pr-3">{t("scraperField")}</TableHead>
                    <TableHead className="w-1/4 py-3 pr-3">{t("scraperOld")}</TableHead>
                    <TableHead className="py-3 pr-3">{t("mergeAfter")}</TableHead>
                    <TableHead className="w-20 py-3">{t("mergeResult")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {identityChanges.map((id) => (
                    <TableRow key={`id:${id.namespace}`} className="border-t border-border align-top">
                      <TableCell className="py-3 pr-3 break-words">
                        {t("scraperExternalIds")}
                        <span className="mt-1 block text-xs text-muted-foreground">{id.namespace}</span>
                      </TableCell>
                      <TableCell className="py-3 pr-3 whitespace-pre-wrap break-all text-muted-foreground">
                        {id.old.join(" · ") || t("none")}
                      </TableCell>
                      <TableCell className="py-3 pr-3 whitespace-pre-wrap break-all">
                        {id.new.join(" · ") || t("none")}
                      </TableCell>
                      <TableCell className="py-3">
                        <Badge>{t(!id.new.length ? "scraperIdRemoved" : !id.old.length ? "mergeResult_added" : "mergeResult_overwritten")}</Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                  {projection.rows.map((row) => (
                    <TableRow
                      className="border-t border-border align-top"
                      key={`${row.key}:${row.language}`}
                    >
                      <TableCell className="py-3 pr-3 break-words">
                        {t(`scraperField_${row.key}`, {
                          defaultValue: row.key,
                        })}
                        {row.language && (
                          <span className="mt-1 block text-xs text-muted-foreground">
                            {row.language}
                          </span>
                        )}
                      </TableCell>
                      <TableCell className="py-3 pr-3 whitespace-pre-wrap break-words text-muted-foreground">
                        {row.old ? value(row.old.value) : t("none")}
                      </TableCell>
                      <TableCell className="py-3 pr-3 whitespace-pre-wrap break-words">
                        {row.result === "removed" ? t("none") : value(row.value) || t("none")}
                      </TableCell>
                      <TableCell className="py-3">
                        <Badge
                          variant={
                            row.result === "added" ||
                            row.result === "overwritten" || row.result === "removed"
                              ? "default"
                              : "outline"
                          }
                        >
                          {row.protected
                            ? t("locked")
                            : row.result === "removed" ? t("scraperIdRemoved") : t(`mergeResult_${row.result}`)}
                        </Badge>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          </section>
        </div>
      </div>
      <footer className="flex flex-wrap items-center justify-between gap-4 border-t border-border bg-surface px-4 py-4 sm:px-5">
        <div className="min-w-0 space-y-1" role="status" aria-live="polite">
          {done ? (
            <Badge variant="secondary">{t("reviewState_confirmed")}</Badge>
          ) : (
            <p className="text-sm font-medium">
              {t("mergeSummary", { ...count, count: count.total })}
            </p>
          )}
          {!done && workspace.mergePreviewPending && (
            <p className="text-xs text-muted-foreground">
              {t("mergeValidating")}
            </p>
          )}
          {!done && !count.total && (
            <p className="text-xs text-muted-foreground">
              {t("mergeNoChanges")}
            </p>
          )}
          {!done && canExportSaved && (
            <p className="text-xs text-muted-foreground">{t("mergeExportSavedHelp")}</p>
          )}
          {workspace.validTarget && metadata.kind !== workspace.targetKind && (
            <p role="alert" className="text-sm text-destructive">
              {t("scraperTypeMismatch")}
            </p>
          )}
          {!workspace.validTarget && (
            <p className="text-xs text-muted-foreground">
              {t("scraperSelectBeforeMerge")}
            </p>
          )}
        </div>
        <div className="flex flex-wrap gap-3">
          {exportAvailable && (
            <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
                <span>{t("scraperOutputMode")}</span>
                <Select
                  value={workspace.outputModeReady ? workspace.outputMode : ""}
                  aria-busy={workspace.outputModeLoading || undefined}
                  disabled={
                    workspace.write.isPending ||
                    workspace.discard.isPending ||
                    workspace.confirm.isPending
                  }
                  onChange={(event) =>
                    workspace.setOutputMode(event.target.value as NfoExportMode)
                  }
                >
                  {!workspace.outputModeReady && <option value="" disabled>{t(workspace.outputModeLoading ? "loading" : "none")}</option>}
                  <option value="online">{t("scraperOutputOnline")}</option>
                  <option value="offline">{t("scraperOutputOffline")}</option>
                </Select>
            </Label>
          )}
          <Button variant="outline" disabled={busy} onClick={onCancel}>
            {t(done ? "mergeDone" : "cancel")}
          </Button>
          {exportAvailable && (
            <Button
              variant="outline"
              disabled={busy || !workspace.outputModeReady}
              onClick={() => workspace.outputPlan.mutate()}
            >
              {t(done ? "scraperOutputPreview" : "mergeExportSavedPreview")}
            </Button>
          )}
          {!done && (
              <Button
                disabled={
                  busy ||
                  !preview ||
                  !workspace.mergeSelectionValid ||
                  (!preview.fields.some(
                    (field) => field.selected && !field.protected,
                  ) &&
                    !preview.artwork.length && !identityChanges.length) ||
                  workspace.mergePreviewPending ||
                  Boolean(workspace.mergePreviewError) ||
                  count.total === 0
                }
                onClick={() => confirm.mutate()}
              >
                {t("mergeConfirm", { count: count.total })}
              </Button>
          )}
        </div>
        {exportAvailable && workspace.outputModeError && (
          <div className="w-full">
            <ErrorNotice error={workspace.outputModeError} retry={workspace.retryOutputMode} />
          </div>
        )}
        {preview && !done && !workspace.mergeSelectionValid && (
          <div role="alert" className="w-full text-sm">
            <p>{t("mergeChanged")}</p>
            <Button
              className="mt-3"
              size="sm"
              variant="outline"
              disabled={busy || workspace.mergePreviewPending}
              onClick={workspace.retryMergePreview}
            >
              {t("mergeRefresh")}
            </Button>
          </div>
        )}
        {Boolean(workspace.mergePreviewError) && (
          <div className="w-full">
            <ErrorNotice
              error={workspace.mergePreviewError}
              retry={workspace.retryMergePreview}
            />
          </div>
        )}
      </footer>
    </section>
    </Card>
  );
}
