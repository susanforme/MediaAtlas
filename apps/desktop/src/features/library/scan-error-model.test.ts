import { expect, it } from "vitest";
import { scanRecovery } from "./scan-error-model";

it.each([
  ["invalid_root", "scanRecoveryRoot", "manage"],
  ["invalid_exclude_pattern", "scanRecoveryExclusions", "manage"],
  ["local_file_unreadable", "scanRecoveryUnreadable", "manage"],
  ["unsafe_xml", "scanRecoveryUnsafeMetadata", "retry"],
  ["invalid_local_metadata", "scanRecoveryInvalidMetadata", "retry"],
  ["local_metadata_too_large", "scanRecoveryLargeMetadata", "retry"],
  ["files_changed", "scanRecoveryChanged", "retry"],
  ["scan_failed", "scanRecoveryFailed", "retry"],
])(
  "maps %s to a public instruction and supported action",
  (error_code, message, action) => {
    expect(scanRecovery({ state: "failed", error_code })).toEqual({
      message,
      action,
      alert: true,
    });
  },
);

it.each([
  "__proto__",
  "constructor",
  "file /private/secret.xml: detailed internal failure",
  null,
])(
  "does not use unknown error data %s as public text or a translation key",
  (error_code) => {
    expect(scanRecovery({ state: "failed", error_code })).toEqual({
      message: "scanRecoveryFailed",
      action: "retry",
      alert: true,
    });
  },
);

it("describes a stopped task without treating it as an error", () => {
  expect(
    scanRecovery({ state: "cancelled", error_code: "internal/path" }),
  ).toEqual({
    message: "scanRecoveryCancelled",
    action: "retry",
    alert: false,
  });
  expect(scanRecovery({ state: "succeeded", error_code: null })).toBeNull();
});
