export interface ScannerSearch {
  root?: string;
  task?: string;
  kind?: "adult_video";
}
export function validateScannerSearch(
  value: Record<string, unknown>,
): ScannerSearch {
  return {
    root: typeof value.root === "string" ? value.root : undefined,
    task: typeof value.task === "string" ? value.task : undefined,
    ...(value.kind === "adult_video" ? { kind: "adult_video" as const } : {}),
  };
}
