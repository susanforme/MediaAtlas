import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { chooseSelectOption, selectOptions } from "../../test/select";
import { Dialog, DialogContent, DialogTitle } from "./dialog";
import { Label } from "./label";
import { Select } from "./select";

function Control({ changed = () => {} }: { changed?: (value: string) => void }) {
  const [value, setValue] = useState("movie");
  return (
    <Label>
      Media type
      <Select value={value} onChange={(event) => {
        setValue(event.currentTarget.value);
        changed(event.currentTarget.value);
      }}>
        <option value="movie">电影</option>
        <option value="tv" disabled>电视剧</option>
        <option value="anime">动画</option>
      </Select>
    </Label>
  );
}

it("uses a custom popup and real change events while preserving nested labels", () => {
  const changed = vi.fn();
  const { container } = render(<Control changed={changed} />);
  const control = screen.getByRole("combobox", { name: "Media type" });
  expect(control.tagName).toBe("BUTTON");
  expect(screen.queryByRole("option")).not.toBeInTheDocument();
  const options = selectOptions(control);
  expect(options.map((option) => option.textContent)).toEqual(["电影", "电视剧", "动画"]);
  expect(options[1]).toBeDisabled();
  expect(container.contains(screen.getByRole("listbox"))).toBe(false);
  chooseSelectOption(control, "anime");
  expect(control).toHaveValue("anime");
  expect(control).toHaveTextContent("动画");
  expect(control).toHaveFocus();
  expect(changed).toHaveBeenCalledExactlyOnceWith("anime");
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
});

it("skips disabled options by keyboard and cancels without changing the selection", () => {
  render(<Control />);
  const control = screen.getByRole("combobox", { name: "Media type" });
  control.focus();
  fireEvent.keyDown(control, { key: "ArrowDown" });
  fireEvent.keyDown(control, { key: "Enter" });
  expect(control).toHaveValue("anime");
  fireEvent.keyDown(control, { key: "Home" });
  fireEvent.keyDown(control, { key: "Escape" });
  expect(control).toHaveValue("anime");
  expect(control).toHaveAttribute("aria-expanded", "false");
  fireEvent.keyDown(control, { key: "动" });
  expect(document.getElementById(control.getAttribute("aria-activedescendant") ?? "")).toHaveTextContent("动画");
  fireEvent.keyDown(control, { key: "Tab" });
  expect(control).toHaveAttribute("aria-expanded", "false");
});

it("keeps required validation and native form values without exposing a native popup", () => {
  const { container } = render(
    <form>
      <Label htmlFor="choice">Source</Label>
      <Select id="choice" name="source" required defaultValue="" aria-invalid="true">
        <option value="">Choose</option>
        <optgroup label="Sources">
          <option value="tmdb">TMDB</option>
        </optgroup>
      </Select>
    </form>,
  );
  const control = screen.getByRole("combobox", { name: "Source" });
  const form = container.querySelector("form");
  if (!form) throw new Error("Expected the form");
  expect(control).toHaveAttribute("aria-required", "true");
  expect(control).toHaveAttribute("aria-invalid", "true");
  act(() => { expect(form.checkValidity()).toBe(false); });
  expect(control).toHaveFocus();
  chooseSelectOption(control, "tmdb");
  expect(form.checkValidity()).toBe(true);
  expect(new FormData(form).get("source")).toBe("tmdb");
  expect(screen.getAllByRole("combobox")).toHaveLength(1);
});

it("keeps a popup inside its enclosing dialog and does not dismiss the dialog on selection", async () => {
  render(
    <Dialog defaultOpen>
      <DialogContent aria-describedby={undefined}>
        <DialogTitle>Settings</DialogTitle>
        <Control />
      </DialogContent>
    </Dialog>,
  );
  const dialog = await screen.findByRole("dialog", { name: "Settings" });
  const control = within(dialog).getByRole("combobox", { name: "Media type" });
  selectOptions(control);
  expect(dialog.contains(screen.getByRole("listbox"))).toBe(true);
  chooseSelectOption(control, "anime");
  expect(screen.getByRole("dialog", { name: "Settings" })).toBeInTheDocument();
  expect(control).toHaveValue("anime");
  expect(control).toHaveFocus();
});

it("closes when disabled and does not reopen a stale menu when enabled again", () => {
  const view = render(<Select aria-label="Kind"><option value="movie">Movie</option></Select>);
  fireEvent.click(screen.getByRole("combobox", { name: "Kind" }));
  expect(screen.getByRole("listbox")).toBeInTheDocument();
  view.rerender(<Select disabled aria-label="Kind"><option value="movie">Movie</option></Select>);
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  view.rerender(<Select aria-label="Kind"><option value="movie">Movie</option></Select>);
  expect(screen.getByRole("combobox", { name: "Kind" })).toHaveAttribute("aria-expanded", "false");
});

it("opens above a trigger near the viewport edge without extending past the right edge", () => {
  render(<Control />);
  const control = screen.getByRole("combobox", { name: "Media type" });
  vi.spyOn(control, "getBoundingClientRect").mockReturnValue(
    new DOMRect(window.innerWidth - 32, window.innerHeight - 36, 32, 36),
  );
  fireEvent.click(control);
  const popup = screen.getByRole("listbox");
  expect(Number.parseFloat(popup.style.bottom)).toBeGreaterThan(0);
  expect(Number.parseFloat(popup.style.left) + Number.parseFloat(popup.style.width)).toBeLessThanOrEqual(window.innerWidth - 4);
});

it("starts at the final enabled option for ArrowUp when the current value is unavailable", () => {
  const changed = vi.fn();
  render(
    <Select aria-label="Kind" value="unavailable" onChange={(event) => changed(event.currentTarget.value)}>
      <option value="movie">Movie</option>
      <option value="anime">Anime</option>
    </Select>,
  );
  const control = screen.getByRole("combobox", { name: "Kind" });
  fireEvent.keyDown(control, { key: "ArrowUp" });
  fireEvent.keyDown(control, { key: "Enter" });
  expect(changed).toHaveBeenCalledExactlyOnceWith("anime");
});
