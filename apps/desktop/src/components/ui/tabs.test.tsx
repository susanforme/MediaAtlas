import { fireEvent, render, screen } from "@testing-library/react";
import { type KeyboardEventHandler, useState } from "react";
import { expect, it } from "vitest";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./tabs";

function PreviewTabs({
  onKeyDown,
}: {
  onKeyDown?: KeyboardEventHandler<HTMLButtonElement>;
}) {
  const [value, setValue] = useState("merged");
  return (
    <Tabs value={value} onValueChange={setValue}>
      <TabsList aria-label="Preview mode">
        <TabsTrigger value="merged" onKeyDown={onKeyDown}>
          Merged
        </TabsTrigger>
        <TabsTrigger value="unavailable" disabled>
          Unavailable
        </TabsTrigger>
        <TabsTrigger value="compare">Compare</TabsTrigger>
        <TabsTrigger value="source">Source</TabsTrigger>
      </TabsList>
      <TabsContent value="merged">Merged metadata</TabsContent>
      <TabsContent value="unavailable">Unavailable metadata</TabsContent>
      <TabsContent value="compare">Metadata differences</TabsContent>
      <TabsContent value="source">Source metadata</TabsContent>
    </Tabs>
  );
}

it("exposes one selected tab and its labelled panel with roving tab order", () => {
  render(<PreviewTabs />);
  const merged = screen.getByRole("tab", { name: "Merged" });
  const compare = screen.getByRole("tab", { name: "Compare" });
  const panel = screen.getByRole("tabpanel", { name: "Merged" });
  expect(merged).toHaveAttribute("aria-selected", "true");
  expect(merged).toHaveAttribute("tabindex", "0");
  expect(compare).toHaveAttribute("aria-selected", "false");
  expect(compare).toHaveAttribute("tabindex", "-1");
  expect(merged).toHaveAttribute("aria-controls", panel.id);
  expect(panel).toHaveAttribute("aria-labelledby", merged.id);
  expect(screen.queryByRole("tabpanel", { name: "Compare" })).toBeNull();
  fireEvent.click(compare);
  expect(compare).toHaveAttribute("aria-selected", "true");
  expect(compare).toHaveAttribute("tabindex", "0");
  expect(merged).toHaveAttribute("tabindex", "-1");
  expect(screen.getByRole("tabpanel", { name: "Compare" })).toBeVisible();
  expect(panel).not.toBeVisible();
});

it("Arrow keys move focus and selection, skipping disabled tabs and wrapping", () => {
  render(<PreviewTabs />);
  const merged = screen.getByRole("tab", { name: "Merged" });
  const compare = screen.getByRole("tab", { name: "Compare" });
  const source = screen.getByRole("tab", { name: "Source" });
  const unavailable = screen.getByRole("tab", { name: "Unavailable" });
  merged.focus();
  fireEvent.keyDown(merged, { key: "ArrowRight" });
  expect(compare).toHaveFocus();
  expect(compare).toHaveAttribute("aria-selected", "true");
  expect(unavailable).toBeDisabled();
  expect(unavailable).toHaveAttribute("aria-selected", "false");
  fireEvent.keyDown(compare, { key: "ArrowLeft" });
  expect(merged).toHaveFocus();
  fireEvent.keyDown(merged, { key: "ArrowLeft" });
  expect(source).toHaveFocus();
  expect(source).toHaveAttribute("aria-selected", "true");
  fireEvent.keyDown(source, { key: "ArrowRight" });
  expect(merged).toHaveFocus();
  expect(screen.getByRole("tabpanel", { name: "Merged" })).toBeVisible();
});

it("Home and End select the first and last enabled tabs without leaving the tablist", () => {
  render(<PreviewTabs />);
  const merged = screen.getByRole("tab", { name: "Merged" });
  const compare = screen.getByRole("tab", { name: "Compare" });
  const source = screen.getByRole("tab", { name: "Source" });
  compare.focus();
  fireEvent.keyDown(compare, { key: "End" });
  expect(source).toHaveFocus();
  expect(screen.getByRole("tabpanel", { name: "Source" })).toBeVisible();
  fireEvent.keyDown(source, { key: "Home" });
  expect(merged).toHaveFocus();
  expect(screen.getByRole("tabpanel", { name: "Merged" })).toBeVisible();
});

it("honors prevented keyboard events and leaves unrelated keys to the browser", () => {
  render(<PreviewTabs onKeyDown={(event) => event.preventDefault()} />);
  const merged = screen.getByRole("tab", { name: "Merged" });
  merged.focus();
  fireEvent.keyDown(merged, { key: "ArrowRight" });
  expect(merged).toHaveFocus();
  expect(merged).toHaveAttribute("aria-selected", "true");
  fireEvent.keyDown(merged, { key: "Tab" });
  expect(merged).toHaveAttribute("aria-selected", "true");
});
