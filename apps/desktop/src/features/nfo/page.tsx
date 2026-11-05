import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useNavigate } from "@tanstack/react-router";
import { FolderOpen } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type NfoDocument, nfoApi } from "../../api/nfo";
import { useWindowChrome } from "../../components/shell/use-window-controls";
import { WindowControls } from "../../components/shell/window-controls";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Details, DetailsSummary } from "../../components/ui/details";
import { MediaDetailView } from "../library/media-detail-view";
import { Empty, ErrorNotice, Loading, PageHeading } from "../library/shared";
import { nfoDetail } from "./detail-model";
import "./translations";

export function NfoPage() {
  const { t, i18n } = useTranslation();
  const [document, setDocument] = useState<NfoDocument | null>(null);
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  const version = useRef(0);

  const read = useCallback(
    async (load: () => Promise<NfoDocument | null>) => {
      const current = ++version.current;
      setBusy(true);
      setError(undefined);
      try {
        const next = await load();
        if (current === version.current && next) setDocument(next);
      } catch (error) {
        if (current === version.current) {
          setDocument(null);
          setError(error);
        }
      } finally {
        if (current === version.current) setBusy(false);
      }
    },
    [],
  );

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let reading = false;
    let replay = false;
    async function readOpened() {
      replay = true;
      if (reading) return;
      reading = true;
      while (replay && !disposed) {
        replay = false;
        await read(nfoApi.takeOpened);
      }
      reading = false;
    }
    void listen("nfo-opened", () => {
      if (!disposed) void readOpened();
    })
      .then((stop) => {
        if (disposed) stop();
        else {
          unlisten = stop;
          void readOpened();
        }
      })
      .catch((error) => {
        if (!disposed) setError(error);
      });
    return () => {
      disposed = true;
      version.current++;
      unlisten?.();
    };
  }, [read]);

  const detail = document ? nfoDetail(document) : null;
  const open = (
    <Button
      disabled={busy}
      onClick={() => void read(() => nfoApi.open(i18n.resolvedLanguage === "en-US" ? "en-US" : "zh-CN"))}
    >
      <FolderOpen size={17} aria-hidden="true" />
      {t("nfoOpen")}
    </Button>
  );
  return (
    <div className="min-w-0 space-y-6">
      {document?.format !== "xml" && (
        <>
          <PageHeading title={document?.name ?? t("nfoViewer")}>{open}</PageHeading>
          <p className="text-sm text-muted-foreground">{t("nfoReadOnly")}</p>
        </>
      )}
      {busy && <Loading />}
      {error !== undefined && <ErrorNotice error={error} />}
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
              genres={detail.genres}
              overview={detail.overview}
              navigation={<div className="mb-6 flex flex-wrap items-center gap-3">{open}{document.name !== detail.title && <span className="text-sm text-muted-foreground">{document.name}</span>}</div>}
              actions={<p className="text-sm text-muted-foreground">{t("nfoReadOnly")}</p>}
              facts={[
                ...detail.fields.map((field) => ({ ...field, label: t(`nfoField_${field.key}`, { defaultValue: field.key }) })),
                ...document.metadata.external_ids.filter((id) => id.value.trim()).map((id) => ({ key: `${id.provider_id}:${id.namespace}:${id.value}`, label: `${id.provider_id} / ${id.namespace}`, value: id.value })),
              ]}
            >
              <Details>
                <DetailsSummary>{t("nfoSource")}</DetailsSummary>
                <pre className="mt-4 max-h-[60vh] overflow-auto whitespace-pre-wrap wrap-anywhere text-sm">{document.source}</pre>
              </Details>
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
  const chrome = useWindowChrome();
  return (
    <div className="min-h-screen">
      <div
        className="relative h-10"
        data-tauri-drag-region={chrome.platform ? "deep" : undefined}
      >
        <WindowControls chrome={chrome} />
      </div>
      <main className="mx-auto max-w-[1440px] px-6 pb-6 sm:px-10 sm:pb-10">
        <NfoPage />
      </main>
    </div>
  );
}

export function NfoOpenListener() {
  const navigate = useNavigate();
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    function show() {
      if (!disposed) void navigate({ to: "/nfo" });
    }
    void listen("nfo-opened", show)
      .then(async (stop) => {
        if (disposed) {
          stop();
          return;
        }
        unlisten = stop;
        if (await nfoApi.hasOpened()) show();
      })
      .catch(() => { /* The preview page owns file-read errors. */ });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [navigate]);
  return null;
}
