import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, it } from "vitest";

const config = (name: string) =>
  JSON.parse(
    readFileSync(
      resolve(
        dirname(fileURLToPath(import.meta.url)),
        "../../../src-tauri",
        name,
      ),
      "utf8",
    ),
  );

it("removes only platform caption chrome while preserving configured window geometry and security", () => {
  const base = config("tauri.conf.json");
  const windows = config("tauri.windows.conf.json");
  const macos = config("tauri.macos.conf.json");
  for (const override of [windows, macos]) {
    expect(override.app.windows).toHaveLength(1);
    for (const [key, value] of Object.entries(base.app.windows[0]))
      expect(override.app.windows[0][key]).toEqual(value);
    expect(override.app.security).toBeUndefined();
    expect(override.app.windows[0].backgroundColor).toBe("#101016");
  }
  expect(base.app.windows[0].decorations).toBeUndefined();
  expect(windows.app.windows[0].decorations).toBe(false);
  expect(macos.app.windows[0]).toMatchObject({
    decorations: true,
    titleBarStyle: "Overlay",
    hiddenTitle: true,
    trafficLightPosition: { x: 16, y: 18 },
  });
});

it("grants only local main and NFO preview caption and drag operations", () => {
  const capability = config("capabilities/window-chrome.json");
  expect(capability.windows).toEqual(["main", "nfo-*"]);
  expect(capability.local).toBe(true);
  expect(capability.platforms.sort()).toEqual(["linux", "macOS", "windows"]);
  expect(capability.remote).toBeUndefined();
  expect(capability.permissions.sort()).toEqual(
    [
      "core:window:allow-close",
      "core:window:allow-internal-toggle-maximize",
      "core:window:allow-is-focused",
      "core:window:allow-is-maximized",
      "core:window:allow-minimize",
      "core:window:allow-start-dragging",
      "core:window:allow-toggle-maximize",
    ].sort(),
  );
});
