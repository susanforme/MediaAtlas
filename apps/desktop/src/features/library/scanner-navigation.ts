export interface ScannerSearch {
  root?: string;
  task?: string;
}
export function validateScannerSearch(
  value: Record<string, unknown>,
): ScannerSearch {
  return {
    root: typeof value.root === "string" ? value.root : undefined,
    task: typeof value.task === "string" ? value.task : undefined,
  };
}
