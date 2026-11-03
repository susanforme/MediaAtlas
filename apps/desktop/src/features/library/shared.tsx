import { useTranslation } from "react-i18next";
import { DesktopError } from "../../api/desktop";
import type { MediaKind } from "../../api/library";
import { Button } from "../../components/ui/button";

export const ROOT_KINDS: MediaKind[] = [
  "movie",
  "tv_series",
  "anime",
  "music_release",
  "adult_video",
];
export const ALL_KINDS: MediaKind[] = [
  "movie",
  "tv_series",
  "tv_season",
  "tv_episode",
  "anime",
  "music_artist",
  "music_release_group",
  "music_release",
  "music_track",
  "music_recording",
  "music_video",
  "concert",
  "adult_video",
];
export const HEALTH = [
  "missing_poster",
  "missing_backdrop",
  "missing_nfo",
  "incomplete_metadata",
  "duplicate",
];
export function ErrorNotice({
  error,
  retry,
}: {
  error: unknown;
  retry?: () => void;
}) {
  const { t } = useTranslation();
  const desktop =
    error instanceof DesktopError && error.code === "desktop_required";
  const key =
    error instanceof DesktopError && error.code === "invalid_response"
      ? "invalidResponse"
      : error instanceof DesktopError && error.envelope
        ? `error_${error.envelope.code}`
        : "operationFailed";
  return (
    <div role="alert" className="notice error-notice">
      <p className="font-medium">
        {desktop
          ? t("desktopRequired")
          : t(key, {
              defaultValue:
                error instanceof DesktopError && error.envelope
                  ? t(`providerFailure_${error.envelope.code}`, {
                      defaultValue: t("operationFailed"),
                    })
                  : t("operationFailed"),
            })}
      </p>
      {retry && !desktop && (
        <Button className="mt-3" variant="outline" size="sm" onClick={retry}>
          {t("retry")}
        </Button>
      )}
    </div>
  );
}
export function Loading() {
  const { t } = useTranslation();
  return (
    <p className="py-12 text-center text-muted-foreground" role="status">
      {t("loading")}
    </p>
  );
}
export function Empty({ title, body }: { title: string; body?: string }) {
  return (
    <div className="empty-state">
      <h2 className="text-lg font-semibold">{title}</h2>
      {body && (
        <p className="mt-2 max-w-xl text-sm text-muted-foreground">{body}</p>
      )}
    </div>
  );
}
export function PageHeading({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="mb-7 flex flex-wrap items-start justify-between gap-4">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {description && (
          <p className="mt-2 max-w-3xl text-sm leading-6 text-muted-foreground">
            {description}
          </p>
        )}
      </div>
      {children}
    </div>
  );
}
export function Pager({
  page,
  hasNext,
  previous,
  next,
}: {
  page: number;
  hasNext: boolean;
  previous: () => void;
  next: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="mt-6 flex items-center justify-between gap-4">
      <Button variant="outline" disabled={page === 1} onClick={previous}>
        {t("previous")}
      </Button>
      <span className="text-sm text-muted-foreground">
        {t("page", { page })}
      </span>
      <Button variant="outline" disabled={!hasNext} onClick={next}>
        {t("next")}
      </Button>
    </div>
  );
}
export function useFormat() {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage === "en-US" ? "en-US" : "zh-CN";
  return {
    number: (v: number) => new Intl.NumberFormat(locale).format(v),
    date: (v: string) =>
      new Intl.DateTimeFormat(locale, {
        dateStyle: "medium",
        timeStyle: "short",
      }).format(new Date(v)),
  };
}
export function HealthBadges({ health }: { health: string[] }) {
  const { t } = useTranslation();
  return (
    <div className="flex flex-wrap gap-1">
      {health.map((key) => (
        <span key={key} className="badge">
          {t(`health_${key}`, { defaultValue: t("incompleteMetadata") })}
        </span>
      ))}
    </div>
  );
}
