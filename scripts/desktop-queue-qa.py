"""Real-browser community-workflow QA, with explicitly synthetic IPC fixtures.

Requires a running Vite app and the same Python/Chromium tools as desktop-ui-qa.
No real provider, credential store, native window or user file is accessed.
"""

import json
import os
from pathlib import Path
import re
import runpy
from urllib.parse import urlencode

from playwright.sync_api import expect, sync_playwright
from select_support import select_option

ROOT = Path(__file__).resolve().parents[1]
ui = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))
workflow = runpy.run_path(str(ROOT / "scripts/desktop-usability-qa.py"))
URL, cards = ui["URL"], ui["CARDS"]
OUT = Path(os.environ.get("MEDIAATLAS_QA_OUTPUT_DIR", ROOT / "work/qa/desktop-queue-qa"))
OUT.mkdir(parents=True, exist_ok=True)

BATCH = r"""
(() => {
  const previous = __TAURI_INTERNALS__.invoke;
  const wrap = data => ({schema_version:1,data});
  const stamp = '2026-11-03T00:00:00Z';
  __qa.roots = [{id:'qa-root',name:'QA Movies',path:'/QA/Movies',kind:'movie',enabled:true,
    path_style:'posix',exclude_patterns:[],last_scan_at:null,created_at:stamp,updated_at:stamp}];
  const cards = __queueCards.map((card, index) => ({...card,
    health:index === 2 ? ['missing_poster'] : []}));
  const privateCard = {...cards[0],media:{...cards[0].media,id:'qa-private',kind:'adult_video',title:'QA PRIVATE MUST STAY HIDDEN'}};
  const musicCard = {...cards[0],media:{...cards[0].media,id:'qa-music',kind:'music_release',title:'QA unsupported music'}};
  const records = [cards[0],privateCard,...cards.slice(1),musicCard];
  __TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    if (cmd === 'library_tools_detail') {
      const response = await previous(cmd, args);
      return wrap({...response.data,metadata_values:[]});
    }
    if (cmd !== 'library_tools_media') return previous(cmd, args);
    const input = args.input || {};
    __qa.calls.push({cmd,input});
    let items = records;
    if (input.root_id && input.root_id !== 'qa-root') items = [];
    if (input.kind) items = items.filter(card => card.media.kind === input.kind);
    if (input.health) items = items.filter(card => card.health.includes(input.health));
    if (input.query) items = items.filter(card => card.media.title.includes(input.query));
    if (input.sort === 'title_asc') items = items.toSorted((a,b) => a.media.title.localeCompare(b.media.title));
    if (input.sort === 'year_desc') items = items.toSorted((a,b) => b.media.year-a.media.year);
    const offset = input.cursor ? Number(input.cursor.replace('qa-page-', '')) : 0;
    return wrap({items:items.slice(offset,offset+3),next_cursor:offset+3 < items.length ? `qa-page-${offset+3}` : null});
  };
})();
"""


def context(browser, narrow=False, missing_auth=False):
    result = browser.new_context(viewport={"width": 390, "height": 844} if narrow else {"width": 1648, "height": 928},
                                 reduced_motion="reduce", timezone_id="Asia/Shanghai")
    script = "(" + ui["ADAPTER"] + ")(" + json.dumps(cards) + ");" + workflow["WORKFLOW"]
    script += ";const __queueCards = " + json.dumps(cards) + ";" + BATCH
    if missing_auth:
        script += "__qa.authConfigured = false;"
    result.add_init_script(script)
    return result


def capture(page, name, full=False):
    ui["no_overflow"](page)
    page.screenshot(path=str(OUT / name), full_page=full)


def queue_url(media="qa-0"):
    return URL + "/scraper?" + urlencode({"media": media, "queue": json.dumps(["qa-0", "qa-1", "qa-2"], separators=(",", ":"))})


with sync_playwright() as p:
    browser = p.chromium.launch(executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"),
                                headless=True, args=["--no-sandbox"])
    page = context(browser).new_page()
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.goto(URL + "/tools")
    expect(page.get_by_role("cell", name="QA 媒体 01", exact=True)).to_be_visible()
    expect(page.get_by_text("QA PRIVATE MUST STAY HIDDEN", exact=True)).to_have_count(0)
    page.get_by_role("checkbox", name="选择本页支持刮削的媒体", exact=True).check()
    expect(page.get_by_role("button", name="已选择 2 项", exact=True)).to_be_visible()
    page.get_by_role("button", name="查看资源：QA 媒体 01", exact=True).click()
    expect(page.get_by_role("button", name="关闭资源详情", exact=True)).to_be_visible()
    expect(page.get_by_role("button", name="已选择 2 项", exact=True)).to_be_visible()
    page.get_by_role("button", name="关闭资源详情", exact=True).click()
    page.get_by_role("button", name="下一页候选", exact=True).click()
    expect(page.get_by_role("cell", name="QA 媒体 03", exact=True)).to_be_visible()
    page.get_by_role("checkbox", name="选择刮削：QA 媒体 03", exact=True).check()
    expect(page.get_by_role("button", name="已选择 3 项", exact=True)).to_be_visible()
    capture(page, "community-fixture-cross-page-selection-1648.png")
    page.get_by_role("link", name="刮削所选", exact=True).click()
    expect(page.get_by_text("逐项核对 · 第 1 / 3 项", exact=True)).to_be_visible()
    expect(page.get_by_label("影片名称", exact=True)).to_have_value("QA 媒体 01")
    capture(page, "community-fixture-review-queue-1648.png")
    page.get_by_role("button", name="跳过当前项", exact=True).click()
    expect(page.get_by_text("逐项核对 · 第 2 / 3 项", exact=True)).to_be_visible()
    expect(page.get_by_label("影片名称", exact=True)).to_have_value("QA 媒体 02")
    assert not any(c["cmd"] in {"scraper_search", "scraper_confirm", "scraper_output_execute"} for c in page.evaluate("__qa.calls"))

    page.goto(URL + "/tools?cursor=qa-page-3&page=2")
    expect(page.get_by_role("cell", name="QA 媒体 03", exact=True)).to_be_visible()
    page.get_by_role("checkbox", name="选择刮削：QA 媒体 03", exact=True).check()
    select_option(page, page.get_by_role("combobox", name="健康状态", exact=True), "missing_poster")
    expect(page.get_by_role("cell", name="QA 媒体 03", exact=True)).to_be_visible()
    expect(page.get_by_role("button", name="已选择 0 项", exact=True)).to_be_visible()
    assert "cursor=" not in page.url and "page=" not in page.url
    latest = [c["input"] for c in page.evaluate("__qa.calls") if c["cmd"] == "library_tools_media"][-1]
    assert latest["health"] == "missing_poster" and latest["cursor"] is None and latest["include_adult"] is False
    select_option(page, page.get_by_role("combobox", name="排序", exact=True), "title_asc")
    expect(page).to_have_url(re.compile(r"sort=title_asc"))
    capture(page, "community-fixture-server-filters-1648.png")
    assert errors == [], errors

    missing = context(browser, missing_auth=True).new_page()
    missing.goto(queue_url("qa-1"))
    expect(missing.get_by_role("button", name="检索来源", exact=True)).to_be_disabled()
    missing.get_by_role("link", name="配置凭据", exact=True).click()
    expect(missing).to_have_url(re.compile(r"/settings\?.*queue="))
    missing.get_by_label("凭据", exact=True).fill("synthetic-qa-credential")
    missing.get_by_role("button", name="保存认证", exact=True).click()
    expect(missing.get_by_text("认证已保存，来源启用状态保持不变。")).to_be_visible()
    expect(missing.get_by_label("凭据", exact=True)).to_have_value("")
    missing.get_by_role("link", name="返回刮削", exact=True).click()
    expect(missing.get_by_text("逐项核对 · 第 2 / 3 项", exact=True)).to_be_visible()
    expect(missing.get_by_label("影片名称", exact=True)).to_have_value("QA 媒体 02")
    expect(missing.get_by_role("button", name="检索来源", exact=True)).to_be_enabled()
    capture(missing, "community-fixture-queue-settings-return-1648.png")
    missing.get_by_role("button", name="跳过当前项", exact=True).click()
    expect(missing.get_by_text("逐项核对 · 第 3 / 3 项", exact=True)).to_be_visible()
    expect(missing.get_by_role("link", name="返回媒体清单", exact=True)).to_have_attribute("href", "/tools")
    expect(missing.get_by_role("button", name="跳过当前项", exact=True)).to_be_disabled()

    single = context(browser).new_page()
    single.goto(URL + "/scraper?media=qa-0")
    single.get_by_role("button", name="检索来源", exact=True).click()
    single.get_by_role("button", name="选择此候选", exact=True).click()
    expect(single.get_by_role("checkbox", name=re.compile(r"^标题(?:\s|$)"))).to_be_checked()
    single.get_by_role("button", name="仅补空缺", exact=True).click()
    expect(single.get_by_role("checkbox", name=re.compile(r"^标题(?:\s|$)"))).not_to_be_checked()
    expect(single.get_by_role("checkbox", name=re.compile(r"^简介(?:\s|$)"))).to_be_checked()
    single.get_by_role("button", name="全不选", exact=True).click()
    expect(single.get_by_role("checkbox", name=re.compile(r"^简介(?:\s|$)"))).not_to_be_checked()
    single.get_by_role("button", name="全选未保护字段", exact=True).click()
    expect(single.get_by_role("checkbox", name=re.compile(r"^简介(?:\s|$)"))).to_be_checked()
    capture(single, "community-fixture-field-presets-1648.png")
    assert not any(c["cmd"] in {"scraper_confirm", "scraper_output_execute"} for c in single.evaluate("__qa.calls"))

    mobile = context(browser, narrow=True).new_page()
    mobile.goto(URL + "/tools")
    expect(mobile.get_by_role("checkbox", name="选择本页支持刮削的媒体", exact=True)).to_be_visible()
    mobile.get_by_role("checkbox", name="选择本页支持刮削的媒体", exact=True).check()
    capture(mobile, "community-fixture-tools-narrow.png", full=True)
    mobile.get_by_role("link", name="刮削所选", exact=True).click()
    expect(mobile.get_by_text("逐项核对 · 第 1 / 2 项", exact=True)).to_be_visible()
    capture(mobile, "community-fixture-queue-narrow.png", full=True)
    browser.close()

print("Community workflow browser QA passed: server filters/sort, cross-page selection, private response defense, bounded manual queue, skip without writes, settings return, field presets and narrow layout.")
