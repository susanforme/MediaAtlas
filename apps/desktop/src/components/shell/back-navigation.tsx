import { Link, useCanGoBack, useRouter } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import type { ReactElement } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../ui/button";
import "./navigation-translations";

export function BackNavigation({
  fallbackTo = "/library",
  adult = false,
}: {
  fallbackTo?: "/library" | "/music";
  adult?: boolean;
}): ReactElement {
  const { t } = useTranslation();
  const router = useRouter();
  const canGoBack = useCanGoBack();
  return canGoBack ? (
    <Button variant="secondary" className="mb-6" onClick={() => router.history.back()}>
      <ArrowLeft size={17} aria-hidden="true" />
      {t("desktopBack")}
    </Button>
  ) : (
    <Button asChild variant="secondary" className="mb-6">
      <Link to={fallbackTo} search={adult && fallbackTo === "/library" ? { kind: "adult_video" } : {}} replace>
        <ArrowLeft size={17} aria-hidden="true" />
        {t(fallbackTo === "/music" ? "musicBack" : "backToLibrary")}
      </Link>
    </Button>
  );
}
