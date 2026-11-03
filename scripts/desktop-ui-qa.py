"""Browser QA against the running Vite app; never imported into production.

Requires Python Playwright and Chromium. Start pnpm dev:web, then run this file.
The default screenshot has no desktop transport. Other screenshots use an
explicit synthetic IPC fixture to exercise states without a native host or user data.
"""

import base64
import io
import json
import os
from pathlib import Path

from PIL import Image, ImageDraw
from playwright.sync_api import sync_playwright, expect


ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get("MEDIAATLAS_QA_OUTPUT_DIR", ROOT / "screenshots"))
OUT.mkdir(parents=True, exist_ok=True)
URL = os.environ.get("MEDIAATLAS_QA_URL", "http://localhost:1420")


def poster(index):
    """Clearly labeled test artwork, never a pretend movie poster."""
    colors = ["#4b394b", "#394553", "#565044", "#374c4b", "#4a4256", "#53404a", "#40495a"]
    image = Image.new("RGB", (360, 540), colors[index % len(colors)])
    draw = ImageDraw.Draw(image)
    draw.rectangle((28, 28, 332, 512), outline="#b0adbd", width=2)
    draw.text((48, 72), "MEDIAATLAS", fill="#eeeaf5", font_size=25)
    draw.text((48, 398), f"QA FIXTURE {index + 1:02}", fill="#eeeaf5", font_size=30)
    draw.text((48, 450), "TEST ARTWORK / 2:3", fill="#d0cbd8", font_size=19)
    buffer = io.BytesIO()
    image.save(buffer, "PNG")
    return "data:image/png;base64," + base64.b64encode(buffer.getvalue()).decode()


CARDS = [
    {
        "media": {
            "id": f"qa-{i}", "kind": "movie", "title": f"QA 媒体 {i + 1:02}",
            "original_title": None, "year": 2020 + i,
            "created_at": "2026-11-03T00:00:00Z", "updated_at": "2026-11-03T00:00:00Z",
        },
        "poster_uri": poster(i), "health": [],
    }
    for i in range(7)
]

ADAPTER = r"""(cards) => {
  window.isTauri = true;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {unregisterListener: () => {}};
  window.__qa = { mode: 'content', calls: [], privacy: {
    entry_visible: false, covers_visible: false, remember_recent: false, unlocked: false
  }};
  const wrap = (data) => ({schema_version: 1, data});
  window.__TAURI_INTERNALS__ = {
    transformCallback: () => 1,
    unregisterCallback: () => {},
    invoke: async (cmd, args = {}) => {
      const input = args.input || {};
      window.__qa.calls.push({cmd, input});
      if (cmd.startsWith('plugin:event|')) return 1;
      if (cmd === 'adult_settings') return wrap(window.__qa.privacy);
      if (cmd === 'adult_save_settings') {
        window.__qa.privacy = {...input, unlocked: false};
        return wrap(window.__qa.privacy);
      }
      if (cmd === 'library_summary') return wrap({total_media: 7, total_roots: 1,
        pending_tasks: 0, failed_tasks: 0, missing_poster: 0, missing_nfo: 0, incomplete_metadata: 0});
      if (cmd === 'library_list_media' || cmd === 'library_tools_media') {
        if (window.__qa.mode === 'loading') return new Promise(() => {});
        if (window.__qa.mode === 'error') throw {schema_version: 1,
          code: 'database_error', message: 'Synthetic QA error', correlation_id: 'qa-error'};
        let items = window.__qa.mode === 'empty' ? [] : cards;
        if (window.__qa.mode === 'missing') items = cards.map((card, i) =>
          ({...card, poster_uri: i % 2 ? null : 'data:image/png;base64,broken'}));
        if (input.query) items = items.filter(c => c.media.title.includes(input.query));
        return wrap({items, next_cursor: null});
      }
      if (cmd === 'library_get_media') {
        const card = cards.find(c => c.media.id === input.media_id) || cards[0];
        return wrap({...card, files: [0,1].map(i => ({id: `file-${i}`,
          media_item_id: card.media.id, root_id: 'qa-root', relative_path: `QA-version-${i+1}.mkv`,
          size_bytes: 1024, modified_at: null, part: null})),
          artworks: [], external_ids: [], metadata_values: [{id:'qa-overview', field_key:'overview',
            locale:'zh-CN', value:'浏览器视觉测试数据，仅用于验证布局、键盘和状态。不会写入媒体库。',
            provider_id:null, source_url:null, is_user_modified:false, is_locked:false}], relations: []});
      }
      if (cmd === 'library_list_roots') return wrap([]);
      if (cmd === 'music_library') return wrap([]);
      if (cmd === 'maintenance_status') return wrap({directory:'QA fixture backup directory',backups:[],restore_pending:false});
      if (cmd === 'library_facets') return wrap({genres:[],people:[],studios:[],years:[]});
      if (cmd === 'provider_auth_status' || cmd === 'provider_auth_save') return wrap({
        provider_id: input.provider_id, supported_methods: ['bearer','api_key'],
        method: input.method || 'bearer', storage: input.storage || 'system',
        configured: cmd === 'provider_auth_save',
        status: cmd === 'provider_auth_save' ? 'configured' : 'not_configured'});
      if (cmd === 'library_open') return wrap({requested:true});
      throw {schema_version:1,code:'database_error',message:'Unsupported QA command',correlation_id:'qa'};
    }
  };
}"""


def no_overflow(page):
    assert page.evaluate("document.documentElement.scrollWidth <= innerWidth"), "Horizontal page overflow"


def capture(page, name):
    no_overflow(page)
    page.screenshot(path=str(OUT / name), full_page=False)


def fixture_context(browser, **kwargs):
    context = browser.new_context(viewport={"width": 1648, "height": 928}, **kwargs)
    context.add_init_script("(" + ADAPTER + ")(" + json.dumps(CARDS) + ")")
    return context


def main():
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"),
                                    headless=True, args=["--no-sandbox"])
        plain = browser.new_page(viewport={"width": 1648, "height": 928})
        plain.goto(URL)
        expect(plain.get_by_text("请在桌面应用中打开")).to_be_visible()
        capture(plain, "desktop-required-1648.png")
        plain.close()

        context = fixture_context(browser)
        page = context.new_page()
        failures = []
        page.on("pageerror", lambda e: failures.append(str(e)))
        page.goto(URL)
        expect(page.locator(".home-poster-card")).to_have_count(7)
        expect(page.locator(".home-hero h1")).to_have_text("QA 媒体 01")
        capture(page, "fixture-home-1648.png")
        assert page.locator("video, audio, iframe").count() == 0
        assert page.get_by_role("link", name="成人媒体库", exact=True).count() == 0

        # Search has one effective query: toolbar routes to the library's URL model.
        page.get_by_role("textbox", name="搜索标题", exact=True).fill("QA 媒体 03")
        page.get_by_role("textbox", name="搜索标题", exact=True).press("Enter")
        expect(page).to_have_url(__import__('re').compile(r"/library\?q="))
        expect(page.locator(".poster-card")).to_have_count(1)
        capture(page, "fixture-library-search-1648.png")
        assert page.locator("input[type=search]").count() == 1

        page.goto(URL)
        expect(page.locator(".home-poster-card")).to_have_count(7)
        launch = page.locator(".home-actions button").first
        expect(launch).to_be_visible()
        launch.focus()
        launch.press("Enter")
        dialog = page.get_by_role("dialog")
        expect(dialog).to_be_visible()
        assert dialog.evaluate("el => el.contains(document.activeElement)")
        for _ in range(5):
            page.keyboard.press("Tab")
            assert dialog.evaluate("el => el.contains(document.activeElement)")
        capture(page, "fixture-version-dialog-1648.png")
        page.keyboard.press("Escape")
        expect(dialog).to_have_count(0)
        expect(launch).to_be_focused()
        assert not any(c["cmd"] == "library_open" for c in page.evaluate("__qa.calls"))

        for mode in ["loading", "empty", "error", "missing"]:
            mode_context = fixture_context(browser)
            mode_context.add_init_script(f"__qa.mode = '{mode}'")
            page = mode_context.new_page()
            page.goto(URL + "/library")
            if mode == "loading":
                expect(page.get_by_role("status")).to_be_visible()
            elif mode == "error":
                expect(page.get_by_role("alert")).to_be_visible()
            elif mode == "empty":
                expect(page.locator(".empty-state")).to_be_visible()
            else:
                # Reset query; the test adapter returns malformed and absent PNGs.
                page.get_by_role("searchbox").fill("")
                expect(page.locator(".poster-placeholder")).to_have_count(7)
            capture(page, f"fixture-{mode}-1648.png")

        page = context.new_page()
        page.goto(URL + "/settings")
        expect(page.get_by_label("凭据", exact=True)).to_be_visible()
        credential = page.get_by_label("凭据", exact=True)
        credential.fill("synthetic-qa-credential")
        assert not any(c["cmd"] == "provider_auth_save" for c in page.evaluate("__qa.calls"))
        page.get_by_role("button", name="保存认证", exact=True).click()
        expect(credential).to_have_value("")
        expect(page.get_by_text("认证已保存，来源启用状态保持不变。")).to_be_visible()
        capture(page, "fixture-settings-1648.png")
        for route, name in [("tools", "tools"), ("music", "music-empty"), ("diagnostics", "maintenance")]:
            page.goto(URL + "/" + route)
            expect(page.locator("main h1")).to_be_visible()
            expect(page.get_by_role("status")).to_have_count(0)
            expect(page.get_by_role("alert")).to_have_count(0)
            capture(page, f"fixture-{name}-1648.png")
        assert failures == [], failures

        narrow = fixture_context(browser, reduced_motion="reduce")
        mobile = narrow.new_page()
        mobile.set_viewport_size({"width": 390, "height": 844})
        mobile.goto(URL)
        expect(mobile.locator(".home-poster-card")).to_have_count(7)
        assert mobile.evaluate("matchMedia('(prefers-reduced-motion: reduce)').matches")
        capture(mobile, "fixture-home-narrow-reduced-motion.png")
        mobile.locator(".home-poster-open").first.hover()
        assert mobile.locator(".home-poster-open .poster-image").first.evaluate("el => getComputedStyle(el).transform") == "none"
        mobile.keyboard.press("Tab")
        assert mobile.locator(":focus-visible").count() == 1
        mobile.goto(URL + "/settings")
        expect(mobile.get_by_label("凭据", exact=True)).to_be_visible()
        capture(mobile, "fixture-settings-narrow.png")
        for route in ["library", "tools", "diagnostics"]:
            mobile.goto(URL + "/" + route)
            expect(mobile.locator("main h1")).to_be_visible()
            expect(mobile.get_by_role("status")).to_have_count(0)
            expect(mobile.get_by_role("alert")).to_have_count(0)
            capture(mobile, f"fixture-{route}-narrow.png")
        browser.close()

    print("Browser QA passed: reference/narrow, search, modal keyboard, reduced motion, loading/empty/error/missing, explicit credential save.")


if __name__ == "__main__":
    main()
