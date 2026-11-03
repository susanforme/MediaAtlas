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
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { Empty, ErrorNotice, HealthBadges, Loading } from "../library/shared";
import { MAX_SCRAPE_QUEUE } from "../scraper/search";
import { isSelectableCandidate } from "./candidate-model";
import {
  TOOLS_HEALTH,
  TOOLS_KINDS,
  TOOLS_SORTS,
  type ToolsSearch,
} from "./search";
import { useCandidateBrowser } from "./use-candidate-browser";

export function OrdinaryCandidates({
  search = {},
  onSearch,
  renderDetail,
}: {
  search?: ToolsSearch;
  onSearch?: (search: ToolsSearch, replace?: boolean) => void;
  renderDetail: (mediaId: string, close: () => void) => ReactNode;
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
      <div className="panel space-y-4">
        <div className="filter-grid">
          <Label className="filter-field">
            <span>{t("toolsSearch")}</span>
            <Input
              type="search"
              maxLength={256}
              value={browser.search.q ?? ""}
              onChange={(event) =>
                browser.filter({ q: event.target.value || undefined })
              }
            />
          </Label>
          <Label className="filter-field">
            <span>{t("root")}</span>
            <Select
              value={browser.search.root ?? ""}
              onChange={(event) =>
                browser.filter({ root: event.target.value || undefined })
              }
            >
              <option value="">{t("allRoots")}</option>
              {browser.roots.data
                ?.filter((root) => root.kind !== "adult_video")
                .map((root) => (
                  <option key={root.id} value={root.id}>
                    {root.name}
                  </option>
                ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("type")}</span>
            <Select
              value={browser.search.kind ?? ""}
              onChange={(event) =>
                browser.filter({
                  kind: (event.target.value ||
                    undefined) as ToolsSearch["kind"],
                })
              }
            >
              <option value="">{t("allKinds")}</option>
              {TOOLS_KINDS.map((kind) => (
                <option key={kind} value={kind}>
                  {t(`kind_${kind}`)}
                </option>
              ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("health")}</span>
            <Select
              value={browser.search.health ?? ""}
              onChange={(event) =>
                browser.filter({
                  health: (event.target.value ||
                    undefined) as ToolsSearch["health"],
                })
              }
            >
              <option value="">{t("allHealth")}</option>
              {TOOLS_HEALTH.map((health) => (
                <option key={health} value={health}>
                  {t(`health_${health}`)}
                </option>
              ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("sort")}</span>
            <Select
              value={browser.search.sort ?? "updated_desc"}
              onChange={(event) =>
                browser.filter({
                  sort: event.target.value as ToolsSearch["sort"],
                })
              }
            >
              {TOOLS_SORTS.map((sort) => (
                <option key={sort} value={sort}>
                  {t(`sort_${sort}`)}
                </option>
              ))}
            </Select>
          </Label>
        </div>
        <Button variant="ghost" size="sm" onClick={browser.clearFilters}>
          {t("clearFilters")}
        </Button>
        {browser.roots.isError && (
          <ErrorNotice
            error={browser.roots.error}
            retry={() => void browser.roots.refetch()}
          />
        )}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
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
            <Button variant="ghost" size="sm" onClick={browser.clearSelection}>
              {t("toolsClearSelection")}
            </Button>
          )}
        </div>
        {selectedIds.length > 0 ? (
          <Button asChild>
            <Link
              to="/scraper"
              search={{ media: selectedIds[0], queue: selectedIds }}
            >
              {t("toolsScrapeSelected")}
            </Link>
          </Button>
        ) : (
          <Button disabled>{t("toolsScrapeSelected")}</Button>
        )}
      </div>
      <p className="text-sm text-muted-foreground">
        {t("toolsBatchHelp", { limit: MAX_SCRAPE_QUEUE })}
      </p>
      {browser.limitReached && (
        <p role="alert" className="notice">
          {t("toolsSelectionLimit", { limit: MAX_SCRAPE_QUEUE })}
        </p>
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
                <span className="max-w-48 truncate">{card.media.title}</span>
                <X size={14} aria-hidden="true" />
              </Button>
            </li>
          ))}
        </ul>
      )}
      {browser.media.isPending ? (
        <Loading />
      ) : browser.media.isError ? (
        <ErrorNotice
          error={browser.media.error}
          retry={() => void browser.media.refetch()}
        />
      ) : browser.media.data.items.length === 0 ? (
        <Empty title={t("noResults")} />
      ) : (
        <div className="table-wrap">
          <table className="min-w-[620px]">
            <thead>
              <tr>
                <th>
                  <Checkbox
                    ref={selectPage}
                    checked={allSelected}
                    disabled={eligible.length === 0}
                    aria-label={t("toolsSelectPage")}
                    onChange={(event) =>
                      browser.select(eligible, event.target.checked)
                    }
                  />
                </th>
                <th>{t("titleLabel")}</th>
                <th>{t("year")}</th>
                <th>{t("type")}</th>
                <th>{t("health")}</th>
                <th>{t("toolsView")}</th>
              </tr>
            </thead>
            <tbody>
              {browser.media.data.items.map((card) => (
                <tr
                  key={card.media.id}
                  className={
                    browser.search.media === card.media.id
                      ? "bg-primary/5"
                      : undefined
                  }
                >
                  <td>
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
                  </td>
                  <td className="font-medium">{card.media.title}</td>
                  <td>{card.media.year ?? t("unknownYear")}</td>
                  <td>{t(`kind_${card.media.kind}`)}</td>
                  <td>
                    {card.health.length ? (
                      <HealthBadges health={card.health} />
                    ) : (
                      t("none")
                    )}
                  </td>
                  <td>
                    <Button
                      variant="outline"
                      size="sm"
                      aria-label={t("toolsViewResource", {
                        title: card.media.title,
                      })}
                      onClick={() => browser.view(card.media.id)}
                    >
                      {t("toolsView")}
                    </Button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <div className="flex flex-wrap items-center justify-between gap-3">
        <Button
          variant="outline"
          disabled={browser.page <= 1 || browser.media.isPending}
          onClick={browser.previous}
        >
          {t("toolsBack")}
        </Button>
        <span className="text-sm text-muted-foreground">
          {t("page", { page: browser.page })}
        </span>
        <Button
          variant="outline"
          disabled={browser.media.isPending || !browser.media.data?.next_cursor}
          onClick={browser.next}
        >
          {t("toolsMore")}
        </Button>
      </div>
      {browser.search.media &&
        renderDetail(browser.search.media, browser.closeDetail)}
    </section>
  );
}
