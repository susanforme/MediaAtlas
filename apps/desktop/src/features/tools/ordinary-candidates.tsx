import { Link } from "@tanstack/react-router";
import { ChevronDown, X } from "lucide-react";
import {
  type ReactNode,
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
  const [showSelection, setShowSelection] = useState(false);
  const selectPage = useRef<HTMLInputElement>(null);
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
    <section className="min-w-0 space-y-5">
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
        <Table className="min-w-[620px]">
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
              <TableHead>{t("year")}</TableHead>
              <TableHead>{t("type")}</TableHead>
              <TableHead>{t("health")}</TableHead>
              <TableHead>{t("toolsView")}</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {browser.media.data.items.map((card) => (
              <TableRow
                key={card.media.id}
                className={
                  browser.search.media === card.media.id
                    ? "bg-primary/5"
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
                <TableCell className="font-medium">{card.media.title}</TableCell>
                <TableCell>{card.media.year ?? t("unknownYear")}</TableCell>
                <TableCell>{t(`kind_${card.media.kind}`)}</TableCell>
                <TableCell>
                  {card.health.length ? (
                    <HealthBadges health={card.health} />
                  ) : (
                    t("none")
                  )}
                </TableCell>
                <TableCell>
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label={
                      browser.adult
                        ? `${t("scraperThisMedia")}: ${card.media.title}`
                        : t("toolsViewResource", { title: card.media.title })
                    }
                    onClick={() => browser.view(card.media.id)}
                  >
                    {t(browser.adult ? "scraperThisMedia" : "toolsView")}
                  </Button>
                  {!browser.adult && isSelectableCandidate(card) ? (
                    <Button asChild variant="ghost" size="sm">
                      <Link to="/scraper" search={{ media: card.media.id }}>
                        {t("scraperThisMedia")}
                      </Link>
                    </Button>
                  ) : null}
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
      {browser.allowed &&
        browser.search.media &&
        renderDetail(browser.search.media, browser.closeDetail, browser.adult)}
    </section>
  );
}
