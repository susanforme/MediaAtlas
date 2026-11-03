import "../scraper/translations";
import { Link } from "@tanstack/react-router";
import { lazy, Suspense, useState } from "react";
import { useTranslation } from "react-i18next";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { Select } from "../../components/ui/select";
import "../adult/translations";
import { Button } from "../../components/ui/button";
import { usePrivacy } from "../library/context";
import { ErrorNotice, Loading, PageHeading } from "../library/shared";

const ProviderAuthSettings = lazy(() =>
  import("./provider-auth").then((module) => ({
    default: module.ProviderAuthSettings,
  })),
);

export function SettingsPage({
  returnMediaId,
  returnQueue,
  returnToScraper = false,
}: {
  returnMediaId?: string;
  returnQueue?: string[];
  returnToScraper?: boolean;
} = {}) {
  const { t, i18n } = useTranslation();
  return (
    <>
      <PageHeading title={t("settings")}>
        {returnToScraper && (
          <Button asChild variant="outline">
            <Link
              to="/scraper"
              search={{ media: returnMediaId, queue: returnQueue }}
            >
              {t("scraperReturn")}
            </Link>
          </Button>
        )}
      </PageHeading>
      <div className="grid items-start gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)]">
        <Suspense fallback={<Loading />}>
          <ProviderAuthSettings />
        </Suspense>
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
      <h2 className="section-title">{t("kind")}</h2>
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
