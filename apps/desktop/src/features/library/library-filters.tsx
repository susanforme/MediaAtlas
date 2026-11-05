import { LayoutGrid, List, X } from "lucide-react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type {
  FacetOption,
  Facets,
  MediaKind,
  RootView,
} from "../../api/library";
import { Button } from "../../components/ui/button";
import { FilterBar, FilterChip } from "../../components/ui/filter-bar";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import { type LibrarySearch, supportedLibraryHealth } from "./browse-model";
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
  filter: (patch: LibrarySearch) => void;
  reset: () => void;
  onView: (view: "wall" | "list") => void;
  children: ReactNode;
}) {
  const { t } = useTranslation();
  const health = supportedLibraryHealth(search.health, adult);
  const visibleRoots = roots.filter((root) =>
    adult
      ? root.kind === "adult_video" || root.kind === null
      : root.kind !== "adult_video",
  );
  const activeFilters = [
    {
      key: "root",
      label: t("root"),
      value: search.root,
      display: visibleRoots.find((root) => root.id === search.root)?.name ?? t("root"),
    },
    {
      key: "health",
      label: t("health"),
      value: health,
      display: health ? t(`health_${health}`) : "",
    },
    ...(!adult
      ? [
          { key: "genre", label: t("genre"), value: search.genre, display: search.genre },
          { key: "person", label: t("person"), value: search.person, display: search.person },
          { key: "studio", label: t("studio"), value: search.studio, display: search.studio },
          { key: "year", label: t("year"), value: search.year, display: search.year },
        ]
      : []),
  ].filter((item) => item.value);
  const filtered = Boolean(
    search.health ||
      search.q ||
      search.genre ||
      search.person ||
      search.studio ||
      search.year ||
      (search.sort && search.sort !== "updated_desc"),
  );
  return (
    <FilterBar
      label={t("filters")}
      optionsLabel={t("advancedFilters")}
      activeCount={activeFilters.length}
      summary={
        activeFilters.length > 0 && activeFilters.map((item) => (
          <FilterChip
            key={item.key}
            label={t("removeFilter", { label: item.label, value: item.display })}
            onRemove={() => filter({ [item.key]: undefined })}
          >
            {item.label} · {item.display}
          </FilterChip>
        ))
      }
      actions={
        <>
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
        </>
      }
      filters={
        <>
          <Label className="filter-field">
            <span>{t("root")}</span>
            <Select
              value={search.root ?? ""}
              onChange={(event) =>
                filter({ root: event.target.value || undefined })
              }
            >
              <option value="">{t("allRoots")}</option>
              {visibleRoots.map((root) => (
                <option key={root.id} value={root.id}>
                  {root.name}
                </option>
              ))}
            </Select>
          </Label>
          <Label className="filter-field">
            <span>{t("health")}</span>
            <Select
              value={health ?? ""}
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
            <p className="text-sm text-muted-foreground" role="status">
              {t("operationFailed")}
            </p>
          )}
        </>
      }
    >
      {children}
      <Label>
        <span className="sr-only">{t("type")}</span>
        <Select
          value={search.kind ?? ""}
          onChange={(event) =>
            filter({
              kind: (event.target.value || undefined) as MediaKind | undefined,
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
              {t(kind === "music_release" ? "libraryKindMusic" : `kind_${kind}`)}
            </option>
          ))}
        </Select>
      </Label>
      <Label>
        <span className="sr-only">{t("sort")}</span>
        <Select
          value={search.sort ?? "updated_desc"}
          onChange={(event) => filter({ sort: event.target.value })}
        >
          {["title_asc", "year_desc", "added_desc", "updated_desc"].map((sort) => (
            <option key={sort} value={sort}>
              {t(`sort_${sort}`)}
            </option>
          ))}
        </Select>
      </Label>
    </FilterBar>
  );
}
