// Actual browser screenshots with synthetic Mock IPC. No native/backend evidence.
const { chromium } = require(
  process.env.PLAYWRIGHT_MODULE ||
    "/opt/codex/cua_node/lib/node_modules/playwright",
);
const fs = require("node:fs");
const { execFileSync } = require("node:child_process");
const path = require("node:path");
const assert = require("node:assert/strict");
const out = __dirname;
const sourceCommit = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: path.resolve(out, ".."),
  encoding: "utf8",
}).trim();
const appSourceInputsUnchanged =
  execFileSync(
    "git",
    [
      "status",
      "--porcelain=v1",
      "--untracked-files=all",
      "--",
      "apps",
      "crates",
      "Cargo.toml",
      "Cargo.lock",
      "package.json",
      "pnpm-lock.yaml",
      ".github/tauri.release.conf.json",
    ],
    { cwd: path.resolve(out, ".."), encoding: "utf8" },
  ).trim().length === 0;
(async () => {
  const browser = await chromium.launch({
    executablePath: process.env.CHROMIUM_PATH || "/usr/bin/chromium",
    headless: true,
    args: ["--no-sandbox"],
  });
  const errors = [],
    reports = [];
  try {
    const context = await browser.newContext({
      viewport: { width: 1440, height: 1080 },
      deviceScaleFactor: 1,
      colorScheme: "light",
    });
    await context.addInitScript(() => {
      localStorage.setItem("mediaatlas.language", "zh-CN");
      window.isTauri = true;
      window.__screenshotCommands = [];
      const now = "2026-11-03T00:00:00Z";
      const names = [
        "Synthetic Demo: Mountain Journey",
        "Synthetic Demo: City Lights",
        "Synthetic Demo: Ocean Notes",
        "Synthetic Demo: Winter Stories",
      ];
      const items = names.map((title, index) => ({
        id: `synthetic-film-${index + 1}`,
        kind: "movie",
        title,
        original_title: null,
        year: 2026 - index,
        created_at: now,
        updated_at: now,
      }));
      const root = {
        id: "synthetic-root",
        name: "Synthetic Demo Library",
        path: "/synthetic-demo/no-real-media",
        path_style: "posix",
        kind: "movie",
        enabled: true,
        created_at: now,
        updated_at: now,
        exclude_patterns: [],
        last_scan_at: now,
      };
      const file = (media) => ({
        id: `${media.id}-file`,
        media_item_id: media.id,
        root_id: root.id,
        relative_path: `${media.title.replace("Synthetic Demo: ", "")}.mkv`,
        size_bytes: 104857600,
        modified_at: now,
        part: null,
      });
      const callbacks = new Map();
      let next = 1;
      window.__TAURI_INTERNALS__ = {
        transformCallback: (fn) => {
          const id = next++;
          callbacks.set(id, fn);
          return id;
        },
        unregisterCallback: (id) => callbacks.delete(id),
        invoke: async (command, args = {}) => {
          window.__screenshotCommands.push({ command, input: args.input });
          if (command === "plugin:event|listen") return next++;
          if (command === "plugin:event|unlisten") return null;
          const input = args.input || {};
          let data;
          switch (command) {
            case "adult_settings":
              data = {
                entry_visible: false,
                covers_visible: false,
                remember_recent: false,
                unlocked: false,
              };
              break;
            case "library_list_roots":
              data = [
                root,
                {
                  ...root,
                  id: "synthetic-destination",
                  name: "Synthetic Demo Destination",
                  path: "/synthetic-demo/no-output",
                },
              ];
              break;
            case "library_list_media":
            case "library_tools_media":
              data = {
                items: items.map((media) => ({
                  media,
                  poster_uri: null,
                  health: [],
                })),
                next_cursor: null,
              };
              break;
            case "library_facets":
              data = { genres: [], people: [], studios: [], years: [] };
              break;
            case "library_get_media":
            case "library_tools_detail": {
              const media =
                items.find((item) => item.id === input.media_id) || items[0];
              data = {
                media,
                files: [file(media)],
                artworks: [],
                external_ids: [],
                metadata_values: [
                  {
                    id: "synthetic-title-source",
                    field_key: "title",
                    locale: null,
                    value: media.title,
                    provider_id: "local_nfo",
                    source_url: null,
                    is_user_modified: false,
                    is_locked: true,
                  },
                ],
                relations: [],
                health: [],
                poster_uri: null,
              };
              break;
            }
            case "organization_history":
              data = [];
              break;
            case "music_library":
            case "music_tools_library":
              data = [
                {
                  release_id: "synthetic-album-1",
                  title: "Synthetic Demo: Quiet Mornings",
                  artist: "Synthetic Demo Ensemble",
                  country: "GB",
                  date: "2026-10-01",
                  medium_count: 1,
                  track_count: 10,
                },
                {
                  release_id: "synthetic-album-2",
                  title: "Synthetic Demo: Night Transit",
                  artist: "Synthetic Demo Quartet",
                  country: "JP",
                  date: "2025-11-01",
                  medium_count: 2,
                  track_count: 18,
                },
              ];
              break;
            case "provider_auth_status":
              data = {
                provider_id: input.provider_id,
                supported_methods: ["bearer", "api_key"],
                method: "bearer",
                storage: "system",
                configured: false,
                status: "not_configured",
              };
              break;
            case "maintenance_status":
              data = {
                backups: [
                  {
                    id: "synthetic-backup",
                    created_at: now,
                    app_version: "0.1.0-beta.1",
                    schema_version: 17,
                    bytes: 262144,
                    contains_private_metadata: false,
                  },
                ],
                restore_pending: false,
                directory: "/synthetic-demo/app-data/maintenance",
              };
              break;
            case "maintenance_backup_preview":
              data = {
                id: "synthetic-backup-preview",
                contains_private_metadata: false,
                includes_secrets: false,
                includes_source_media: false,
                may_include_tag_recovery_bytes: true,
              };
              break;
            default:
              throw {
                schema_version: 1,
                code: "synthetic_unhandled_command",
                message: `Synthetic screenshot fixture has no command: ${command}`,
                correlation_id: "synthetic-only",
              };
          }
          return { schema_version: 1, data };
        },
      };
      window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: () => {},
      };
    });
    const page = await context.newPage();
    page.on("pageerror", (error) => errors.push(error.message));
    await page.route("**/*", (route) => {
      const url = route.request().url();
      return url.startsWith("http://127.0.0.1:1420/") ||
        url.startsWith("ws://127.0.0.1:1420/")
        ? route.continue()
        : route.abort();
    });
    const cases = [
      {
        route: "/library",
        name: "browser-mock-01-library.png",
        ready: "Synthetic Demo: Mountain Journey",
      },
      {
        route: "/tools?media=synthetic-film-1",
        name: "browser-mock-02-tools.png",
        ready: "Synthetic Demo: Mountain Journey",
      },
      {
        route: "/music",
        name: "browser-mock-03-music.png",
        ready: "Synthetic Demo: Quiet Mornings",
      },
      {
        route: "/settings",
        name: "browser-mock-04-settings.png",
        ready: "TMDB",
      },
      {
        route: "/diagnostics",
        name: "browser-mock-05-maintenance.png",
        ready: "2026-11-03T00:00:00Z · v0.1.0-beta.1 · 262144 B",
      },
    ];
    for (const item of cases) {
      await page.goto(`http://127.0.0.1:1420${item.route}`);
      await page.locator("main h1").waitFor();
      if (item.route.startsWith("/tools")) {
        await page
          .locator("main select")
          .first()
          .selectOption("synthetic-film-1");
        await page
          .getByRole("heading", { name: item.ready, exact: true })
          .waitFor();
        await page.getByRole("button", { name: "预览整理" }).waitFor();
      } else if (item.route === "/settings") {
        await page.locator("input[type=password]").waitFor();
      } else {
        await page
          .getByText(item.ready, { exact: item.route === "/diagnostics" })
          .first()
          .waitFor();
      }
      if (item.route === "/diagnostics") {
        await page.getByRole("button", { name: "预览完整数据库备份" }).click();
        await page.getByRole("button", { name: "确认备份数据库" }).waitFor();
      }
      await page.evaluate(() => document.fonts.ready);
      await page.waitForTimeout(400);
      const mainText = await page.locator("main").textContent();
      assert(
        !/undefined|\[object Object\]|当前操作无法完成|The operation failed/.test(
          mainText,
        ),
        "No failed or malformed content",
      );
      assert.equal(
        await page.locator("main [role=alert]").count(),
        0,
        "No error notices",
      );
      assert.equal(
        await page
          .locator("input[type=password]")
          .evaluateAll(
            (inputs) => inputs.filter((input) => input.value.length).length,
          ),
        0,
        "No credentials visible or entered",
      );
      await page.screenshot({
        path: path.join(out, item.name),
        fullPage: true,
        animations: "disabled",
      });
      reports.push({
        route: item.route,
        file: item.name,
        heading: await page.locator("main h1").textContent(),
        mode: "Chromium browser with synthetic Mock IPC",
        errors: 0,
      });
    }
    assert.deepEqual(errors, []);
    const calls = await page.evaluate(() => window.__screenshotCommands);
    assert(
      !calls.some((call) =>
        /_confirm$|_execute$|_save$|_import$/.test(call.command),
      ),
      "No write confirmation or execution",
    );
    fs.writeFileSync(
      path.join(out, "browser-capture-report.json"),
      `${JSON.stringify(
        {
          version: "0.1.0-beta.1",
          candidateAppSourceCommit: sourceCommit,
          appSourceInputsUnchanged,
          captureOutputs:
            "screenshots only; committed separately after the source commit",
          captured_at: new Date().toISOString(),
          mode: "Browser rendering only; not native application, real backend or installation evidence",
          locale: "zh-CN",
          viewport: { width: 1440, height: 1080 },
          synthetic_data: true,
          native_backend: false,
          page_errors: errors,
          screenshots: reports,
        },
        null,
        2,
      )}\n`,
    );
    console.log(
      JSON.stringify({ screenshots: reports, pageErrors: errors }, null, 2),
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
