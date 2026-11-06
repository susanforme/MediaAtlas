import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type NfoDocument, nfoApi } from "../../api/nfo";
import { Card } from "../../components/ui/card";
import { Details, DetailsSummary } from "../../components/ui/details";
import { MediaArtwork } from "../library/media-artwork";
import { MediaDetailSections, MediaDetailView } from "../library/media-detail-view";
import { MediaCast, MediaMetadataFields } from "../library/media-metadata";
import { Empty, ErrorNotice, Loading, PageHeading } from "../library/shared";
import { nfoDetail } from "./detail-model";
import "./translations";

export function NfoPage() {
  const { t } = useTranslation();
  const [document, setDocument] = useState<NfoDocument | null>(null);
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  const opened = useRef<Promise<NfoDocument | null> | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    setBusy(true);
    setError(undefined);
    const pending = opened.current ??= nfoApi.takeOpened();
    void pending.then((next) => {
      if (!disposed) setDocument(next);
    })
      .catch((error) => {
        if (!disposed) setError(error);
      })
      .finally(() => {
        if (!disposed) setBusy(false);
      });
    return () => {
      disposed = true;
    };
  }, [attempt]);

  const detail = document ? nfoDetail(document) : null;
  return (
    <div className="min-w-0 space-y-6">
      {document?.format !== "xml" && (
          <PageHeading title={document?.name ?? t("nfoViewer")} />
      )}
      {busy && <Loading />}
      {error !== undefined && <ErrorNotice error={error} retry={() => { opened.current = null; setAttempt((value) => value + 1); }} />}
      {!document && !busy && error === undefined && (
        <Empty title={t("nfoEmpty")} />
      )}
      {document && detail && (
        <>
          {document.format === "xml" ? (
            <MediaDetailView
              key={document.name}
              title={detail.title}
              originalTitle={detail.originalTitle}
              year={detail.year}
              runtime={detail.runtime}
              rating={detail.rating}
              poster={document.poster_uri ? { uri: document.poster_uri } : undefined}
              backdrop={document.backdrop_uri}
              genres={detail.genres}
              overview={detail.overview}
              facts={[
                ...detail.facts.map((field) => ({ ...field, label: t(`nfoField_${field.key}`, { defaultValue: field.key }) })),
                ...(detail.cast.length ? [{ key: "cast", label: t("videoDetailCast"), value: detail.cast.slice(0, 4).join(" / ") }] : []),
                ...document.metadata.external_ids.filter((id) => id.value.trim()).map((id) => ({ key: `${id.provider_id}:${id.namespace}:${id.value}`, label: `${id.provider_id} / ${id.namespace}`, value: id.value })),
              ]}
            >
              <MediaDetailSections initialTab="metadata" sections={{
                cast: detail.cast.length ? <MediaCast names={detail.cast} /> : undefined,
                metadata: <>
                  <MediaMetadataFields fields={detail.fields.map((field) => ({ ...field, field: field.key, label: t(`nfoField_${field.key}`, { defaultValue: field.key }) }))} />
                  <Details className="mt-6">
                    <DetailsSummary>{t("nfoSource")}</DetailsSummary>
                    <pre className="mt-4 max-h-[60vh] overflow-auto whitespace-pre-wrap wrap-anywhere text-sm">{document.source}</pre>
                  </Details>
                </>,
                artwork: document.poster_uri || document.backdrop_uri ? (
                  <div className="grid items-start gap-6 sm:grid-cols-2">
                    {document.poster_uri && <figure><div className="aspect-2/3 max-w-[320px] overflow-hidden rounded-xl"><MediaArtwork uri={document.poster_uri} title={detail.title} /></div><figcaption className="mt-2 text-sm text-muted-foreground">{t("videoDetailPoster")}</figcaption></figure>}
                    {document.backdrop_uri && <figure><div className="aspect-video overflow-hidden rounded-xl [&_img]:object-contain"><MediaArtwork uri={document.backdrop_uri} title={detail.title} /></div><figcaption className="mt-2 text-sm text-muted-foreground">{t("nfoBackdrop")}</figcaption></figure>}
                  </div>
                ) : undefined,
              }} />
            </MediaDetailView>
          ) : (
            <Card>
              <pre className="overflow-x-auto whitespace-pre-wrap wrap-anywhere text-sm">
                {document.source}
              </pre>
            </Card>
          )}
        </>
      )}
    </div>
  );
}

export function NfoStandalone() {
  return (
    <main className="mx-auto min-h-screen max-w-[1440px] overflow-x-clip px-6 py-8 sm:px-10 sm:py-10">
      <NfoPage />
    </main>
  );
}
