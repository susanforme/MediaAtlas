import { Star } from "lucide-react";
import { type ComponentProps, type ReactNode, useState } from "react";
import { useTranslation } from "react-i18next";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../../components/ui/tabs";
import { cn } from "../../lib/utils";
import { MediaArtwork } from "./media-artwork";
import { useFormat } from "./shared";
import "./video-detail-translations";

export function MediaDetailView({
  title,
  originalTitle,
  year,
  kind,
  runtime,
  genres = [],
  rating,
  poster,
  backdrop,
  overview,
  facts = [],
  navigation,
  actions,
  status,
  children,
}: {
  title: string;
  originalTitle?: string | null;
  year?: number | null;
  kind?: string;
  runtime?: number | null;
  genres?: string[];
  rating?: { value: number; provider: string | null } | null;
  poster?: { uri: string | null; updatedAt?: string };
  backdrop?: string | null;
  overview?: string;
  facts?: { key: string; label: string; value: ReactNode }[];
  navigation?: ReactNode;
  actions?: ReactNode;
  status?: ReactNode;
  children?: ReactNode;
}) {
  const { t } = useTranslation();
  const { number, date } = useFormat();
  const [expanded, setExpanded] = useState(false);
  const canExpand = (overview?.length ?? 0) > 240;
  const background = backdrop || poster?.uri;
  return (
    <article data-slot="media-detail" className="min-w-0 [&_h2:not([data-slot=video-status-heading])]:mb-3 [&_h2:not([data-slot=video-status-heading])]:text-[20px] [&_h2:not([data-slot=video-status-heading])]:font-semibold">
      <header className={cn("relative isolate pt-0 pb-9", background && "min-h-[300px] pb-[100px] [@media(width<=760px)]:min-h-0 [@media(width<=760px)]:pb-9")}>
        {background && (
          <div data-slot="media-backdrop" className={cn("pointer-events-none absolute -top-[104px] -right-10 -bottom-[70px] left-[35%] -z-1 overflow-hidden opacity-40 [mask-image:linear-gradient(to_right,transparent,black_35%),linear-gradient(to_bottom,black_40%,transparent)] [mask-composite:intersect] [&_img]:object-[right_top] [&_[data-slot=poster-placeholder]]:hidden [@media(width<=760px)]:-right-6 [@media(width<=760px)]:left-[20%] [@media(width<=760px)]:opacity-25", backdrop ? "[&_img]:object-cover" : "[&_img]:object-contain")} aria-hidden="true">
            <MediaArtwork uri={background} title="" eager />
          </div>
        )}
        <div className="max-w-[900px]">
          {navigation}
          <h1 className="m-0 max-w-[780px] text-[clamp(28px,3vw,42px)] leading-[1.2] font-[650] tracking-[-0.035em] [overflow-wrap:anywhere]">{title}</h1>
          {originalTitle && originalTitle !== title && (
            <p className="mt-2 text-[18px] text-foreground">{originalTitle}</p>
          )}
          {(year != null || kind || runtime != null || genres.length > 0 || rating) && (
            <div className="mt-4 flex flex-wrap items-center gap-x-3.5 gap-y-2 text-[14px] text-muted-foreground [&>span+span:not([data-slot=badge])]:before:mr-3.5 [&>span+span:not([data-slot=badge])]:before:content-['·']">
              {year != null && <span>{year}</span>}
              {kind && <span>{kind}</span>}
              {runtime != null && <span>{t("videoDetailRuntime", { count: runtime })}</span>}
              {genres.length > 0 && <span>{genres.join(" / ")}</span>}
              {rating && (
                <Badge variant="secondary" className="gap-[7px] text-foreground [&_svg]:text-[color:var(--rating)] [&>span]:text-muted-foreground">
                  <Star size={17} aria-hidden="true" />
                  {number(rating.value)}
                  <span>{rating.provider ?? t("localMetadata")}</span>
                </Badge>
              )}
            </div>
          )}
          {actions && <div className="mt-6 flex flex-wrap items-center gap-3 [&>[data-slot=button]]:min-h-[46px]">{actions}</div>}
        </div>
      </header>
      <div className={cn("relative z-10", background && "-mt-16 [@media(width<=760px)]:mt-0", poster ? "grid grid-cols-[minmax(180px,260px)_minmax(0,1fr)] items-start gap-8 [@media(760px<width<=1350px)]:grid-cols-[minmax(160px,220px)_minmax(0,1fr)] [@media(width<=1350px)]:gap-6 [@media(width<=760px)]:grid-cols-[minmax(0,1fr)]" : "min-w-0")}>
        {poster && (
          <aside className="[@media(width<=760px)]:max-w-[180px]">
            <div className="poster-image aspect-2/3 overflow-hidden rounded-[14px] border border-input bg-muted">
              <MediaArtwork uri={poster.uri} title={title} eager />
            </div>
            {poster.updatedAt && <p className="mt-3.5 text-[12px] text-muted-foreground">{t("updated")} {date(poster.updatedAt)}</p>}
          </aside>
        )}
        <div className="min-w-0">
          {(overview || facts.length > 0 || status) && (
            <div className={status ? "mb-8 grid grid-cols-[minmax(0,1fr)_minmax(240px,280px)] items-start gap-6 [@media(width<=1350px)]:grid-cols-[minmax(0,1fr)]" : "mb-8 min-w-0"}>
              <section className="min-w-0">
                {overview && (
                  <>
                    <h2 className="mb-3 text-[20px] font-semibold">{t("videoDetailOverview")}</h2>
                    <p className={cn("whitespace-pre-line leading-[1.8] text-muted-foreground", canExpand && !expanded && "line-clamp-3")}>{overview}</p>
                    {canExpand && (
                      <Button variant="ghost" size="sm" className="mt-1.5 mr-0 mb-0 ml-auto flex text-ring" aria-expanded={expanded} onClick={() => setExpanded(!expanded)}>
                        {t(expanded ? "videoDetailCollapse" : "videoDetailExpand")}
                      </Button>
                    )}
                  </>
                )}
                {(facts.length > 0 || status) && (
                  <dl className="mt-6 grid gap-4 [&>div]:grid [&>div]:grid-cols-[90px_minmax(0,1fr)] [&>div]:gap-3 [&>div]:text-[14px] [&_dt]:text-muted-foreground [&_dd]:[overflow-wrap:anywhere]">
                    {facts.map((fact) => <div key={fact.key}><dt>{fact.label}</dt><dd className="whitespace-pre-wrap">{fact.value}</dd></div>)}
                  </dl>
                )}
              </section>
              {status}
            </div>
          )}
          {children}
        </div>
      </div>
    </article>
  );
}

export function MediaDetailTabs({ className, ...props }: ComponentProps<typeof Tabs>) {
  return <Tabs {...props} className={cn("[&>[data-slot=tabs-list]]:flex [&>[data-slot=tabs-list]]:w-full [&>[data-slot=tabs-list]]:justify-start [&>[data-slot=tabs-list]]:gap-2 [&>[data-slot=tabs-list]]:overflow-x-auto [&>[data-slot=tabs-list]]:rounded-none [&>[data-slot=tabs-list]]:border-b [&>[data-slot=tabs-list]]:border-border [&>[data-slot=tabs-list]]:bg-transparent [&>[data-slot=tabs-list]]:p-0 [&>[data-slot=tabs-list]]:pb-2.5 [&_[data-slot=tabs-trigger]]:shrink-0 [&_[data-slot=tabs-trigger]]:min-h-10 [&_[data-slot=tabs-trigger]]:rounded-[10px] [&_[data-slot=tabs-trigger]]:bg-muted [&_[data-slot=tabs-trigger]]:px-[18px] [&_[data-slot=tabs-trigger]]:whitespace-nowrap [&_[data-slot=tabs-trigger][data-state=active]]:bg-primary [&_[data-slot=tabs-trigger][data-state=active]]:text-primary-foreground [&>[data-slot=tabs-content]]:mt-[18px]", className)} />;
}

const sectionLabels = {
  files: "videoDetailFiles", seasons: "videoDetailSeasons", cast: "videoDetailCast",
  metadata: "metadata", related: "relations", artwork: "artworks",
} as const;
type DetailSection = keyof typeof sectionLabels;

export function MediaDetailSections({ sections, initialTab }: {
  sections: Partial<Record<DetailSection, ReactNode>>;
  initialTab?: DetailSection;
}) {
  const { t } = useTranslation();
  const available = (Object.keys(sectionLabels) as DetailSection[]).filter((key) => sections[key] != null && sections[key] !== false);
  const [tab, setTab] = useState<string>(initialTab ?? available[0] ?? "metadata");
  if (!available.length) return null;
  return (
    <MediaDetailTabs value={available.includes(tab as DetailSection) ? tab : available[0]} onValueChange={setTab}>
      <TabsList aria-label={t("detailsAction")}>
        {available.map((key) => <TabsTrigger key={key} value={key}>{t(sectionLabels[key])}</TabsTrigger>)}
      </TabsList>
      {available.map((key) => <TabsContent key={key} value={key}>{sections[key]}</TabsContent>)}
    </MediaDetailTabs>
  );
}
