import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { validateProviderSetupSearch } from "../settings/setup-search";
import type { LibrarySearch } from "./browse-model";
import { refreshCounts } from "./library-activity-model";
import type { useLibraryActivity } from "./use-library-activity";
import "./library-status-translations";

export function LibraryActivity({
  activity,
  search,
}: {
  activity: ReturnType<typeof useLibraryActivity>;
  search?: LibrarySearch;
}) {
  const { t } = useTranslation();
  const task = activity.active[0] ?? activity.attention[0];
  if (!task) return null;
  const running = activity.active.length > 0;
  const visibleTasks = running ? activity.active : activity.attention;
  const incomplete = visibleTasks.reduce(
    (total, entry) => {
      const counts = refreshCounts(entry);
      return {
        review: total.review + counts.review,
        unconfigured: total.unconfigured + counts.unconfigured,
        failed: total.failed + counts.failed,
      };
    },
    { review: 0, unconfigured: 0, failed: 0 },
  );
  const count = (
    key:
      | "imported_count"
      | "matched_count"
      | "review_count"
      | "metadata_failed_count",
  ) => visibleTasks.reduce((total, entry) => total + (entry[key] ?? 0), 0);
  const title = running
    ? visibleTasks.length > 1
      ? "libraryRefreshActive"
      : task.phase === "scan"
        ? "libraryRefreshScanning"
        : task.phase === "index"
          ? "libraryRefreshIndexing"
          : task.phase === "metadata"
            ? "libraryRefreshMetadata"
            : "libraryRefreshActive"
    : incomplete.unconfigured > 0
      ? "libraryRefreshUnconfigured"
      : "libraryRefreshIncomplete";
  return (
    <Alert asChild>
      <section
        className="mb-6 flex flex-wrap items-center justify-between gap-4"
        aria-label={t(title)}>
        <div role="status" aria-live="polite">
          <p className="font-medium">{t(title)}</p>
          <ul className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-sm text-muted-foreground">
            {activity.active.length > 1 && (
              <li>
                {t("libraryRefreshFolders", { count: activity.active.length })}
              </li>
            )}
            {running && visibleTasks.length === 1 && task.phase === "scan" && (
              <li>{t("libraryRefreshFiles", { count: task.visited_files })}</li>
            )}
            <li>
              {t("libraryRefreshAdded", { count: count("imported_count") })}
            </li>
            <li>
              {t("libraryRefreshMatched", { count: count("matched_count") })}
            </li>
            {incomplete.unconfigured > 0 && (
              <li>
                {t("libraryRefreshUnconfiguredCount", {
                  count: incomplete.unconfigured,
                })}
              </li>
            )}
            {incomplete.review > 0 && (
              <li>{t("libraryRefreshReview", { count: incomplete.review })}</li>
            )}
            {count("metadata_failed_count") > 0 && (
              <li>
                {t("libraryRefreshFailed", {
                  count: count("metadata_failed_count"),
                })}
              </li>
            )}
          </ul>
        </div>
        <div className="flex flex-wrap gap-2">
          {!running && incomplete.unconfigured > 0 && (
            <Button asChild size="sm">
              <Link
                to="/settings"
                search={validateProviderSetupSearch({
                  from: "library",
                  ...(visibleTasks.length === 1 ? { root: task.root_id } : {}),
                  ...(search ? { return: { to: "library", search } } : {}),
                })}
              >
                {t(
                  visibleTasks.length === 1
                    ? "libraryRefreshConfigureAction"
                    : "libraryRefreshConfigureOnly",
                )}
              </Link>
            </Button>
          )}
          {!running && incomplete.review > 0 && (
            <Button asChild variant="outline" size="sm">
              <Link
                to="/tools"
                search={{
                  ...(visibleTasks.length === 1 ? { root: task.root_id } : {}),
                  health: "metadata_needs_review",
                }}
              >
                {t("libraryRefreshReviewAction")}
              </Link>
            </Button>
          )}
          {!running && incomplete.failed > 0 && (
            <Button asChild variant="outline" size="sm">
              <Link
                to="/tools"
                search={{
                  ...(visibleTasks.length === 1 ? { root: task.root_id } : {}),
                  health: "metadata_failed",
                }}
              >
                {t("libraryRefreshFailedAction")}
              </Link>
            </Button>
          )}
          <Button asChild variant="outline" size="sm">
            <Link
              to="/scanner"
              search={
                visibleTasks.length === 1
                  ? { root: task.root_id, task: task.id }
                  : {}
              }
            >
              {t(running ? "libraryRefreshProgress" : "libraryRefreshRetry")}
            </Link>
          </Button>
        </div>
      </section>
    </Alert>
  );
}
