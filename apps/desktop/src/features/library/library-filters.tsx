import { LayoutGrid, List, SlidersHorizontal, X } from "lucide-react";
import { type ReactNode, useId } from "react";
import { useTranslation } from "react-i18next";
import type {
  FacetOption,
  Facets,
  MediaKind,
  RootView,
} from "../../api/library";
import { Button } from "../../components/ui/button";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import type { LibrarySearch } from "./browse-model";
import { HEALTH, LIBRARY_CATEGORIES } from "./library-model";
import { useFormat } from "./shared";
import "./browse-translations";
import "./library-status-translations";

function FacetSelect({
  label,
  value,
  options,
  change,
}: {
  label: string;
  value?: string;
  options: FacetOption[];
  change: (value: string) => void;
}) {
  const { number } = useFormat();
  return (
    <Label className="filter-field">
      <span>{label}</span>
      <Select
        value={value ?? ""}
        onChange={(event) => change(event.target.value)}
      >
        <option value="">{label}</option>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.value} ({number(option.count)})
          </option>
        ))}
      </Select>
    </Label>
  );
}

export function LibraryFilters({
  search,
  adult,
  adultAvailable,
  roots,
  facets,
  facetError,
  advanced,
  setAdvanced,
  filter,
  reset,
  onView,
  children,
}: {
  search: LibrarySearch;
  adult: boolean;
  adultAvailable: boolean;
  roots: RootView[];
  facets: Facets | undefined;
  facetError: boolean;
  advanced: boolean;
  setAdvanced: (value: boolean) => void;
  filter: (patch: LibrarySearch) => void;
  reset: () => void;
  onView: (view: "wall" | "list") => void;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const advancedId = useId();
  const advancedCount = [
    search.root,
    search.health,
    search.genre,
    search.person,
    search.studio,
    search.year,
  ].filter(Boolean).length;
  const filtered = Boolean(
    advancedCount ||
      search.q ||
      search.kind ||
      (search.sort && search.sort !== "updated_desc"),
  );
  return (
    <section className="library-filters" aria-label={t("libraryFilters")}>
      <div className="library-control-row">
        {children}
        <Label className="library-compact-select">
          <span className="sr-only">{t("type")}</span>
          <Select
            value={search.kind ?? ""}
            onChange={(event) =>
              filter({
                kind: (event.target.value || undefined) as
                  | MediaKind
                  | undefined,
                root: undefined,
                health: undefined,
                genre: undefined,
                person: undefined,
                studio: undefined,
                year: undefined,
              })
            }
          >
            <option value="">{t("allKinds")}</option>
            {LIBRARY_CATEGORIES.filter(
              (kind) => kind !== "adult_video" || adultAvailable,
            ).map((kind) => (
              <option value={kind} key={kind}>
                {t(
                  kind === "music_release"
                    ? "libraryKindMusic"
                    : `kind_${kind}`,
                )}
              </option>
            ))}
          </Select>
        </Label>
        <Label className="library-compact-select">
          <span className="sr-only">{t("sort")}</span>
          <Select
            value={search.sort ?? "updated_desc"}
            onChange={(event) => filter({ sort: event.target.value })}
          >
            {["title_asc", "year_desc", "added_desc", "updated_desc"].map(
              (sort) => (
                <option key={sort} value={sort}>
                  {t(`sort_${sort}`)}
                </option>
              ),
            )}
          </Select>
        </Label>
        <Button
          variant={advanced || advancedCount ? "secondary" : "ghost"}
          size="sm"
          aria-expanded={advanced}
          aria-controls={advancedId}
          onClick={() => setAdvanced(!advanced)}
        >
          <SlidersHorizontal size={15} aria-hidden="true" />
          {t("libraryFilters")}
          {advancedCount > 0 && (
            <span className="library-filter-count">{advancedCount}</span>
          )}
        </Button>
        {filtered && (
          <Button variant="ghost" size="sm" onClick={reset}>
            <X size={14} aria-hidden="true" />
            {t("clearFilters")}
          </Button>
        )}
        <fieldset className="library-view-options" aria-label={t("wall")}>
          <Button
            size="sm"
            variant={search.view !== "list" ? "secondary" : "ghost"}
            aria-label={t("wall")}
            aria-pressed={search.view !== "list"}
            onClick={() => onView("wall")}
          >
            <LayoutGrid size={16} aria-hidden="true" />
          </Button>
          <Button
            size="sm"
            variant={search.view === "list" ? "secondary" : "ghost"}
            aria-label={t("list")}
            aria-pressed={search.view === "list"}
            onClick={() => onView("list")}
          >
            <List size={16} aria-hidden="true" />
          </Button>
        </fieldset>
      </div>
      {advanced && (
        <section
          id={advancedId}
          className="library-advanced-filters"
          aria-label={t("libraryFilteredBy")}
        >
          <Label className="filter-field">
            <span>{t("root")}</span>
            <Select
              value={search.root ?? ""}
              onChange={(event) =>
                filter({ root: event.target.value || undefined })
              }
            >
              <option value="">{t("allRoots")}</option>
              {roots
                .filter((root) =>
                  adult
                    ? root.kind === "adult_video" || root.kind === null
                    : root.kind !== "adult_video",
                )
                .map((root) => (
                  <option key={root.id} value={root.id}>
                    {root.name}
                  </option>
                ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("health")}</span>
            <Select
              value={search.health ?? ""}
              onChange={(event) =>
                filter({ health: event.target.value || undefined })
              }
            >
              <option value="">{t("allHealth")}</option>
              {HEALTH.filter(
                (health) => !adult || !health.startsWith("metadata_"),
              ).map((health) => (
                <option key={health} value={health}>
                  {t(`health_${health}`)}
                </option>
              ))}
            </Select>
          </Label>
          {!adult && (
            <>
              <FacetSelect
                label={t("allGenres")}
                options={facets?.genres ?? []}
                value={search.genre}
                change={(value) => filter({ genre: value || undefined })}
              />
              <FacetSelect
                label={t("allPeople")}
                options={facets?.people ?? []}
                value={search.person}
                change={(value) => filter({ person: value || undefined })}
              />
              <FacetSelect
                label={t("allStudios")}
                options={facets?.studios ?? []}
                value={search.studio}
                change={(value) => filter({ studio: value || undefined })}
              />
              <FacetSelect
                label={t("allYears")}
                options={facets?.years ?? []}
                value={search.year?.toString()}
                change={(value) =>
                  filter({ year: value ? Number(value) : undefined })
                }
              />
            </>
          )}
          {facetError && !adult && (
            <p className="text-sm text-muted-foreground">
              {t("operationFailed")}
            </p>
          )}
        </section>
      )}
    </section>
  );
}
