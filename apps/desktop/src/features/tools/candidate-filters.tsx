import { Search, X } from "lucide-react";
import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { RootView } from "../../api/library";
import { Button } from "../../components/ui/button";
import { FilterBar, FilterChip } from "../../components/ui/filter-bar";
import { Input } from "../../components/ui/input";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import {
  TOOLS_HEALTH,
  TOOLS_KINDS,
  TOOLS_SORTS,
  type ToolsRouteFilters,
  type ToolsRouteSearch,
} from "./search";
import "../library/library-status-translations";

export function CandidateFilters({
  search,
  roots,
  adult,
  adultEnabled,
  filter,
  clear,
}: {
  search: ToolsRouteSearch;
  roots: RootView[];
  adult: boolean;
  adultEnabled: boolean;
  filter: (patch: ToolsRouteFilters) => void;
  clear: () => void;
}) {
  const { t } = useTranslation();
  const form = useRef<HTMLFormElement>(null);
  useEffect(() => {
    form.current?.reset();
  }, [search.q, search.root, search.kind, search.health, search.sort]);
  const visibleRoots = roots.filter((root) =>
    adult ? root.kind === "adult_video" : root.kind !== "adult_video",
  );
  const active = [
    {
      key: "root",
      label: t("root"),
      value: search.root,
      display: visibleRoots.find((root) => root.id === search.root)?.name ?? t("root"),
    },
    {
      key: "health",
      label: t("health"),
      value: search.health,
      display: search.health ? t(`health_${search.health}`) : "",
    },
  ].filter((item) => item.value);
  const filtered = Boolean(
    search.q || active.length || (search.sort && search.sort !== "updated_desc"),
  );
  return (
    <FilterBar
      label={t("filters")}
      optionsLabel={t("advancedFilters")}
      activeCount={active.length}
      actions={
        filtered && (
          <Button variant="ghost" size="sm" onClick={clear}>
            <X size={14} aria-hidden="true" />
            {t("clearFilters")}
          </Button>
        )
      }
      summary={
        active.length > 0 && active.map((item) => (
          <FilterChip
            key={item.key}
            label={t("removeFilter", { label: item.label, value: item.display })}
            onRemove={() => filter({ [item.key]: undefined })}
          >
            {item.label} · {item.display}
          </FilterChip>
        ))
      }
      filters={
        <>
          <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
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
          <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
            <span>{t("health")}</span>
            <Select
              value={search.health ?? ""}
              onChange={(event) =>
                filter({
                  health: (event.target.value || undefined) as ToolsRouteSearch["health"],
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
        </>
      }
    >
      <form
        ref={form}
        role="search"
        aria-label={t("toolsSearch")}
        className="relative min-w-0 flex-[1_1_220px] max-w-[360px] [@media(max-width:640px)]:basis-full [@media(max-width:640px)]:max-w-none"
        onSubmit={(event) => {
          event.preventDefault();
          const value = new FormData(event.currentTarget).get("q");
          const query = typeof value === "string" ? value.trim() : "";
          if (query !== (search.q ?? "")) filter({ q: query || undefined });
        }}
      >
        <Input
          className="pr-10"
          name="q"
          type="search"
          maxLength={256}
          aria-label={t("toolsSearch")}
          placeholder={t("toolsSearch")}
          defaultValue={search.q ?? ""}
        />
        <Button type="submit" variant="ghost" size="icon" className="absolute top-0.5 right-0.5 size-8 p-0" aria-label={t("search")}>
          <Search size={16} aria-hidden="true" />
        </Button>
      </form>
      <Label>
        <span className="sr-only">{t("type")}</span>
        <Select
          value={search.kind ?? ""}
          onChange={(event) =>
            filter({
              kind: (event.target.value || undefined) as ToolsRouteSearch["kind"],
            })
          }
        >
          <option value="">{t("allKinds")}</option>
          {[
            ...TOOLS_KINDS,
            ...(adultEnabled || adult ? ["adult_video"] : []),
          ].map((kind) => (
            <option
              key={kind}
              value={kind}
              disabled={kind === "adult_video" && !adultEnabled}
            >
              {t(`kind_${kind}`)}
            </option>
          ))}
        </Select>
      </Label>
      <Label>
        <span className="sr-only">{t("sort")}</span>
        <Select
          value={search.sort ?? "updated_desc"}
          onChange={(event) =>
            filter({ sort: event.target.value as ToolsRouteSearch["sort"] })
          }
        >
          {TOOLS_SORTS.map((sort) => (
            <option key={sort} value={sort}>
              {t(`sort_${sort}`)}
            </option>
          ))}
        </Select>
      </Label>
    </FilterBar>
  );
}
