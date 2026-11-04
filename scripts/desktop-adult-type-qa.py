"""Verify unified resource-type routes in Chromium with explicitly synthetic IPC.

No live adult provider, private media, OS opener, or source file is used.
Run against pnpm dev:web or a production preview via MEDIAATLAS_QA_URL.
"""
import json
import os
from pathlib import Path
import re
import runpy

from playwright.sync_api import expect, sync_playwright
from select_support import select_option

ROOT = Path(__file__).resolve().parents[1]
support = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))
OUT = Path(os.environ.get("MEDIAATLAS_QA_OUTPUT_DIR", ROOT / "work/qa/desktop-adult-type-qa"))
URL = support["URL"]

PRIVATE_ADAPTER = r"""
(() => {
  const previous = __TAURI_INTERNALS__.invoke;
  const wrap = data => ({schema_version:1,data:structuredClone(data)});
  __qa.privacy = {entry_visible:true,covers_visible:true,remember_recent:false,unlocked:UNLOCKED};
  const card = {media:{id:'qa-private-type',kind:'adult_video',title:'QA 成人资源类型 · 合成资料',
    original_title:null,year:2026,created_at:'2026-11-03T00:00:00Z',updated_at:'2026-11-03T00:00:00Z'},
    poster_uri:POSTER,health:[]};
  __TAURI_INTERNALS__.invoke = async (cmd,args={}) => {
    const input=args.input || {};
    if (!cmd.startsWith('adult_') && cmd !== 'scan_list_tasks') return previous(cmd,args);
    __qa.calls.push({cmd,input});
    if(cmd==='adult_settings') return wrap(__qa.privacy);
    if(cmd==='adult_unlock') { __qa.privacy.unlocked=true; return wrap(__qa.privacy); }
    if(cmd==='adult_lock') { __qa.privacy.unlocked=false; return wrap(__qa.privacy); }
    if(cmd==='adult_save_settings') { __qa.privacy={...input,unlocked:false}; return wrap(__qa.privacy); }
    if(cmd==='scan_list_tasks') return wrap([]);
    if(!__qa.privacy.entry_visible || !__qa.privacy.unlocked) throw {schema_version:1,code:'not_found',message:'Synthetic gated fixture',correlation_id:'qa-private'};
    if(cmd==='adult_list_media' || cmd==='adult_tools_media') return wrap({items:[card],next_cursor:null});
    if(cmd==='adult_list_roots' || cmd==='adult_tasks' || cmd==='adult_recent' || cmd==='adult_organization_history') return wrap([]);
    if(cmd==='adult_get_media' || cmd==='adult_tools_detail') return wrap({...card,files:[],artworks:[],external_ids:[],metadata_values:[],relations:[]});
    if(cmd==='adult_media_detail') return wrap(null);
    if(cmd==='adult_providers') return wrap({schema_version:1,providers:[],tasks:[],requests:[]});
    throw {schema_version:1,code:'database_error',message:'Unsupported synthetic adult command',correlation_id:'qa-private'};
  };
})();
"""


def context(browser, unlocked=False, narrow=False):
    ctx = browser.new_context(
        viewport={"width": 390, "height": 844} if narrow else {"width": 1648, "height": 928},
        reduced_motion="reduce", timezone_id="Asia/Shanghai")
    adapter = PRIVATE_ADAPTER.replace("UNLOCKED", json.dumps(unlocked)).replace("POSTER", json.dumps(support["CARDS"][0]["poster_uri"]))
    ctx.add_init_script("(" + support["ADAPTER"] + ")(" + json.dumps(support["CARDS"]) + ");" + adapter)
    return ctx


def capture(page, name):
    support["no_overflow"](page)
    page.screenshot(path=str(OUT / name), full_page=page.viewport_size["width"] < 500)


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"), headless=True, args=["--no-sandbox"])
        page = context(browser).new_page()
        page.goto(URL + "/library?kind=adult_video")
        expect(page.get_by_text("QA 成人资源类型 · 合成资料", exact=True)).to_be_visible()
        expect(page.get_by_role("button", name=re.compile("锁定|解锁"))).to_have_count(0)
        expect(page.locator("main img")).to_have_count(1)
        expect(page.locator("nav a[href='/adult']")).to_have_count(0)
        expect(page.get_by_role("heading", name="媒体库", exact=True)).to_be_visible()
        calls = page.evaluate("__qa.calls.map(c=>c.cmd)")
        assert calls.index("adult_unlock") < calls.index("adult_list_media")
        capture(page, "automatic-fixture-adult-type-1648.png")
        page.locator("main a[href*='/media/qa-private-type']").click()
        expect(page).to_have_url(re.compile(r"/media/qa-private-type\?.*adult_video"))
        expect(page.get_by_role("heading", name="QA 成人资源类型 · 合成资料", exact=True)).to_be_visible()
        expect(page.get_by_role("button", name=re.compile("锁定|解锁"))).to_have_count(0)
        capture(page, "automatic-fixture-adult-detail-1648.png")
        page.get_by_role("link", name="媒体工具", exact=True).last.click()
        expect(page).to_have_url(re.compile(r"/tools\?.*adult_video"))
        expect(page.get_by_role("heading", name="媒体工具", exact=True)).to_be_visible()
        expect(page.locator("main").get_by_role("combobox", name="媒体类型")).to_have_attribute("value", "adult_video")
        expect(page.get_by_text("成人工作台", exact=True)).to_have_count(0)
        expect(page.get_by_role("button", name=re.compile("锁定|解锁"))).to_have_count(0)
        expect(page.locator("main").get_by_role("alert")).to_have_count(0)
        capture(page, "automatic-fixture-adult-tools-1648.png")
        page.get_by_role("link", name="媒体目录", exact=True).click()
        expect(page).to_have_url(re.compile(r"/roots$"))
        page.get_by_role("button", name="添加目录", exact=True).click()
        select_option(page, page.get_by_role("combobox", name="目录媒体类型", exact=True), "adult_video")
        expect(page.get_by_role("combobox", name="目录媒体类型", exact=True)).to_have_attribute("value", "adult_video")
        capture(page, "automatic-fixture-adult-roots-1648.png")
        page.get_by_role("link", name="设置", exact=True).click()
        toggle = page.get_by_role("checkbox", name="启用成人资源类型", exact=True)
        expect(page.get_by_role("checkbox")).to_have_count(1)
        toggle.focus()
        page.keyboard.press("Space")
        expect(toggle).not_to_be_checked()
        page.get_by_role("button", name="保存", exact=True).last.click()
        page.get_by_role("link", name="媒体库", exact=True).first.click()
        page.get_by_role("combobox", name="媒体类型", exact=True).click()
        expect(page.get_by_role("option", name="成人视频", exact=True)).to_have_count(0)
        page.keyboard.press("Escape")
        expect(page.get_by_text("QA 成人资源类型 · 合成资料", exact=True)).to_have_count(0)
        assert not page.evaluate("__qa.calls.some(c => (c.cmd==='library_list_media' || c.cmd==='library_facets') && c.input.include_adult)")
        legacy = context(browser).new_page()
        for old, new in [("/adult", "/library"), ("/adult/tools", "/tools"), ("/adult/media/qa-private-type", "/media/qa-private-type")]:
            legacy.goto(URL + old)
            expect(legacy).to_have_url(re.compile(re.escape(new) + r"\?.*adult_video"))
        narrow = context(browser, narrow=True).new_page()
        narrow.goto(URL + "/library?kind=adult_video")
        expect(narrow.get_by_text("QA 成人资源类型 · 合成资料", exact=True)).to_be_visible()
        capture(narrow, "automatic-fixture-adult-type-narrow.png")
        browser.close()
    print("Unified category QA passed: automatic internal authorization, no unlock/lock/workspace UI, shared library/detail/tools/directory forms, enabled artwork, keyboard settings toggle, disable cleanup, legacy redirects, 1648x928 and 390x844 reduced motion.")


if __name__ == "__main__":
    main()
