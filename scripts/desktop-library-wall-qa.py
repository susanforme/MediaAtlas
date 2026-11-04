"""Real Chromium QA of the library-first flow, using explicit synthetic IPC.

Run with a Vite development or preview server at MEDIAATLAS_QA_URL. Screenshots
come from the actual application DOM. Labeled test artwork is fixture data;
no screenshot is generated, composited, retouched or used as a product asset.
This does not validate the OS picker, keyring, real providers or native launch.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
import sys
from urllib.parse import parse_qs, urlparse

from playwright.sync_api import expect, sync_playwright


ROOT = Path(__file__).resolve().parents[1]
BASE = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))
URL = BASE["URL"]
OUT = Path(os.environ.get("MEDIAATLAS_QA_OUTPUT_DIR", ROOT / "screenshots/library-wall-2026-11-04"))
CAPTURED = []
UNCONFIGURED_TASK = {
    "id":"qa-unconfigured", "root_id":"qa-root", "state":"waiting_for_user",
    "mode":"automatic", "phase":"complete", "visited_files":7, "skipped_files":0,
    "candidate_count":7, "imported_count":7, "matched_count":0, "review_count":7,
    "unconfigured_count":7, "metadata_failed_count":0, "warnings":0, "error_code":None,
    "created_at":"2026-11-04T00:00:00Z", "updated_at":"2026-11-04T00:00:00Z",
}

FLOW = r"""
(() => {
  const previous = __TAURI_INTERNALS__.invoke;
  const wrap = data => ({schema_version:1,data});
  const stamp = '2026-11-04T00:00:00Z';
  const root = {id:'qa-root',name:'QA Movies',path:'/QA/Movies',kind:'movie',enabled:true,
    path_style:'posix',exclude_patterns:[],last_scan_at:stamp,created_at:stamp,updated_at:stamp};
  Object.assign(__qa, {roots:[root],tasks:[],mediaVisible:true,
    pickerResult:'/QA/Movies',authConfigured:true,providerEnabled:true});
  const descriptor = {id:'tmdb',name:'TMDB',description:'Synthetic QA provider',website:null,
    terms_url:null,privacy_url:null,documentation_url:null,engine_types:['api'],
    requires_api_key:true,requires_login:false,rate_limit_policy:'QA fixture',attribution:'QA',
    data_usage_notes:'Test only',license:'QA',media_kinds:['movie','tv_series'],
    languages:['zh-CN','en-US'],capabilities:['search'],fixture:false};
  const config = {enabled:true,revision:1,timeout_ms:10000,concurrency:1,image_concurrency:1,
    interval_ms:250,max_response_bytes:2097152,max_retries:1,credential_ref:null};
  __TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    const input = args.input || {};
    const owned = ['plugin:dialog|open','library_list_roots','library_save_root','library_sync',
      'scan_list_tasks','provider_snapshot','provider_auth_status','provider_auth_save',
      'provider_set_enabled','provider_update_config','library_list_media','library_tools_media',
      'scraper_media_detail','library_tools_detail','scraper_history','organization_history'];
    if (!owned.includes(cmd)) return previous(cmd,args);
    __qa.calls.push({cmd,input:cmd === 'plugin:dialog|open' ? args : input});
    if (cmd === 'plugin:dialog|open') return __qa.pickerResult;
    if (cmd === 'library_list_roots') return wrap(__qa.roots);
    if (cmd === 'library_save_root') {
      const saved = {...root,...input,id:input.id || root.id};
      __qa.roots = [saved];
      return wrap(saved);
    }
    if (cmd === 'library_sync') {
      const task = {id:'qa-sync',root_id:input.root_id,state:'running',mode:'automatic',phase:'metadata',
        visited_files:7,skipped_files:0,candidate_count:7,imported_count:7,matched_count:0,review_count:0,
        unconfigured_count:0,metadata_failed_count:0,warnings:0,error_code:null,created_at:stamp,updated_at:stamp};
      __qa.tasks = [task]; __qa.mediaVisible = true;
      return wrap(task);
    }
    if (cmd === 'scan_list_tasks') return wrap(__qa.tasks);
    if (cmd === 'provider_snapshot') return wrap({schema_version:1,tasks:[],requests:[],providers:[{
      descriptor,config:{...config,enabled:__qa.providerEnabled},health:'ready'}]});
    if (cmd === 'provider_set_enabled') { __qa.providerEnabled = input.enabled; return wrap(null); }
    if (cmd === 'provider_update_config') { __qa.providerEnabled = input.config.enabled; return wrap(null); }
    if (cmd === 'provider_auth_save') __qa.authConfigured = true;
    if (cmd === 'provider_auth_status' || cmd === 'provider_auth_save') return wrap({
      provider_id:input.provider_id,supported_methods:['bearer','api_key'],method:input.method || 'bearer',
      storage:input.storage || 'system',configured:__qa.authConfigured,
      status:__qa.authConfigured ? 'configured' : 'not_configured'});
    if (cmd === 'library_list_media' || cmd === 'library_tools_media') {
      if (!__qa.mediaVisible) return wrap({items:[],next_cursor:null});
      const value = await previous(cmd,args);
      if (input.root_id && !__qa.roots.some(root => root.id === input.root_id)) return wrap({items:[],next_cursor:null});
      return wrap({...value.data,items:value.data.items
        .filter(card => !input.kind || card.media.kind === input.kind)
        .map(card => ({...card,health:!__qa.authConfigured || !__qa.providerEnabled
          ? ['metadata_unconfigured'] : __qa.tasks.some(task => task.state === 'running') ? ['metadata_pending'] : card.health}))});
    }
    if (cmd === 'library_tools_detail') return previous('library_get_media',args);
    if (cmd === 'scraper_media_detail') return wrap(null);
    if (cmd === 'scraper_history' || cmd === 'organization_history') return wrap([]);
  };
})();
"""


def fixture_context(browser, mode="content", *, narrow=False, extra="", cards=None):
    context = browser.new_context(
        viewport={"width": 390, "height": 844} if narrow else {"width": 1648, "height": 928},
        reduced_motion="reduce" if narrow else "no-preference", timezone_id="Asia/Shanghai",
    )
    context.add_init_script(
        "(" + BASE["ADAPTER"] + ")(" + json.dumps(cards if cards is not None else BASE["CARDS"]) + ");" + FLOW
        + "__qa.mode = " + json.dumps(mode) + ";" + extra
    )
    return context


def screenshot(page, name, *, full_page=False):
    BASE["no_overflow"](page)
    page.screenshot(path=str(OUT / name), full_page=full_page)
    CAPTURED.append(OUT / name)


def commands(page):
    return page.evaluate("__qa.calls.map(call => call.cmd)")


def assert_no_media_writes(page):
    forbidden = {"scan_confirm", "scan_start", "scraper_output_execute", "adult_output_execute",
                 "organization_execute", "music_tag_execute"}
    assert not forbidden.intersection(commands(page)), commands(page)


def library_ready(page):
    expect(page.locator(".poster-card")).to_have_count(7)
    expect(page.get_by_role("heading", name="媒体库", exact=True)).to_be_visible()



def settings_tabs_qa():
    """Selective rerun for the embedded settings tabs and compact directory list."""
    errors = []
    before = len(CAPTURED)
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"),
            headless=True, args=["--no-sandbox"],
        )
        page = fixture_context(browser).new_page()
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.goto(URL + "/settings?tab=general")
        expect(page.get_by_role("tab", name="通用", exact=True)).to_have_attribute("aria-selected", "true")
        assert "provider_snapshot" not in commands(page) and "maintenance_status" not in commands(page)
        for label, value in [("数据来源", "providers"), ("网络检查器", "network"), ("连接诊断", "diagnostics")]:
            tab = page.get_by_role("tab", name=label, exact=True)
            tab.click()
            expect(tab).to_have_attribute("aria-selected", "true")
            panel_id = tab.get_attribute("aria-controls")
            assert panel_id, label
            expect(page.get_by_role("tabpanel")).to_have_attribute("id", panel_id)
            expect(page.locator("main h1")).to_have_count(1)
            expect(page.locator("main h1")).to_have_text("设置")
            expect(page.get_by_role("alert")).to_have_count(0)
            assert urlparse(page.url).path == "/settings", page.url
            assert parse_qs(urlparse(page.url).query).get("tab") == [value], page.url
            if value == "providers":
                expect(page.get_by_role("heading", name="TMDB", exact=True)).to_be_visible()
            elif value == "network":
                expect(page.get_by_role("tabpanel").locator(".empty-state")).to_be_visible()
            else:
                expect(page.get_by_role("tabpanel").locator("input[readonly]")).to_have_value("QA fixture backup directory")
            screenshot(page, f"fixture-settings-{value}-1648.png")
        diagnostics = page.get_by_role("tab", name="连接诊断", exact=True)
        diagnostics.press("Home")
        expect(page.get_by_role("tab", name="通用", exact=True)).to_be_focused()
        expect(page.get_by_role("tab", name="通用", exact=True)).to_have_attribute("aria-selected", "true")
        page.keyboard.press("ArrowRight")
        expect(page.get_by_role("tab", name="来源认证", exact=True)).to_be_focused()
        expect(page.get_by_label("凭据", exact=True)).to_be_visible()
        page.keyboard.press("End")
        expect(diagnostics).to_be_focused()
        expect(diagnostics).to_have_attribute("aria-selected", "true")
        page.keyboard.press("ArrowRight")
        expect(page.get_by_role("tab", name="通用", exact=True)).to_be_focused()
        page.goto(URL + "/roots")
        expect(page.locator(".root-card")).to_have_count(1)
        expect(page.locator(".root-card").get_by_role("link", name="QA Movies", exact=True)).to_be_visible()
        screenshot(page, "fixture-directory-cards-1648.png")
        page.set_viewport_size({"width":390,"height":844})
        screenshot(page, "fixture-directory-cards-390.png")
        assert errors == [], errors
        browser.close()
    report_path = OUT / "report.json"
    report = json.loads(report_path.read_text())
    scenario = "Settings source/network/diagnostic tabs render in place with one title; keyboard Home/End/Arrow keys select and focus tabs; directory cards at both widths"
    if scenario not in report["scenarios"]:
        report["scenarios"].append(scenario)
    report["screenshots"].update({path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in CAPTURED[before:]})
    report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({"result":"passed", "supplement":"settings tabs and directory cards", "screenshots":len(report["screenshots"]), "scenarios":len(report["scenarios"])}, ensure_ascii=False))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    CAPTURED.clear()
    errors = []
    scenarios = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"),
            headless=True, args=["--no-sandbox"],
        )
        def new_page(context):
            page = context.new_page()
            page.on("pageerror", lambda error: errors.append(str(error)))
            return page

        plain = new_page(browser.new_context(viewport={"width":1648,"height":928}))
        plain.goto(URL)
        expect(plain).to_have_url(URL + "/library")
        expect(plain.get_by_text("请在桌面应用中打开").first).to_be_visible()
        screenshot(plain, "browser-desktop-required-1648.png")

        page = new_page(fixture_context(browser))
        page.goto(URL)
        expect(page).to_have_url(URL + "/library")
        library_ready(page)
        sidebar_routes = page.locator(".app-sidebar a").evaluate_all("links => links.map(link => link.getAttribute('href'))")
        assert sidebar_routes == ["/library","/library?kind=movie","/library?kind=tv_series",
                                  "/library?kind=anime","/music","/roots","/tools","/tasks","/settings"], sidebar_routes
        assert page.locator("video,audio,iframe,.home-hero").count() == 0
        assert page.locator(".poster-card button").count() == 0
        for box in page.locator(".poster-card .poster-image").evaluate_all("elements => elements.map(element => ({width:element.clientWidth,height:element.clientHeight}))"):
            assert abs(box["width"] / box["height"] - 2 / 3) < 0.01, box
        wall_cards = [{**card, "media":{**card["media"], "id":f"qa-{index}", "title":f"QA 媒体 {index + 1:02}"},
                       "poster_uri":BASE["poster"](index)}
                      for index, card in enumerate(BASE["CARDS"] * 3)]
        wall = new_page(fixture_context(browser, cards=wall_cards))
        wall.goto(URL)
        expect(wall.locator(".poster-card")).to_have_count(21)
        screenshot(wall, "fixture-library-wall-1648.png")
        screenshot(wall, "fixture-library-wall-1648-full.png", full_page=True)
        wall.close()
        scenarios.append("Root redirects to library; nested media categories and workspace navigation; actual poster links")

        search = page.get_by_role("searchbox", name="搜索标题", exact=True)
        expect(search).to_have_count(1)
        search.fill("QA 媒体 03")
        expect(page.locator(".poster-card")).to_have_count(1)
        expect(page).to_have_url(re.compile(r"/library\?q="))
        screenshot(page, "fixture-library-search-1648.png")
        search.fill("")
        library_ready(page)
        first_poster = page.locator(".poster-card a").first
        first_poster.focus()
        first_poster.press("Enter")
        expect(page).to_have_url(URL + "/media/qa-0")
        expect(page.get_by_role("heading", name="QA 媒体 01", exact=True)).to_be_visible()
        assert "library_open" not in commands(page)
        launch = page.get_by_role("button", name="使用系统应用打开", exact=True)
        launch.focus()
        launch.press("Enter")
        dialog = page.get_by_role("dialog")
        expect(dialog).to_be_visible()
        for _ in range(5):
            page.keyboard.press("Tab")
            assert dialog.evaluate("el => el.contains(document.activeElement)")
        screenshot(page, "fixture-version-picker-1648.png")
        page.keyboard.press("Escape")
        expect(dialog).to_have_count(0)
        expect(launch).to_be_focused()
        assert "library_open" not in commands(page)
        launch.press("Enter")
        dialog.get_by_role("button", name="QA-version-1.mkv", exact=True).click()
        expect(dialog).to_have_count(0)
        expect(launch).to_be_focused()
        assert commands(page).count("library_open") == 1
        screenshot(page, "fixture-system-open-requested-1648.png")
        scenarios.append("Poster keyboard opens details only; version picker traps focus; Escape cancels; successful system dispatch closes and restores focus")

        fresh = new_page(fixture_context(browser, extra="__qa.roots=[];__qa.mediaVisible=false;"))
        fresh.goto(URL)
        expect(fresh.locator(".empty-state")).to_be_visible()
        screenshot(fresh, "fixture-first-library-empty-1648.png")
        fresh.get_by_role("link", name="添加目录", exact=True).first.click()
        expect(fresh).to_have_url(re.compile(r"/roots\?add=true"))
        expect(fresh.get_by_label("目录名称", exact=True)).to_have_value("Movies")
        assert commands(fresh).count("plugin:dialog|open") == 1
        assert "library_save_root" not in commands(fresh)
        screenshot(fresh, "fixture-folder-selected-1648.png")
        fresh.get_by_role("button", name="添加到媒体库", exact=True).click()
        expect(fresh).to_have_url(re.compile(r"/library\?root=qa-root"))
        library_ready(fresh)
        assert commands(fresh).count("library_save_root") == 1
        assert commands(fresh).count("library_sync") == 1
        assert_no_media_writes(fresh)
        screenshot(fresh, "fixture-background-library-1648.png")
        scenarios.append("Add directory immediately requests native picker once; one save and one background sync; no manual import or media writes")

        cancelled = new_page(fixture_context(browser, extra="__qa.roots=[];__qa.mediaVisible=false;__qa.pickerResult=null;"))
        cancelled.goto(URL + "/roots?add=true")
        expect(cancelled.get_by_label("目录名称", exact=True)).to_have_value("")
        assert commands(cancelled).count("plugin:dialog|open") == 1
        assert "library_save_root" not in commands(cancelled)
        assert "library_sync" not in commands(cancelled)
        scenarios.append("Picker cancellation saves and scans nothing")

        for mode in ["loading", "empty", "error", "missing"]:
            state = new_page(fixture_context(browser, mode=mode))
            state.goto(URL + "/library")
            if mode == "loading":
                expect(state.get_by_role("status").first).to_be_visible()
            elif mode == "empty":
                expect(state.locator(".empty-state")).to_be_visible()
            elif mode == "error":
                expect(state.get_by_role("alert").first).to_be_visible()
            else:
                expect(state.locator(".poster-placeholder")).to_have_count(7)
            screenshot(state, f"fixture-{mode}-1648.png")
        scenarios.append("Loading, empty, error and absent/broken image states")

        setup = new_page(fixture_context(browser, extra="__qa.authConfigured=false;__qa.providerEnabled=false;__qa.tasks=" + json.dumps([UNCONFIGURED_TASK]) + ";"))
        setup.goto(URL + "/library?root=qa-root&kind=movie")
        library_ready(setup)
        setup_link = setup.locator('main a[href*="/settings"][href*="from=library"]').first
        expect(setup_link).to_be_visible()
        screenshot(setup, "fixture-source-not-configured-1648.png")
        setup_link.click()
        query = parse_qs(urlparse(setup.url).query)
        assert query.get("root") == ["qa-root"], query
        assert query.get("provider", ["tmdb"]) == ["tmdb"] and query.get("kind", ["movie"]) == ["movie"], query
        credential = setup.get_by_label("凭据", exact=True)
        expect(credential).to_be_visible()
        expect(setup.get_by_role("combobox", name="认证来源", exact=True)).to_have_attribute("value", "tmdb")
        assert setup.get_by_text("kind", exact=True).count() == 0
        assert "provider_auth_save" not in commands(setup)
        credential.fill("synthetic-qa-manual-token")
        assert "provider_auth_save" not in commands(setup)
        screenshot(setup, "fixture-source-setup-1648.png")
        setup.get_by_role("button", name=re.compile(r"保存.*继续")).click()
        expect(setup).to_have_url(re.compile(r"/library\?"))
        query = parse_qs(urlparse(setup.url).query)
        assert query.get("root") == ["qa-root"] and query.get("kind") == ["movie"], query
        calls = commands(setup)
        assert calls.count("provider_auth_save") == 1
        assert calls.count("provider_set_enabled") == 1
        assert calls.count("library_sync") == 1
        assert calls.index("provider_auth_save") < calls.index("provider_set_enabled") < calls.index("library_sync"), calls
        library_ready(setup)
        screenshot(setup, "fixture-source-setup-return-1648.png")
        assert_no_media_writes(setup)
        scenarios.append("Source setup preserves the root and resolves its kind/provider; manual save precedes enable and sync; returns to same library context")

        historical = new_page(fixture_context(browser, extra="__qa.authConfigured=false;__qa.providerEnabled=false;__qa.tasks=[];"))
        historical.goto(URL + "/library?root=qa-root&kind=movie")
        library_ready(historical)
        recovery = historical.locator('main a[href*="/settings"][href*="from=library"]')
        expect(recovery).to_have_count(1)
        expect(recovery).to_be_visible()
        query = parse_qs(urlparse(recovery.get_attribute("href")).query)
        assert query.get("root") == ["qa-root"] and query.get("kind") == ["movie"], query
        screenshot(historical, "fixture-historical-source-recovery-1648.png")
        scenarios.append("Historical cards without a retained task still expose source setup for the same root and kind")

        mobile = new_page(fixture_context(browser, narrow=True))
        mobile.goto(URL)
        library_ready(mobile)
        assert mobile.evaluate("matchMedia('(prefers-reduced-motion: reduce)').matches")
        mobile.locator(".poster-card a").first.hover()
        assert mobile.locator(".poster-card .poster-image").first.evaluate("element => getComputedStyle(element).transform") == "none"
        assert mobile.locator(".poster-card .poster-image").first.evaluate("element => getComputedStyle(element).transitionDuration") == "0s"
        mobile.keyboard.press("Tab")
        assert mobile.locator(":focus-visible").count() == 1
        screenshot(mobile, "fixture-library-wall-390-reduced-motion.png")
        mobile.goto(URL + "/roots?add=true")
        expect(mobile.get_by_label("目录名称", exact=True)).to_have_value("Movies")
        screenshot(mobile, "fixture-folder-selected-390.png")
        mobile.goto(URL + "/settings?from=library&root=qa-root&kind=movie&provider=tmdb")
        expect(mobile.get_by_label("凭据", exact=True)).to_be_visible()
        assert mobile.get_by_text("kind", exact=True).count() == 0
        screenshot(mobile, "fixture-source-setup-390.png")
        mobile.goto(URL + "/settings?tab=general")
        expect(mobile.get_by_role("checkbox", name="启用成人资源类型", exact=True)).to_be_visible()
        assert mobile.get_by_text("kind", exact=True).count() == 0
        screenshot(mobile, "fixture-settings-general-390.png")
        english = new_page(fixture_context(browser, extra="localStorage.setItem('mediaatlas.language','en-US');"))
        english.goto(URL + "/settings?tab=general")
        expect(english.get_by_role("heading", name="Settings", exact=True)).to_be_visible()
        expect(english.get_by_role("checkbox", name="Enable adult media type", exact=True)).to_be_visible()
        assert english.get_by_text("kind", exact=True).count() == 0
        scenarios.append("390×844 without horizontal overflow; reduced motion; visible keyboard focus; directory and source setup layouts")
        assert errors == [], errors
        browser.close()

    report = {
        "date": "2026-11-04", "data": "explicit synthetic IPC and labeled test artwork",
        "runtime": "Actual app rendered in system Chromium via Playwright",
        "source_head": subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
        "working_tree_changes": bool(subprocess.check_output(["git","status","--porcelain"],cwd=ROOT,text=True).strip()),
        "url": URL, "viewports": ["1648×928","390×844"], "scenarios": scenarios,
        "page_errors": errors,
        "limitations": ["Native OS picker/keyring/system association not exercised", "No live Provider or real media directory", "No native startup or packaging claim"],
        "screenshots": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(CAPTURED)},
    }
    (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    settings_tabs_qa()
    print(json.dumps({"result":"passed","output":str(OUT)}, ensure_ascii=False))


if __name__ == "__main__":
    if "--settings-only" in sys.argv:
        settings_tabs_qa()
    else:
        main()
