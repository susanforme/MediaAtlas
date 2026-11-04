import { fireEvent, within } from "@testing-library/react";

/** Drive the visible listbox rather than bypassing it through the native form bridge. */
export function selectOptions(control: HTMLElement): HTMLElement[] {
  if (control.getAttribute("aria-expanded") !== "true") {
    fireEvent.pointerDown(control);
    fireEvent.click(control);
  }
  const list = document.getElementById(control.getAttribute("aria-controls") ?? "");
  if (!list) throw new Error("Select listbox did not open");
  return within(list).getAllByRole("option");
}

export function chooseSelectOption(control: HTMLElement, value: string): void {
  if (control instanceof HTMLButtonElement && control.disabled) {
    fireEvent.click(control);
    return;
  }
  const option = selectOptions(control).find((item) => (item as HTMLButtonElement).value === value);
  if (!option) throw new Error(`Select option not found: ${value}`);
  fireEvent.pointerDown(option);
  fireEvent.click(option);
}
