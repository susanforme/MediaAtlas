import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { ScanTask } from "../../api/library";
import { Button } from "../../components/ui/button";
import { useFormat } from "./shared";
import "./translations";

/** Automatic discovery publishes local media without an import-selection step. */
export function AutomaticScanResult({
  task,
  adult = false,
}: {
  task: ScanTask;
  adult?: boolean;
}) {
  const { t } = useTranslation();
  const { number } = useFormat();
  const running = task.state === "queued" || task.state === "running";
  const needsAttention =
    (task.review_count ?? 0) > 0 || (task.metadata_failed_count ?? 0) > 0;
  const scope = adult ? { kind: "adult_video" as const } : {};
  return (
    <section className="panel space-y-5">
      <p role="status">
        {t(
          running
            ? "autoRefreshRunning"
            : task.state === "succeeded" || task.state === "waiting_for_user"
              ? needsAttention
                ? "autoRefreshReview"
                : "autoRefreshComplete"
              : `state_${task.state}`,
        )}
      </p>
      <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
        {(
          [
            ["autoIndexed", task.imported_count],
            ["autoMatched", task.matched_count ?? 0],
            ["autoNeedsReview", task.review_count ?? 0],
            ["autoMetadataFailed", task.metadata_failed_count ?? 0],
          ] as const
        ).map(([label, count]) => (
          <div key={label}>
            <dt className="text-sm text-muted-foreground">{t(label)}</dt>
            <dd className="mt-1 text-xl font-semibold">{number(count)}</dd>
          </div>
        ))}
      </dl>
      <div className="flex flex-wrap gap-3">
        <Button asChild>
          <Link to="/library" search={{ root: task.root_id, ...scope }}>
            {t("autoBrowseLibrary")}
          </Link>
        </Button>
        {(task.review_count ?? 0) > 0 && (
          <Button asChild variant="outline">
            <Link
              to={adult ? "/library" : "/tools"}
              search={{
                root: task.root_id,
                health: "metadata_needs_review",
                ...scope,
              }}
            >
              {t("autoReviewItems")}
            </Link>
          </Button>
        )}
        {(task.metadata_failed_count ?? 0) > 0 && (
          <Button asChild variant="outline">
            <Link
              to={adult ? "/library" : "/tools"}
              search={{
                root: task.root_id,
                health: "metadata_failed",
                ...scope,
              }}
            >
              {t("autoFailedItems")}
            </Link>
          </Button>
        )}
      </div>
      {needsAttention && (
        <div className="space-y-3 border-t border-border pt-4">
          <p className="text-sm text-muted-foreground">
            {t("autoConfigurationHint")}
          </p>
          <div className="flex flex-wrap gap-3">
            <Button asChild variant="outline">
              <Link
                to={adult ? "/library" : "/tools"}
                search={{
                  root: task.root_id,
                  health: "metadata_unconfigured",
                  ...scope,
                }}
              >
                {t("autoUnconfiguredItems")}
              </Link>
            </Button>
            <Button asChild variant="outline">
              {adult ? (
                <Link to="/tools" search={{ kind: "adult_video" }}>
                  {t("autoConfigureSources")}
                </Link>
              ) : (
                <Link to="/providers">{t("autoConfigureSources")}</Link>
              )}
            </Button>
            {!adult && (
              <Button asChild variant="outline">
                <Link to="/settings">{t("autoConfigureToken")}</Link>
              </Button>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
