import { isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useNavigate } from "@tanstack/react-router";
import { FileText, FolderOpen } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { type NfoDocument, nfoApi } from "../../api/nfo";
import { useWindowChrome } from "../../components/shell/use-window-controls";
import { WindowControls } from "../../components/shell/window-controls";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Details, DetailsSummary } from "../../components/ui/details";
import { Empty, ErrorNotice, Loading, PageHeading } from "../library/shared";
import "./translations";

function text(value: unknown): string {
  if (typeof value === "string" || typeof value === "number") return String(value);
  if (Array.isArray(value)) return value.map(text).join(" · ");
  return JSON.stringify(value) ?? "";
}

function castText(value: unknown): string {
  if (Array.isArray(value)) return value.map(castText).filter(Boolean).join(" · ");
  if (typeof value === "string") return value;
  if (value !== null && typeof value === "object" && "name" in value && typeof value.name === "string") {
    const role = "role" in value && typeof value.role === "string" ? value.role : "";
    return role ? `${value.name} (${role})` : value.name;
  }
  return "";
}

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

  const fields = document?.metadata.fields ?? {};
  return (
    <div className="min-w-0 space-y-6">
      <PageHeading title={document?.name ?? t("nfoViewer")}>
        <Button
          disabled={busy}
          onClick={() =>
            void read(() =>
              nfoApi.open(i18n.resolvedLanguage === "en-US" ? "en-US" : "zh-CN"),
            )
          }
        >
          <FolderOpen size={17} aria-hidden="true" />
          {t("nfoOpen")}
        </Button>
      </PageHeading>
      <p className="text-sm text-muted-foreground">{t("nfoReadOnly")}</p>
      {busy && <Loading />}
      {error !== undefined && <ErrorNotice error={error} />}
      {!document && !busy && error === undefined && (
        <Empty title={t("nfoEmpty")} />
      )}
      {document && (
        <>
          {document.format === "xml" ? (
            <Card className="space-y-6">
              <div className="flex items-start gap-4">
                <FileText className="mt-1 shrink-0 text-muted-foreground" aria-hidden="true" />
                <div className="min-w-0 space-y-2">
                  <h2 className="wrap-anywhere text-2xl font-semibold">
                    {text(fields.title) || document.name}
                  </h2>
                  {fields.original_title != null && (
                    <p className="text-muted-foreground">
                      {text(fields.original_title)}
                    </p>
                  )}
                  <div className="flex flex-wrap gap-2">
                    {fields.year != null && (
                      <Badge variant="secondary">{text(fields.year)}</Badge>
                    )}
                    {Array.isArray(fields.genres) &&
                      [...new Set(fields.genres.map(text))].map((genre) => (
                        <Badge variant="outline" key={genre}>
                          {genre}
                        </Badge>
                      ))}
                  </div>
                </div>
              </div>
              {fields.overview != null && (
                <p className="whitespace-pre-wrap wrap-anywhere leading-7">
                  {text(fields.overview)}
                </p>
              )}
              <dl className="grid gap-x-8 gap-y-4 sm:grid-cols-2">
                {Object.entries(fields)
                  .filter(([key]) =>
                    !["title", "original_title", "year", "overview", "genres"].includes(key) &&
                    !(key === "people" && Array.isArray(fields.cast) && fields.cast.length > 0),
                  )
                  .map(([key, value]) => (
                    <div key={key} className="min-w-0">
                      <dt className="text-sm text-muted-foreground">
                        {t(`nfoField_${key}`, { defaultValue: key })}
                      </dt>
                      <dd className="mt-1 whitespace-pre-wrap wrap-anywhere">
                        {key === "cast" ? castText(value) : text(value)}
                      </dd>
                    </div>
                  ))}
              </dl>
              {document.metadata.external_ids.length > 0 && (
                <dl className="grid gap-3 border-t border-border pt-4 text-sm">
                  {document.metadata.external_ids.map((id) => (
                    <div key={`${id.provider_id}:${id.namespace}:${id.value}`} className="min-w-0">
                      <dt className="text-muted-foreground">{id.provider_id}</dt>
                      <dd className="wrap-anywhere">{id.value}</dd>
                    </div>
                  ))}
                </dl>
              )}
            </Card>
          ) : (
            <Card>
              <pre className="overflow-x-auto whitespace-pre-wrap wrap-anywhere text-sm">
                {document.source}
              </pre>
            </Card>
          )}
          {document.format === "xml" && (
            <Details>
              <DetailsSummary>{t("nfoSource")}</DetailsSummary>
              <pre className="mt-4 max-h-[60vh] overflow-auto whitespace-pre-wrap wrap-anywhere text-sm">
                {document.source}
              </pre>
            </Details>
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
      <main className="mx-auto max-w-5xl px-6 pb-6 sm:px-10 sm:pb-10">
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
