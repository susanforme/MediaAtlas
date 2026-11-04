import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { ScanTask } from "../../api/library";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { refreshCounts, refreshResultTitle } from "./library-activity-model";
import { useFormat } from "./shared";
import "./translations";
import "./library-status-translations";

/** Local discovery and remote metadata completion have separate outcomes. */
export function AutomaticScanResult({
  task,
  adult = false,
}: {
  task: ScanTask;
  adult?: boolean;
}) {
  const { t } = useTranslation();
  const { number } = useFormat();
  const counts = refreshCounts(task);
  const scope = adult ? { kind: "adult_video" as const } : {};
  const metrics = [
    ["autoIndexed", task.imported_count],
    ["autoMatched", task.matched_count ?? 0],
    ["autoNeedsReview", counts.review],
    ["autoMetadataFailed", counts.failed],
    ...(counts.unconfigured > 0
      ? [["libraryRefreshWaitingSource", counts.unconfigured] as const]
      : []),
  ] as const;
  return (
    <Card asChild>
      <section className="space-y-5">
        <p role="status">{t(refreshResultTitle(task))}</p>
        <dl className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          {metrics.map(([label, count]) => (
            <div key={label}>
              <dt className="text-sm text-muted-foreground">{t(label)}</dt>
              <dd className="mt-1 text-xl font-semibold">{number(count)}</dd>
            </div>
          ))}
        </dl>
        {counts.unconfigured > 0 && (
          <p className="text-sm text-muted-foreground">
            {t("libraryRefreshSourceRequired")}
          </p>
        )}
        <div className="flex flex-wrap gap-3">
          {counts.unconfigured > 0 && (
            <Button asChild>
              <Link
                to="/settings"
                search={{ from: "library", root: task.root_id, ...scope }}
              >
                {t("libraryRefreshConfigureAction")}
              </Link>
            </Button>
          )}
          <Button
            asChild
            variant={counts.unconfigured > 0 ? "outline" : "default"}
          >
            <Link to="/library" search={{ root: task.root_id, ...scope }}>
              {t("autoBrowseLibrary")}
            </Link>
          </Button>
          {counts.review > 0 && (
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
          {counts.failed > 0 && (
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
      </section>
    </Card>
  );
}
