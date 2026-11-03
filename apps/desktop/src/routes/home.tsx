import { useQuery } from "@tanstack/react-query";
import { CircleCheck, CircleHelp, LoaderCircle, RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { DesktopError, getAppInfo } from "../api/desktop";
import { Button } from "../components/ui/button";

export function HomePage() {
  const { t } = useTranslation();
  const info = useQuery({
    queryKey: ["app-info", 1],
    queryFn: getAppInfo,
    retry: false,
    refetchOnWindowFocus: false,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const error = info.error instanceof DesktopError ? info.error : undefined;
  const desktopRequired = error?.code === "desktop_required";
  const status = info.isPending
    ? t("loading")
    : info.isSuccess
      ? t("connected")
      : desktopRequired
        ? t("desktopRequired")
        : error?.code === "invalid_response"
          ? t("invalidResponse")
          : t("requestFailed");

  return (
    <main className="mx-auto w-full max-w-4xl px-6 py-14 sm:px-12 sm:py-20">
      <p className="mb-4 text-sm font-medium tracking-wide text-primary">
        {t("phase")}
      </p>
      <h1 className="text-3xl font-semibold tracking-tight sm:text-4xl">
        {t("title")}
      </h1>
      <p className="mt-4 text-base text-muted-foreground">{t("subtitle")}</p>

      <section
        className="mt-10 rounded-2xl border border-border bg-white p-6 shadow-sm sm:p-8"
        aria-labelledby="connection-title"
      >
        <h2 id="connection-title" className="text-lg font-semibold">
          {t("cardTitle")}
        </h2>
        <p className="mt-2 text-sm text-muted-foreground">
          {t("cardDescription")}
        </p>
        <div
          className="mt-7 flex items-start gap-3 rounded-xl bg-muted p-4"
          role="status"
          aria-live="polite"
        >
          {info.isPending ? (
            <LoaderCircle
              className="mt-0.5 size-5 shrink-0 animate-spin text-muted-foreground"
              aria-hidden="true"
            />
          ) : info.isSuccess ? (
            <CircleCheck
              className="mt-0.5 size-5 shrink-0 text-primary"
              aria-hidden="true"
            />
          ) : (
            <CircleHelp
              className="mt-0.5 size-5 shrink-0 text-muted-foreground"
              aria-hidden="true"
            />
          )}
          <div>
            <p className="text-sm font-medium">{status}</p>
            {desktopRequired && (
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                {t("desktopDescription")}
              </p>
            )}
            {info.isSuccess && (
              <p className="mt-2 text-sm text-muted-foreground">
                {t("version")} {info.data.app_version}
              </p>
            )}
            {error?.envelope && (
              <p className="mt-2 break-all text-xs text-muted-foreground">
                {t("reference")}: {error.envelope.correlation_id}
              </p>
            )}
          </div>
        </div>
        <Button
          className="mt-5"
          variant="outline"
          disabled={info.isFetching}
          onClick={() => void info.refetch()}
        >
          <RefreshCw className="size-4" aria-hidden="true" />
          {info.isFetching ? t("checking") : t("retry")}
        </Button>
      </section>
      <p className="mt-6 max-w-2xl text-sm leading-6 text-muted-foreground">
        {t("scope")}
      </p>
    </main>
  );
}
