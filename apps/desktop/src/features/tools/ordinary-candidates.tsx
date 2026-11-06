import { Link } from "@tanstack/react-router";
import { ChevronDown, FolderOpen, X } from "lucide-react";
import {
  type ReactNode,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import {
  Table,
  TableHeader,
  TableRow,
  TableHead,
  TableBody,
  TableCell,
} from "../../components/ui/table";
import { Empty, ErrorNotice, HealthBadges, Loading } from "../library/shared";
import { MediaArtwork } from "../library/media-artwork";
import { ReviewQueueLauncher } from "../scraper/review-queue-launcher";
import { MAX_SCRAPE_QUEUE } from "../scraper/search";
import { isSelectableCandidate } from "./candidate-model";
import { CandidateFilters } from "./candidate-filters";
import type { ToolsRouteSearch } from "./search";
import { useCandidateBrowser } from "./use-candidate-browser";

export function OrdinaryCandidates({
  search = {},
  onSearch,
  renderDetail,
}: {
  search?: ToolsRouteSearch;
  onSearch?: (search: ToolsRouteSearch, replace?: boolean) => void;
  renderDetail: (
    mediaId: string,
    close: () => void,
    adult: boolean,
  ) => ReactNode;
}) {
  const { t } = useTranslation();
  const browser = useCandidateBrowser(search, onSearch);
  const selectionId = useId();
  const detailId = useId();
  const [showSelection, setShowSelection] = useState(false);
  const selectPage = useRef<HTMLInputElement>(null);
  const workspacePanel = useRef<HTMLElement>(null);
  const detailPanel = useRef<HTMLElement>(null);
  const detailTrigger = useRef<HTMLButtonElement | null>(null);
  useEffect(() => {
    if (browser.allowed && browser.search.media) detailPanel.current?.focus();
  }, [browser.allowed, browser.search.media]);
  const eligible =
    browser.media.data?.items.filter(isSelectableCandidate) ?? [];
  const selectedIds = browser.selected.map((card) => card.media.id);
  const allSelected =
    eligible.length > 0 &&
    eligible.every((card) => selectedIds.includes(card.media.id));
  const someSelected = eligible.some((card) =>
    selectedIds.includes(card.media.id),
  );
  useLayoutEffect(() => {
    if (selectPage.current)
      selectPage.current.indeterminate = someSelected && !allSelected;
  }, [someSelected, allSelected]);
  return (
    <section ref={workspacePanel} className="min-w-0 space-y-5">
      <CandidateFilters
        search={browser.search}
        roots={browser.roots.data ?? []}
        adult={browser.adult}
        adultEnabled={browser.adultEnabled}
        filter={browser.filter}
        clear={browser.clearFilters}
      />
      {browser.roots.isError && (
        <ErrorNotice error={browser.roots.error} retry={() => void browser.roots.refetch()} />
      )}
      {!browser.adult && (
        <>
          <ReviewQueueLauncher mediaIds={selectedIds} context={browser.search}>
            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                aria-expanded={showSelection}
                aria-controls={selectionId}
                disabled={browser.selected.length === 0}
                onClick={() => setShowSelection(!showSelection)}
              >
                {t("toolsSelected", { count: browser.selected.length })}
                <ChevronDown size={16} aria-hidden="true" />
              </Button>
              {browser.selected.length > 0 && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={browser.clearSelection}
                >
                  {t("toolsClearSelection")}
                </Button>
              )}
            </div>
            <span className="text-xs text-muted-foreground">
              {t("toolsBatchHelp", { limit: MAX_SCRAPE_QUEUE })}
            </span>
          </ReviewQueueLauncher>
          {browser.limitReached && (
            <Alert asChild>
              <p role="alert">
                {t("toolsSelectionLimit", { limit: MAX_SCRAPE_QUEUE })}
              </p>
            </Alert>
          )}
          {showSelection && browser.selected.length > 0 && (
            <ul id={selectionId} className="flex flex-wrap gap-2">
              {browser.selected.map((card) => (
                <li key={card.media.id}>
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label={t("toolsRemoveSelection", {
                      title: card.media.title,
                    })}
                    onClick={() => browser.removeSelected(card.media.id)}
                  >
                    <span className="max-w-48 truncate">
                      {card.media.title}
                    </span>
                    <X size={14} aria-hidden="true" />
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
      {browser.allowed && browser.search.media && (
        <section id={detailId} ref={detailPanel} tabIndex={-1} aria-label={t("toolsDetails")} className="scroll-mt-[calc(var(--toolbar-height,76px)+16px)] space-y-4 rounded-xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
          {renderDetail(browser.search.media, () => {
            browser.closeDetail();
            if (detailTrigger.current?.isConnected && !detailTrigger.current.disabled) detailTrigger.current.focus();
            else workspacePanel.current?.querySelector<HTMLInputElement>('input[type="search"]')?.focus();
          }, browser.adult)}
        </section>
      )}
      {!browser.allowed ? (
        browser.privacyError ? (
          <ErrorNotice error={browser.privacyError} />
        ) : browser.privacyLoading || browser.adultEnabled ? (
          <Loading />
        ) : (
          <Empty title={t("privacyHidden")} />
        )
      ) : browser.media.isPending ? (
        <Loading />
      ) : browser.media.isError ? (
        <ErrorNotice
          error={browser.media.error}
          retry={() => void browser.media.refetch()}
        />
      ) : browser.media.data.items.length === 0 ? (
        <Empty title={t("noResults")} />
      ) : (
        <Table className="min-w-[580px]">
          <TableHeader>
            <TableRow>
              {!browser.adult && (
                <TableHead>
                  <Checkbox
                    ref={selectPage}
                    checked={allSelected}
                    disabled={eligible.length === 0}
                    aria-label={t("toolsSelectPage")}
                    onChange={(event) =>
                      browser.select(eligible, event.target.checked)
                    }
                  />
                </TableHead>
              )}
              <TableHead>{t("titleLabel")}</TableHead>
              <TableHead>{t("health")}</TableHead>
              <TableHead>{t("toolsActions")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {browser.media.data.items.map((card) => (
              <TableRow
                key={card.media.id}
                className={
                  browser.search.media === card.media.id || selectedIds.includes(card.media.id)
                    ? "bg-primary/10"
                    : undefined
                }
              >
                {!browser.adult && (
                  <TableCell>
                    <Checkbox
                      checked={selectedIds.includes(card.media.id)}
                      disabled={!isSelectableCandidate(card)}
                      aria-label={t("toolsSelectForScrape", {
                        title: card.media.title,
                      })}
                      onChange={(event) =>
                        browser.select([card], event.target.checked)
                      }
                    />
                  </TableCell>
                )}
                <TableCell>
                  <div className="flex min-w-0 items-center gap-3">
                    <div aria-hidden="true" className="aspect-2/3 w-10 shrink-0 overflow-hidden rounded-md bg-muted [&_[data-slot=poster-placeholder]>span]:hidden [&_[data-slot=poster-placeholder]_svg]:size-4">
                      <MediaArtwork uri={card.poster_uri} title="" />
                    </div>
                    <div className="min-w-0">
                      <p className="font-medium wrap-anywhere">{card.media.title}</p>
                      <p className="mt-1 text-xs text-muted-foreground">{card.media.year ?? t("unknownYear")} · {t(`kind_${card.media.kind}`)}</p>
                    </div>
                  </div>
                </TableCell>
                <TableCell className="max-w-[280px]">
                  {card.health.length ? (
                    <HealthBadges health={card.health} />
                  ) : (
                    t("none")
                  )}
                </TableCell>
                <TableCell>
                  <div className="flex flex-wrap items-center gap-2">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={selectedIds.length > 0}
                    aria-label={
                      browser.adult
                        ? `${t("scraperThisMedia")}: ${card.media.title}`
                        : t("toolsViewResource", { title: card.media.title })
                    }
                    aria-expanded={browser.search.media === card.media.id}
                    aria-controls={browser.search.media === card.media.id ? detailId : undefined}
                    onClick={(event) => { detailTrigger.current = event.currentTarget; browser.view(card.media.id); }}
                  >
                    <FolderOpen size={15} aria-hidden="true" />
                    {t(browser.adult ? "scraperThisMedia" : "toolsView")}
                  </Button>
                  {!browser.adult && isSelectableCandidate(card) ? (
                    <Button asChild variant="outline" size="sm" className="aria-disabled:pointer-events-none aria-disabled:opacity-45">
                      <Link to="/scraper" search={{ media: card.media.id }} disabled={selectedIds.length > 0}>
                        {t("scraperThisMedia")}
                      </Link>
                    </Button>
                  ) : null}
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Button
          variant="outline"
          disabled={
            !browser.allowed || browser.page <= 1 || browser.media.isPending
          }
          onClick={browser.previous}
        >
          {t("toolsBack")}
        </Button>
        <span className="text-sm text-muted-foreground">
          {t("page", { page: browser.page })}
        </span>
        <Button
          variant="outline"
          disabled={
            !browser.allowed ||
            browser.media.isPending ||
            !browser.media.data?.next_cursor
          }
          onClick={browser.next}
        >
          {t("toolsMore")}
        </Button>
      </div>
    </section>
  );
}
