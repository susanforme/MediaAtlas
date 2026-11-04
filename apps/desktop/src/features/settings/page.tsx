import "../scraper/translations";
import { Link } from "@tanstack/react-router";
import { lazy, Suspense, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import "../adult/translations";
import { Button } from "../../components/ui/button";
import { usePrivacy } from "../library/context";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import {
  type ProviderSetupSearch,
  SETTINGS_TABS,
  type SettingsTab,
  type SetupComplete,
} from "./setup-search";
import "./setup-translations";

const ProviderAuthSettings = lazy(() =>
  import("./provider-auth").then((module) => ({
    default: module.ProviderAuthSettings,
  })),
);
const ProvidersPage = lazy(() =>
  import("../providers/pages").then((module) => ({
    default: module.ProvidersPage,
  })),
);
const NetworkInspectorPage = lazy(() =>
  import("../providers/pages").then((module) => ({
    default: module.NetworkInspectorPage,
  })),
);
const MaintenancePage = lazy(() =>
  import("../maintenance/page").then((module) => ({
    default: module.MaintenancePage,
  })),
);

export function SettingsPage({
  returnMediaId,
  returnQueue,
  returnToScraper = false,
  setup: inputSetup,
  onSetupComplete,
  onTabChange,
}: {
  returnMediaId?: string;
  returnQueue?: string[];
  returnToScraper?: boolean;
  setup?: ProviderSetupSearch;
  onSetupComplete?: SetupComplete;
  onTabChange?: (tab: SettingsTab) => void;
} = {}) {
  const { t, i18n } = useTranslation();
  const setup =
    inputSetup ??
    ((returnToScraper
      ? { from: "scraper", media: returnMediaId, queue: returnQueue }
      : {}) as ProviderSetupSearch);
  const [localTab, setLocalTab] = useState<SettingsTab>();
  const tab =
    setup.tab ??
    localTab ??
    (setup.from || setup.provider ? "auth" : "general");
  const tabs = useRef<Partial<Record<SettingsTab, HTMLButtonElement | null>>>(
    {},
  );
  function changeTab(next: SettingsTab) {
    setLocalTab(next);
    onTabChange?.(next);
  }
  return (
    <>
      <PageHeading title={t("settings")}>
        {setup.from === "scraper" && (
          <Button asChild variant="outline">
            <Link
              to="/scraper"
              search={{
                media: setup.media,
                queue: setup.queue,
                session: setup.session,
              }}
            >
              {t("scraperReturn")}
            </Link>
          </Button>
        )}
        {setup.from === "library" && (
          <Button asChild variant="outline">
            <Link to="/roots" search={{ kind: setup.kind }}>
              {t("setupReturnLibrary")}
            </Link>
          </Button>
        )}
      </PageHeading>
      <div
        className="mb-6 flex flex-wrap gap-2"
        role="tablist"
        aria-label={t("settings")}
      >
        {SETTINGS_TABS.map((value, index) => (
          <Button
            key={value}
            ref={(element) => {
              tabs.current[value] = element;
            }}
            id={`settings-tab-${value}`}
            role="tab"
            aria-selected={tab === value}
            aria-controls={`settings-panel-${value}`}
            tabIndex={tab === value ? 0 : -1}
            variant={tab === value ? "default" : "outline"}
            onClick={() => changeTab(value)}
            onKeyDown={(event) => {
              const nextIndex =
                event.key === "ArrowRight"
                  ? (index + 1) % SETTINGS_TABS.length
                  : event.key === "ArrowLeft"
                    ? (index + SETTINGS_TABS.length - 1) % SETTINGS_TABS.length
                    : event.key === "Home"
                      ? 0
                      : event.key === "End"
                        ? SETTINGS_TABS.length - 1
                        : -1;
              if (nextIndex < 0) return;
              event.preventDefault();
              const next = SETTINGS_TABS[nextIndex];
              changeTab(next);
              tabs.current[next]?.focus();
            }}
          >
            {t(`settingsTab_${value}`)}
          </Button>
        ))}
      </div>
      <div
        id={`settings-panel-${tab}`}
        role="tabpanel"
        aria-labelledby={`settings-tab-${tab}`}
      >
        <Suspense fallback={<Loading />}>
          {tab === "auth" ? (
            <ProviderAuthSettings
              setup={setup}
              onSetupComplete={onSetupComplete}
            />
          ) : tab === "providers" ? (
            <ProvidersPage embedded setup={setup} />
          ) : tab === "network" ? (
            <NetworkInspectorPage embedded />
          ) : tab === "diagnostics" ? (
            <MaintenancePage embedded />
          ) : (
            <div className="grid items-start gap-6 xl:grid-cols-2">
              <div className="space-y-6">
                <section className="panel">
                  <Label className="filter-field">
                    <span>{t("language")}</span>
                    <Select
                      value={i18n.resolvedLanguage ?? "zh-CN"}
                      onChange={(event) =>
                        void i18n.changeLanguage(event.target.value)
                      }
                    >
                      <option value="zh-CN">简体中文</option>
                      <option value="en-US">English (US)</option>
                    </Select>
                  </Label>
                </section>
                <ContentPreferences />
              </div>
            </div>
          )}
        </Suspense>
      </div>
    </>
  );
}

function ContentPreferences() {
  const { t } = useTranslation();
  const { settings, saveSettings, loading, error } = usePrivacy();
  const [enabled, setEnabled] = useState(settings.entry_visible);
  const [edited, setEdited] = useState(false);
  const value = edited ? enabled : settings.entry_visible;
  return (
    <section className="panel">
      <h2 className="section-title">{t("settingsMediaTypes")}</h2>
      <Label className="mb-4 flex items-center gap-3">
        <Checkbox
          checked={value}
          disabled={loading}
          onChange={(event) => {
            setEdited(true);
            setEnabled(event.target.checked);
          }}
        />
        {t("adultEntry")}
      </Label>
      <div className="flex gap-3">
        <Button
          disabled={loading}
          onClick={() => {
            void saveSettings({
              entry_visible: value,
              covers_visible: value,
              remember_recent: false,
            });
            setEdited(false);
          }}
        >
          {t("save")}
        </Button>
      </div>
      {error !== undefined && <ErrorNotice error={error} />}
    </section>
  );
}
