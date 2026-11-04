import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import type { ScanTask } from "../../api/library";
import { Button } from "../../components/ui/button";
import { scanRecovery } from "./scan-error-model";
import "./scan-recovery-translations";

export function ScanRecovery({
  task,
  adult = false,
  onRetry,
  retryDisabled = false,
}: {
  task: ScanTask;
  adult?: boolean;
  onRetry?: () => void;
  retryDisabled?: boolean;
}) {
  const { t } = useTranslation();
  const recovery = scanRecovery(task);
  if (!recovery) return null;
  const scope = adult ? { kind: "adult_video" as const } : {};
  return (
    <div className="mt-4 space-y-3">
      <p
        role={recovery.alert ? "alert" : "status"}
        className="text-sm text-muted-foreground"
      >
        {t(recovery.message)}
      </p>
      {recovery.action === "manage" ? (
        <Button asChild size="sm" variant="outline">
          <Link to="/roots" search={scope}>
            {t("scanRecoveryManage")}
          </Link>
        </Button>
      ) : onRetry ? (
        <Button
          size="sm"
          variant="outline"
          disabled={retryDisabled}
          onClick={onRetry}
        >
          {t("scanRecoveryRetry")}
        </Button>
      ) : (
        <Button asChild size="sm" variant="outline">
          <Link
            to="/scanner"
            search={{ root: task.root_id, task: task.id, ...scope }}
          >
            {t("scanRecoveryViewRetry")}
          </Link>
        </Button>
      )}
    </div>
  );
}
