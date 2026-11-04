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
    # Keep the shared IPC/artwork helpers importable by older focused QA scripts.
    # The current visual regression follows the library-first application flow.
    import runpy

    current = runpy.run_path(str(ROOT / "scripts/desktop-library-wall-qa.py"))
    current["main"]()


if __name__ == "__main__":
    main()
