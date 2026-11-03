import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { usePrivacy } from "../library/context";
import { Empty, ErrorNotice, Loading } from "../library/shared";

/** Internal transport readiness. Resource types share the same visible UI. */
export function AdultGate({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { settings, includeAdult, loading, error } = usePrivacy();
  if (error !== undefined) return <ErrorNotice error={error} />;
  if (loading) return <Loading />;
  if (!settings.entry_visible) return <Empty title={t("emptyLibrary")} />;
  if (!includeAdult) return <Loading />;
  return children;
}
