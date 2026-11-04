import { useTranslation } from "react-i18next";
import type { SearchPreview } from "../../api/scraper";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { searchRecoveries } from "./recovery-model";
import "./recovery-translations";

const providerNames: Record<string, string> = {
  tvmaze: "TVmaze",
  tmdb: "TMDB",
  anilist: "AniList",
  bangumi: "Bangumi",
};

export function ScraperRecovery({
  search,
  error,
  busy = false,
  onRetry,
  onEdit,
  onConfigure,
}: {
  search?: SearchPreview;
  error?: unknown;
  busy?: boolean;
  onRetry?: () => void;
  onEdit?: () => void;
  onConfigure?: (providerId?: string) => void;
}) {
  const { t } = useTranslation();
  const recoveries = searchRecoveries(search, error);
  if (recoveries.length === 0) return null;
  const desktop = recoveries.some((item) => item.kind === "desktop");
  const configure = recoveries
    .filter(
      (item) => item.kind === "credentials" || item.kind === "configuration",
    )
    .filter(
      (item, index, items) =>
        items.findIndex((other) => other.providerId === item.providerId) ===
        index,
    );
  return (
    <Alert asChild>
      <section
        className="mt-4 space-y-3"
        role={
          recoveries.every((item) => item.kind === "no_results")
            ? "status"
            : "alert"
        }>
        {recoveries.map((item) => {
          const name =
            item.providerId && Object.hasOwn(providerNames, item.providerId)
              ? providerNames[item.providerId]
              : undefined;
          return (
            <p key={`${item.kind}:${item.providerId ?? ""}`}>
              {name && <span>{name}: </span>}
              {t(`scraperRecovery_${item.kind}`)}
            </p>
          );
        })}
        {!desktop &&
          (onEdit || onRetry || (onConfigure && configure.length > 0)) && (
            <div className="flex flex-wrap gap-2">
              {onConfigure &&
                configure.map((item) => (
                  <Button
                    key={item.providerId ?? "source"}
                    type="button"
                    variant="outline"
                    disabled={busy}
                    onClick={() => onConfigure(item.providerId)}
                  >
                    {t("scraperRecoveryConfigure")}
                    {item.providerId &&
                    Object.hasOwn(providerNames, item.providerId)
                      ? ` · ${providerNames[item.providerId]}`
                      : ""}
                  </Button>
                ))}
              {onEdit && (
                <Button
                  type="button"
                  variant="outline"
                  disabled={busy}
                  onClick={onEdit}
                >
                  {t("scraperRecoveryEdit")}
                </Button>
              )}
              {onRetry && (
                <Button type="button" disabled={busy} onClick={onRetry}>
                  {t("scraperRecoveryRetry")}
                </Button>
              )}
            </div>
          )}
      </section>
    </Alert>
  );
}
