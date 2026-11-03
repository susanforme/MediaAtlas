import type { ComponentProps } from "react";
import { RootsPage } from "../library/pages";
import { ScannerPage } from "../library/scanner";
import { AdultGate } from "./gate";

export function AdultRootsPage(props: ComponentProps<typeof RootsPage>) {
  return (
    <AdultGate>
      <RootsPage {...props} adult />
    </AdultGate>
  );
}

export function AdultScannerPage(props: ComponentProps<typeof ScannerPage>) {
  return (
    <AdultGate>
      <ScannerPage {...props} adult />
    </AdultGate>
  );
}
