import { Link } from "@tanstack/react-router";
import {
  Clapperboard,
  Film,
  FolderOpen,
  Music2,
  Pencil,
  RefreshCw,
  Trash2,
  Tv2,
} from "lucide-react";
import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";
import type { RootView, ScanTask } from "../../api/library";
import { Button } from "../../components/ui/button";
import { useFormat } from "./shared";

export function RootCard({
  root,
  task,
  refreshing,
  disabled,
  onEdit,
  onRemove,
  onRefresh,
}: {
  root: RootView;
  task?: ScanTask;
  refreshing: boolean;
  disabled: boolean;
  onEdit: () => void;
  onRemove: (event: MouseEvent<HTMLButtonElement>) => void;
  onRefresh: () => void;
}) {
  const { t } = useTranslation();
  const { date } = useFormat();
  const active = task?.state === "queued" || task?.state === "running";
  const attention =
    task &&
    ["waiting_for_user", "failed", "cancelled", "interrupted"].includes(
      task.state,
    );
  const status = !root.enabled
    ? "disabled"
    : active
      ? `autoPhase_${task.phase ?? "scan"}`
      : attention
        ? "rootNeedsAttention"
        : task?.state === "succeeded" || root.last_scan_at
          ? "rootUpToDate"
          : "neverScanned";
  const Icon =
    root.kind === "movie"
      ? Film
      : root.kind === "tv_series"
        ? Tv2
        : root.kind === "anime"
          ? Clapperboard
          : root.kind === "music_release"
            ? Music2
            : FolderOpen;
  const scope =
    root.kind === "adult_video" ? { kind: "adult_video" as const } : {};
  return (
    <article className="root-card">
      <div className="flex items-start gap-3">
        <div className="rounded-xl bg-primary/10 p-3 text-primary">
          <Icon size={22} aria-hidden="true" />
        </div>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-base font-semibold">
            <Button
              asChild
              variant="link"
              className="h-auto max-w-full justify-start truncate p-0 text-left text-foreground"
            >
              <Link to="/library" search={{ root: root.id, ...scope }}>
                {root.name}
              </Link>
            </Button>
          </h2>
          <p className="mt-1 text-xs text-muted-foreground">
            {root.kind ? t(`kind_${root.kind}`) : t("mixed")}
          </p>
        </div>
        <span className="rounded-full bg-white/5 px-2 py-1 text-xs text-muted-foreground">
          {t(status)}
        </span>
      </div>
      <p className="root-card-path">{root.path}</p>
      <p className="mt-3 text-xs text-muted-foreground">
        {t("lastScan")}:{" "}
        {root.last_scan_at ? date(root.last_scan_at) : t("neverScanned")}
      </p>
      <div className="root-card-actions">
        {root.enabled && (
          <Button
            size="sm"
            variant="outline"
            disabled={disabled || refreshing || active}
            onClick={onRefresh}
          >
            <RefreshCw size={15} aria-hidden="true" />
            {t(refreshing || active ? "rootUpdating" : "autoRefreshLibrary")}
          </Button>
        )}
        {attention && (
          <Button asChild size="sm" variant="ghost">
            <Link
              to="/scanner"
              search={{ root: root.id, task: task.id, ...scope }}
            >
              {t("rootReviewUpdate")}
            </Link>
          </Button>
        )}
        <Button size="sm" variant="ghost" disabled={disabled} onClick={onEdit}>
          <Pencil size={15} aria-hidden="true" />
          {t("edit")}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          disabled={disabled || refreshing || active}
          onClick={onRemove}
        >
          <Trash2 size={15} aria-hidden="true" />
          {t("remove")}
        </Button>
      </div>
    </article>
  );
}
