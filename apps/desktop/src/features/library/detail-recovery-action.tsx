import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { MediaDetail } from "../../api/library";
import { Button } from "../../components/ui/button";
import { detailRecovery } from "./detail-recovery-model";
import "./library-status-translations";

export function DetailRecoveryAction({ detail }: { detail: MediaDetail }) {
  const { t } = useTranslation();
  const recovery = detailRecovery(detail);
  if (recovery.action === "none") return null;
  if (recovery.action === "pending")
    return (
      <span role="status" className="badge">
        {t("health_metadata_pending")}
      </span>
    );
  if (recovery.action === "configure")
    return (
      <Button asChild variant="outline">
        <Link to="/settings" search={recovery.search}>
          {t(
            recovery.search.root
              ? "libraryRefreshConfigureAction"
              : "libraryRefreshConfigureOnly",
          )}
        </Link>
      </Button>
    );
  const adult = detail.media.kind === "adult_video";
  return (
    <Button asChild variant="outline">
      <Link
        to={adult ? "/tools" : "/scraper"}
        search={
          adult
            ? { kind: "adult_video", media: detail.media.id }
            : { media: detail.media.id }
        }
      >
        {t(
          recovery.action === "identify"
            ? "libraryIdentify"
            : "libraryCorrectMatch",
        )}
      </Link>
    </Button>
  );
}
