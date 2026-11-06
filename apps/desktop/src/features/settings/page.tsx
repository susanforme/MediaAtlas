import "../scraper/translations";
import { lazyRouteComponent, Link } from "@tanstack/react-router";
import { Suspense, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "../../components/ui/tabs";
import { setTheme, useTheme } from "../../lib/theme";
import "../adult/translations";
import { usePrivacy } from "../library/context";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";
import {
  type ProviderSetupSearch,
  SETTINGS_TABS,
  setupReturnLocation,
  type SettingsTab,
  type SetupComplete,
} from "./setup-search";
import "./setup-translations";

const loadAuth = () => import("./provider-auth");
const loadProviders = () => import("../providers/pages");
const loadMaintenance = () => import("../maintenance/page");
const ProviderAuthSettings = lazyRouteComponent(loadAuth, "ProviderAuthSettings");
const ProvidersPage = lazyRouteComponent(loadProviders, "ProvidersPage");
const NetworkInspectorPage = lazyRouteComponent(loadProviders, "NetworkInspectorPage");
const MaintenancePage = lazyRouteComponent(loadMaintenance, "MaintenancePage");

export async function preloadSettingsTab(setup: ProviderSetupSearch) {
  const tab = setup.tab ?? (setup.from || setup.provider ? "auth" : "general");
  if (tab === "auth") { await loadAuth(); await ProviderAuthSettings.preload?.(); }
  if (tab === "providers") { await loadProviders(); await ProvidersPage.preload?.(); }
  if (tab === "network") { await loadProviders(); await NetworkInspectorPage.preload?.(); }
  if (tab === "diagnostics") { await loadMaintenance(); await MaintenancePage.preload?.(); }
}

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
  const theme = useTheme();
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
  function changeTab(next: SettingsTab) {
    if (onTabChange) onTabChange(next);
    else setLocalTab(next);
  }
  return (
    <>
      <PageHeading title={t("settings")}>
        {setup.from && (
          <Button asChild variant="outline">
            <Link {...setupReturnLocation(setup)}>
              {t(
                setup.from === "scraper"
                  ? "scraperReturn"
                  : setup.return?.to === "roots"
                    ? "setupReturnRoots"
                    : setup.return?.to === "media"
                      ? "setupReturnMedia"
                      : "setupReturnLibrary",
              )}
            </Link>
          </Button>
        )}
      </PageHeading>
      <Tabs
        value={tab}
        onValueChange={(value) => changeTab(value as SettingsTab)}
        className="min-w-0"
      >
        <div className="mb-6 overflow-x-auto pb-1">
          <TabsList aria-label={t("settings")} className="w-max">
            {SETTINGS_TABS.map((value) => (
              <TabsTrigger
                key={value}
                value={value}
                className="shrink-0 whitespace-nowrap"
              >
                {t(`settingsTab_${value}`)}
              </TabsTrigger>
            ))}
          </TabsList>
        </div>
        <TabsContent
          value={tab}
          className={
            tab === "general" || tab === "auth" ? "max-w-[720px]" : undefined
          }
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
              <div>
                <div className="space-y-6">
                  <Card asChild>
                    <section>
                      <h2 className="mb-4 font-semibold">{t("appearance")}</h2>
                      <Label className="mb-5 flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
                        <span>{t("theme")}</span>
                        <Select
                          value={theme}
                          onChange={(event) =>
                            setTheme(event.target.value === "light" ? "light" : "dark")
                          }
                        >
                          <option value="dark">{t("theme_dark")}</option>
                          <option value="light">{t("theme_light")}</option>
                        </Select>
                      </Label>
                      <Label className="flex min-w-0 flex-col gap-2 text-[13px] leading-[var(--text-sm--line-height)]">
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
                  </Card>
                  <ContentPreferences />
                </div>
              </div>
            )}
          </Suspense>
        </TabsContent>
      </Tabs>
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
    <Card asChild>
      <section>
        <h2 className="mb-4 font-semibold">{t("settingsMediaTypes")}</h2>
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
    </Card>
  );
}
