import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { Dialog, DialogContent, DialogTitle } from "../ui/dialog";
import { Input } from "../ui/input";
import { useSearchShortcut } from "./use-search-shortcut";

function SearchShell({ modal = false }: { modal?: boolean }) {
  useSearchShortcut();
  return (
    <>
      <header data-slot="app-toolbar">
        <form role="search"><Input type="search" aria-label="Media search" defaultValue="film" /></form>
      </header>
      <Dialog open={modal}>
        <DialogContent aria-describedby={undefined}>
          <DialogTitle>Edit directory</DialogTitle>
          <Input aria-label="Directory name" />
        </DialogContent>
      </Dialog>
    </>
  );
}

it.each(["ctrlKey", "metaKey"])("focuses and selects the current search with %s+F", (modifier) => {
  render(<SearchShell />);
  const search = screen.getByRole<HTMLInputElement>("searchbox");
  expect(fireEvent.keyDown(window, { key: "f", [modifier]: true })).toBe(false);
  expect(search).toHaveFocus();
  expect([search.selectionStart, search.selectionEnd]).toEqual([0, 4]);
});

it("does not steal focus from a modal or intercept unrelated shortcuts", () => {
  const { rerender } = render(<SearchShell modal />);
  const editor = screen.getByRole("textbox");
  editor.focus();
  expect(fireEvent.keyDown(window, { key: "f", ctrlKey: true })).toBe(true);
  expect(editor).toHaveFocus();
  rerender(<SearchShell />);
  for (const options of [{ shiftKey: true }, { altKey: true }, { isComposing: true }]) {
    expect(fireEvent.keyDown(window, { key: "f", ctrlKey: true, ...options })).toBe(true);
  }
  expect(screen.getByRole("searchbox")).not.toHaveFocus();
});

it("removes the shortcut when the shell unmounts", () => {
  const { unmount } = render(<SearchShell />);
  unmount();
  expect(fireEvent.keyDown(window, { key: "f", metaKey: true })).toBe(true);
});
