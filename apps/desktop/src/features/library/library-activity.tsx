import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import type { useLibraryActivity } from "./use-library-activity";
import "./library-status-translations";

export function LibraryActivity({
  activity,
}: {
  activity: ReturnType<typeof useLibraryActivity>;
}) {
  const { t } = useTranslation();
  const task = activity.active[0] ?? activity.attention[0];
  if (!task) return null;
  const running = activity.active.length > 0;
  const visibleTasks = running ? activity.active : activity.attention;
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
    : "libraryRefreshAttention";
  return (
    <section
      className="notice mb-6 flex flex-wrap items-center justify-between gap-4"
      aria-label={t("libraryRefreshActive")}
    >
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
          {count("review_count") > 0 && (
            <li>
              {t("libraryRefreshReview", { count: count("review_count") })}
            </li>
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
        {!running && count("review_count") > 0 && (
          <Button asChild variant="outline" size="sm">
            <Link
              to="/tools"
              search={visibleTasks.length === 1 ? { root: task.root_id } : {}}
            >
              {t("libraryRefreshReviewAction")}
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
  );
}
