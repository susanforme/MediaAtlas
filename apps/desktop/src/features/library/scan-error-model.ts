import type { ScanTask } from "../../api/library";

const errors = {
  invalid_root: { message: "scanRecoveryRoot", action: "manage" },
  invalid_exclude_pattern: {
    message: "scanRecoveryExclusions",
    action: "manage",
  },
  local_file_unreadable: {
    message: "scanRecoveryUnreadable",
    action: "manage",
  },
  unsafe_xml: { message: "scanRecoveryUnsafeMetadata", action: "retry" },
  invalid_local_metadata: {
    message: "scanRecoveryInvalidMetadata",
    action: "retry",
  },
  local_metadata_too_large: {
    message: "scanRecoveryLargeMetadata",
    action: "retry",
  },
  files_changed: { message: "scanRecoveryChanged", action: "retry" },
  scan_failed: { message: "scanRecoveryFailed", action: "retry" },
} as const;

/** Task errors are untrusted data: only known codes select public instructions. */
export function scanRecovery(task: Pick<ScanTask, "state" | "error_code">): {
  message: string;
  action: "manage" | "retry";
  alert: boolean;
} | null {
  if (task.state === "cancelled")
    return { message: "scanRecoveryCancelled", action: "retry", alert: false };
  if (task.state === "interrupted")
    return {
      message: "scanRecoveryInterrupted",
      action: "retry",
      alert: false,
    };
  if (!task.error_code && task.state !== "failed") return null;
  const known =
    task.error_code && Object.hasOwn(errors, task.error_code)
      ? errors[task.error_code as keyof typeof errors]
      : errors.scan_failed;
  return { ...known, alert: true };
}
