import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import i18n from "../../i18n";
import { useWindowChrome } from "./use-window-controls";
import { WindowControls } from "./window-controls";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: vi.fn() }));
vi.mock("@tauri-apps/api/window", () => ({ getCurrentWindow: vi.fn() }));

const native = {
  label: "main",
  isMaximized: vi.fn<() => Promise<boolean>>(),
  isFocused: vi.fn<() => Promise<boolean>>(),
  minimize: vi.fn<() => Promise<void>>(),
  toggleMaximize: vi.fn<() => Promise<void>>(),
  close: vi.fn<() => Promise<void>>(),
  onResized: vi.fn(),
  onFocusChanged: vi.fn(),
};
let resize: () => void;
let focus: () => void;
const stopResize = vi.fn();
const stopFocus = vi.fn();
let maximized: boolean;
let focused: boolean;

function Harness() {
  const chrome = useWindowChrome();
  return (
    <div data-testid="shell" data-window-platform={chrome.platform ?? "none"}>
      <WindowControls chrome={chrome} />
    </div>
  );
}

beforeEach(async () => {
  vi.resetAllMocks();
  await i18n.changeLanguage("en-US");
  vi.spyOn(navigator, "platform", "get").mockReturnValue("Win32");
  vi.mocked(isTauri).mockReturnValue(true);
  vi.mocked(getCurrentWindow).mockReturnValue(
    native as unknown as ReturnType<typeof getCurrentWindow>,
  );
  maximized = false;
  focused = true;
  native.isMaximized.mockImplementation(async () => maximized);
  native.isFocused.mockImplementation(async () => focused);
  native.minimize.mockResolvedValue(undefined);
  native.close.mockResolvedValue(undefined);
  native.toggleMaximize.mockImplementation(async () => {
    maximized = !maximized;
  });
  native.onResized.mockImplementation(async (handler) => {
    resize = handler;
    return stopResize;
  });
  native.onFocusChanged.mockImplementation(async (handler) => {
    focus = handler;
    return stopFocus;
  });
});

async function showReady() {
  const view = render(<Harness />);
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Maximize window" }),
    ).toBeEnabled(),
  );
  return view;
}

it("uses real window actions and tracks maximize/restore and focus from native state", async () => {
  await showReady();
  expect(native.minimize).not.toHaveBeenCalled();
  expect(native.toggleMaximize).not.toHaveBeenCalled();
  expect(native.close).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Maximize window" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Restore window" }),
    ).toBeEnabled(),
  );
  expect(native.toggleMaximize).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Restore window" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Maximize window" }),
    ).toBeEnabled(),
  );
  expect(native.toggleMaximize).toHaveBeenCalledTimes(2);
  focused = false;
  await act(async () => focus());
  expect(
    screen.getByRole("group", { name: "Window controls" }),
  ).toHaveAttribute("data-focused", "false");
  maximized = true;
  await act(async () => resize());
  expect(
    screen.getByRole("button", { name: "Restore window" }),
  ).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Minimize window" }));
  await waitFor(() => expect(native.minimize).toHaveBeenCalledTimes(1));
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Close window" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Close window" }));
  await waitFor(() => expect(native.close).toHaveBeenCalledTimes(1));
});

it("prevents title-bar dragging and deduplicates rapid actions before a render", async () => {
  const dragged = vi.fn();
  let finish!: () => void;
  native.toggleMaximize.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  render(
    <fieldset onMouseDown={dragged}>
      <Harness />
    </fieldset>,
  );
  const button = screen.getByRole("button", { name: "Maximize window" });
  await waitFor(() => expect(button).toBeEnabled());
  expect(
    screen.getByRole("group", { name: "Window controls" }),
  ).toHaveAttribute("data-tauri-drag-region", "false");
  fireEvent.mouseDown(button, { button: 0 });
  expect(dragged).not.toHaveBeenCalled();
  await act(async () => {
    button.click();
    button.click();
  });
  expect(native.toggleMaximize).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("button", { name: "Close window" })).toBeDisabled();
  maximized = true;
  await act(async () => finish());
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Restore window" }),
    ).toBeEnabled(),
  );
});

it("keeps failed initial controls disabled and retries state/listeners without replaying actions", async () => {
  native.isMaximized.mockRejectedValueOnce(new Error("private native failure"));
  render(<Harness />);
  await screen.findByRole("alert");
  expect(
    screen.getByRole("button", { name: "Minimize window" }),
  ).toBeDisabled();
  expect(screen.queryByText("private native failure")).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Retry window controls" }),
  );
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Maximize window" }),
    ).toBeEnabled(),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(stopResize).toHaveBeenCalledTimes(1);
  expect(stopFocus).toHaveBeenCalledTimes(1);
  expect(native.minimize).not.toHaveBeenCalled();
  expect(native.close).not.toHaveBeenCalled();
  expect(native.toggleMaximize).not.toHaveBeenCalled();
});

it("shows action errors safely and only retries an action after another explicit click", async () => {
  native.minimize.mockRejectedValueOnce(new Error("private path"));
  await showReady();
  fireEvent.click(screen.getByRole("button", { name: "Minimize window" }));
  await screen.findByRole("alert");
  expect(screen.queryByText("private path")).not.toBeInTheDocument();
  expect(native.minimize).toHaveBeenCalledTimes(1);
  fireEvent.click(
    screen.getByRole("button", { name: "Retry window controls" }),
  );
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
  expect(native.minimize).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: "Minimize window" }));
  await waitFor(() => expect(native.minimize).toHaveBeenCalledTimes(2));
});

it("keeps listener failures visible through successful state reads until registration is retried", async () => {
  native.onResized.mockRejectedValueOnce(new Error("private listener error"));
  await showReady();
  await screen.findByRole("alert");
  await act(async () => focus());
  expect(screen.getByRole("alert")).toBeInTheDocument();
  expect(screen.queryByText("private listener error")).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Retry window controls" }),
  );
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
  expect(native.onResized).toHaveBeenCalledTimes(2);
  expect(stopFocus).toHaveBeenCalledTimes(1);
  expect(native.minimize).not.toHaveBeenCalled();
});

it("ignores late state reads and unregisters listeners that finish registering after unmount", async () => {
  const view = await showReady();
  let older!: (value: boolean) => void;
  let newer!: (value: boolean) => void;
  native.isMaximized.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        older = resolve;
      }),
  );
  await act(async () => resize());
  native.isMaximized.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        newer = resolve;
      }),
  );
  await act(async () => focus());
  await act(async () => newer(true));
  expect(screen.getByRole("button", { name: "Restore window" })).toBeEnabled();
  await act(async () => older(false));
  expect(screen.getByRole("button", { name: "Restore window" })).toBeEnabled();
  view.unmount();
  expect(stopResize).toHaveBeenCalledTimes(1);
  expect(stopFocus).toHaveBeenCalledTimes(1);

  let registered!: (off: () => void) => void;
  const stopLate = vi.fn();
  native.onResized.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        registered = resolve;
      }),
  );
  const pending = render(<Harness />);
  pending.unmount();
  await act(async () => registered(stopLate));
  expect(stopLate).toHaveBeenCalledTimes(1);
});

it.each(["browser", "linux", "macos", "missing native metadata"])(
  "does not invent Windows caption buttons for %s",
  async (kind) => {
    if (kind === "browser") vi.mocked(isTauri).mockReturnValue(false);
    if (kind === "linux")
      vi.spyOn(navigator, "platform", "get").mockReturnValue("Linux x86_64");
    if (kind === "macos")
      vi.spyOn(navigator, "platform", "get").mockReturnValue("MacIntel");
    if (kind === "missing native metadata")
      vi.mocked(getCurrentWindow).mockImplementation(() => {
        throw new Error("No window metadata");
      });
    render(<Harness />);
    expect(
      screen.queryByRole("group", { name: "Window controls" }),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("shell")).toHaveAttribute(
      "data-window-platform",
      kind === "macos" ? "macos" : "none",
    );
    expect(native.isMaximized).not.toHaveBeenCalled();
    expect(native.minimize).not.toHaveBeenCalled();
    expect(native.close).not.toHaveBeenCalled();
  },
);
