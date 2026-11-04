import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { chooseSelectOption } from "../../test/select";
import { Anchor, Button } from "./button";
import { Checkbox } from "./checkbox";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogTitle,
  DialogTrigger,
} from "./dialog";
import { Input } from "./input";
import { Label } from "./label";
import { Select } from "./select";

function ControlledForm({ changed }: { changed: () => void }) {
  const [checked, setChecked] = useState(false);
  return (
    <>
      <Label>
        Title
        <Input defaultValue="" />
      </Label>
      <Label>
        Kind
        <Select onChange={changed}>
          <option value="movie">Movie</option>
          <option value="anime">Anime</option>
        </Select>
      </Label>
      <Label>
        <Checkbox
          checked={checked}
          onChange={(event) => setChecked(event.target.checked)}
        />
        Enabled
      </Label>
    </>
  );
}

it("preserves native label and controlled form contracts through shared controls", () => {
  const changed = vi.fn();
  render(<ControlledForm changed={changed} />);
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Atlas" },
  });
  expect(screen.getByLabelText("Title")).toHaveValue("Atlas");
  chooseSelectOption(screen.getByRole("combobox", { name: "Kind" }), "anime");
  expect(changed).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("checkbox", { name: "Enabled" }));
  expect(screen.getByRole("checkbox", { name: "Enabled" })).toBeChecked();
});

it("renders composed links without nesting interactive controls", () => {
  render(
    <>
      <Button asChild>
        <a href="/library">Library</a>
      </Button>
      <Anchor href="https://example.com" target="_blank" rel="noreferrer">
        Source
      </Anchor>
    </>,
  );
  expect(screen.getByRole("link", { name: "Library" })).toHaveAttribute(
    "href",
    "/library",
  );
  expect(screen.getByRole("link", { name: "Source" })).toHaveAttribute(
    "rel",
    "noreferrer",
  );
  expect(screen.queryByRole("button")).not.toBeInTheDocument();
});

it("restores focus to the dialog trigger after Escape", async () => {
  render(
    <Dialog>
      <DialogTrigger asChild>
        <Button>Choose version</Button>
      </DialogTrigger>
      <DialogContent aria-describedby={undefined}>
        <DialogTitle>Versions</DialogTitle>
        <DialogClose asChild>
          <Button>Cancel</Button>
        </DialogClose>
      </DialogContent>
    </Dialog>,
  );
  const trigger = screen.getByRole("button", { name: "Choose version" });
  trigger.focus();
  fireEvent.click(trigger);
  expect(screen.getByRole("dialog", { name: "Versions" })).toBeInTheDocument();
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus(),
  );
  fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  await waitFor(() => expect(trigger).toHaveFocus());
});
