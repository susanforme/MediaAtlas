"""Durable manual-review browser QA with explicitly synthetic IPC.

Run only after the application and queue contract are stable. Requires running
Vite, Python Playwright and Chromium. Fixtures enforce command order and retain
their ledger across browser reloads; they do NOT prove Rust persistence, native
filesystem authorization, a real provider match or metadata publication.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import subprocess
from urllib.parse import parse_qs, urlencode, urlparse

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "screenshots/review-queue-2026-11-04"
os.environ["MEDIAATLAS_QA_OUTPUT_DIR"] = str(OUT)
ui = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))
workflow = runpy.run_path(str(ROOT / "scripts/desktop-usability-qa.py"))
URL = os.environ.get("MEDIAATLAS_QA_URL", "http://localhost:1420")


def uid(number):
    return f"00000000-0000-4000-8000-{number:012x}"


ROOT_ID = uid(500)
CARDS = [
    {**ui["CARDS"][index % len(ui["CARDS"])],
     "media": {**ui["CARDS"][index % len(ui["CARDS"])]["media"],
               "id": uid(index + 1), "title": f"QA 核对媒体 {index + 1:02}", "year": 2020},
     "health": ["incomplete_metadata"]}
    for index in range(35)
]

FIXTURE = r"""
(() => {
  const previous = __TAURI_INTERNALS__.invoke;
  const wrap = data => ({schema_version:1,data});
  const uid = n => `00000000-0000-4000-8000-${n.toString(16).padStart(12,'0')}`;
  const key = 'mediaatlas-review-fixture-v1';
  const stamp = '2026-11-04T00:00:00Z';
  const state = JSON.parse(localStorage.getItem(key) || 'null') || {
    queues:[],calls:[],runs:{},previews:{},counter:1000,
    rootEnabled:true,deleted:[],authConfigured:true,failSearch:false,expired:[]
  };
  const save = () => localStorage.setItem(key,JSON.stringify(state));
  const failure = code => { throw {schema_version:1,code,message:'Synthetic QA failure',correlation_id:uid(999)}; };
  const clone = value => structuredClone(value);
  const root = {id:uid(500),name:'QA 核对目录',path:'/QA/Review',kind:'movie',enabled:true,
    path_style:'posix',exclude_patterns:[],last_scan_at:null,created_at:stamp,updated_at:stamp};
  __qa.authConfigured = state.authConfigured;
  __qa.roots = [root];
  function view(queue) {
    const copy=clone(queue);
    for (const item of copy.items) {
      item.reason=state.deleted.includes(item.media_id)?'missing_media':!state.rootEnabled?'unavailable_media':state.expired.includes(item.media_id)?'stale_checkpoint':null;
      item.needs_recheck=item.reason!==null;
      if (!item.needs_recheck && state.previews[item.preview_id]?.confirmed) item.state='confirmed';
    }
    if (!copy.items.some(item=>item.media_id===copy.current_media_id && item.state==='pending'))
      copy.current_media_id=copy.items.find(item=>item.state==='pending')?.media_id ?? null;
    return copy;
  }
  function queueFor(input) {
    const queue=state.queues.find(q=>q.id===input.queue_id);
    if (!queue) failure('not_found');
    return queue;
  }
  function advance(queue,id) {
    queue.current_media_id=queue.items.find(item=>item.media_id!==id && item.state==='pending')?.media_id ?? null;
  }
  function field(key,value) {return {key,value,language:'zh-CN'};}
  window.__reviewFixture={
    snapshot:()=>clone(state),
    patch:patch=>{Object.assign(state,patch);__qa.authConfigured=state.authConfigured;save();},
    clearCalls:()=>{state.calls=[];save();}
  };
  __TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
    const input=args.input||{};
    state.calls.push({cmd,input:cmd==='provider_auth_save'?{...input,secret:'[synthetic-redacted]'}:clone(input)});
    save();
    if (cmd==='library_list_roots') return wrap([{...root,enabled:state.rootEnabled}]);
    if (cmd==='library_tools_media') {
      let records=__reviewCards.filter(card=>!state.deleted.includes(card.media.id));
      if (input.root_id && input.root_id!==root.id) records=[];
      if (input.query) records=records.filter(card=>card.media.title.includes(input.query));
      if (input.kind) records=records.filter(card=>card.media.kind===input.kind);
      if (input.health) records=records.filter(card=>card.health.includes(input.health));
      if (input.sort==='title_asc') records=records.toSorted((a,b)=>a.media.title.localeCompare(b.media.title));
      const offset=Number(input.cursor?.replace('qa-review-page-','')||0);
      const limit=input.limit||24;
      return wrap({items:records.slice(offset,offset+limit),next_cursor:offset+limit<records.length?`qa-review-page-${offset+limit}`:null});
    }
    if (cmd==='library_tools_detail' || cmd==='library_get_media') {
      if (state.deleted.includes(input.media_id)||!state.rootEnabled) failure('not_found');
      const response=await previous('library_get_media',args);
      return wrap({...response.data,poster_uri:state.noPoster?null:response.data.poster_uri,metadata_values:[],files:response.data.files.map(file=>({...file,root_id:root.id}))});
    }
    if (cmd==='provider_auth_save') {
      state.authConfigured=true;save();
      return previous(cmd,args);
    }
    if (cmd==='review_queue_create') {
      if (!Array.isArray(input.media_ids)||input.media_ids.length<1||input.media_ids.length>32||new Set(input.media_ids).size!==input.media_ids.length||!state.rootEnabled||input.media_ids.some(id=>!__reviewCards.some(card=>card.media.id===id)||state.deleted.includes(id))) failure('invalid_input');
      const queue={id:uid(++state.counter),revision:0,current_media_id:input.media_ids[0],context:clone(input.context),
        items:input.media_ids.map((media_id,position)=>({media_id,position,state:'pending',checkpoint:null,preview_id:null,needs_recheck:false,reason:null})),created_at:stamp,updated_at:stamp};
      state.queues.unshift(queue);save();return wrap(view(queue));
    }
    if (cmd==='review_queue_list') return wrap(state.queues.slice(0,20).map(view));
    if (cmd==='review_queue_get') return wrap(view(queueFor(input)));
    if (cmd==='review_queue_checkpoint'||cmd==='review_queue_update') {
      const queue=queueFor(input);
      if (input.expected_revision!==queue.revision) failure('conflict');
      const item=queue.items.find(item=>item.media_id===input.media_id);
      if (!item) failure('not_found');
      if (cmd==='review_queue_checkpoint') {
        const preview=state.previews[input.preview_id];
        if (!preview||preview.media_id!==item.media_id||preview.confirmed||queue.current_media_id!==item.media_id||view(queue).items[item.position].needs_recheck) failure('conflict');
        item.checkpoint=uid(++state.counter);item.preview_id=input.preview_id;
      } else if (input.action==='select'||input.action==='retry') {
        if (!state.rootEnabled||state.deleted.includes(item.media_id)||item.state==='confirmed') failure('conflict');
        Object.assign(item,{state:'pending',checkpoint:null,preview_id:null});
        state.expired=state.expired.filter(id=>id!==item.media_id);queue.current_media_id=item.media_id;
      } else if (input.action==='confirmed') {
        if (input.checkpoint!==item.checkpoint||!item.checkpoint||!state.previews[item.preview_id]?.confirmed) failure('conflict');
        item.state='confirmed';advance(queue,item.media_id);
      } else if (['defer','failed'].includes(input.action)) {
        if (queue.current_media_id!==item.media_id||item.state!=='pending') failure('conflict');
        Object.assign(item,{state:input.action==='defer'?'deferred':'failed',checkpoint:null,preview_id:null});
        advance(queue,item.media_id);
      } else failure('invalid_input');
      queue.revision++;save();return wrap(view(queue));
    }
    if (cmd==='scraper_search') {
      if (state.failSearch) failure('network_error');
      const card=__reviewCards.find(card=>card.media.title===input.query.title)||__reviewCards[0];
      const run_id=uid(++state.counter);
      const identity={provider_id:'tmdb',remote_id:'qa-remote-'+card.media.id,title:card.media.title,
        original_title:null,year:card.media.year,aliases:[],external_ids:[],source_url:'https://example.invalid/review-fixture',language:'zh-CN'};
      const fields=[field('title',card.media.title),field('overview','明确标记的核对队列浏览器测试资料。')];
      state.runs[run_id]={media_id:card.media.id,identity,fields};save();
      return wrap({schema_version:1,run_id,failures:[],candidates:[{candidate:{kind:'movie',data:identity},reasons:['title_exact'],needs_review:false,duplicate_of:null}]});
    }
    if (cmd==='scraper_detail') {
      const run=state.runs[input.run_id];if (!run) failure('not_found');
      return wrap({kind:'movie',data:{identity:run.identity,fields:run.fields,artwork:[],runtime_minutes:90}});
    }
    if (cmd==='scraper_preview') {
      if (state.failPreview) failure('network_error');
      const run=state.runs[input.run_id];if (!run) failure('not_found');
      const preview_id=uid(++state.counter);
      state.previews[preview_id]={media_id:input.media_item_id,confirmed:false};save();
      return wrap({schema_version:1,preview_id,media_item_id:input.media_item_id,
        fields:run.fields.map(field=>({key:field.key,language:'zh-CN',old:null,new:field,provider_id:'tmdb',source_url:run.identity.source_url,protected:false,needs_review:false,selected:input.selected_fields.includes(field.key)})),artwork:[],confirmed:false});
    }
    if (cmd==='scraper_confirm') {
      const preview=state.previews[input.preview_id];
      if (!preview||!state.queues.some(q=>q.items.some(item=>item.preview_id===input.preview_id&&item.checkpoint))) failure('conflict');
      preview.confirmed=true;save();
      return wrap({schema_version:1,preview_id:input.preview_id,applied_fields:2,already_applied:false});
    }
    if (cmd==='scraper_output_execute') failure('invalid_input');
    return previous(cmd,args);
  };
})();
"""


def make_context(browser, narrow=False):
    context = browser.new_context(viewport={"width": 390, "height": 844} if narrow else {"width": 1648, "height": 928},
                                  reduced_motion="reduce", timezone_id="Asia/Shanghai")
    context.add_init_script("(" + ui["ADAPTER"] + ")(" + json.dumps(CARDS) + ");" + workflow["WORKFLOW"]
                            + ";const __reviewCards=" + json.dumps(CARDS) + ";" + FIXTURE)
    return context


def snapshot(page):
    return page.evaluate("__reviewFixture.snapshot()")


def patch(page, **values):
    page.evaluate("values=>__reviewFixture.patch(values)", values)


def capture(page, filename, full=False):
    ui["no_overflow"](page)
    page.screenshot(path=str(OUT / filename), full_page=full)


def no_writes(page):
    assert not any(call["cmd"] in {"scraper_confirm", "scraper_output_execute", "scan_confirm"}
                   for call in snapshot(page)["calls"]), "Restoring/navigating a queue wrote metadata or files"


def wait_state(page, position, state):
    page.wait_for_function("([position,state])=>__reviewFixture.snapshot().queues[0]?.items[position]?.state===state", arg=[position, state])


def create_selected(page, count=3):
    for index in range(count):
        page.get_by_role("checkbox", name=f"选择刮削：QA 核对媒体 {index + 1:02}", exact=True).check()
    page.get_by_role("button", name="刮削所选", exact=True).click()
    expect(page).to_have_url(re.compile(r"/scraper\?.*session="))
    return parse_qs(urlparse(page.url).query)["session"][0]


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    checks = []
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"),
                                    headless=True, args=["--no-sandbox"])
        context = make_context(browser)
        page = context.new_page()
        errors = []
        page.on("pageerror", lambda error: errors.append(str(error)))
        filters = {"root": ROOT_ID, "q": "QA 核对", "kind": "movie", "health": "incomplete_metadata", "sort": "title_asc"}
        page.goto(URL + "/tools?" + urlencode(filters))
        search_field = page.get_by_label("搜索本地候选", exact=True)
        search_field.evaluate("el => { const m = document.querySelector('.app-main'); m.scrollTop = el.getBoundingClientRect().top + m.scrollTop - 20; }")
        search_field.focus()
        assert search_field.evaluate("el => el.getBoundingClientRect().top >= document.querySelector('.app-toolbar').getBoundingClientRect().bottom")
        checks.append("keyboard focus scrolls the right content below the fixed toolbar")
        session = create_selected(page)
        expect(page.get_by_label("影片名称", exact=True)).to_have_value("QA 核对媒体 01")
        assert snapshot(page)["queues"][0]["context"] == filters
        no_writes(page)
        badge = page.locator('[data-slot="badge"]')
        expect(badge).to_have_count(1)
        expect(badge).to_have_text("待核对")
        assert badge.evaluate("el => el.tagName === 'SPAN' && !el.hasAttribute('tabindex')")
        expect(page.get_by_role("button", name=re.compile(r"\d+.*待核对"))).to_have_count(0)
        capture(page, "fixture-created-session-1648.png")
        page.get_by_role("button", name="暂缓此项", exact=True).click()
        wait_state(page, 0, "deferred")
        page.reload()
        expect(page.get_by_text(re.compile(r"暂缓 1 项"))).to_be_visible()
        assert not any(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"])
        no_writes(page)
        checks.append("create session and defer persist across actual browser reload without writes")
        capture(page, "fixture-restored-session-1648.png")

        # Configure and return carries persistent session and current target.
        patch(page, authConfigured=False)
        page.reload()
        page.get_by_role("link", name="配置凭据", exact=True).click()
        expect(page).to_have_url(re.compile(r"/settings\?.*session="))
        assert parse_qs(urlparse(page.url).query)["session"] == [session]
        page.get_by_label("凭据", exact=True).fill("synthetic-qa-only")
        page.get_by_role("button", name="保存认证", exact=True).click()
        expect(page.get_by_label("凭据", exact=True)).to_have_value("")
        page.get_by_role("link", name="返回刮削", exact=True).click()
        expect(page.get_by_label("影片名称", exact=True)).to_have_value("QA 核对媒体 02")
        no_writes(page)
        capture(page, "fixture-settings-return-1648.png")
        checks.append("manual credential configuration returns to same persistent session and media")

        patch(page, failSearch=True)
        page.get_by_role("button", name="检索来源", exact=True).click()
        wait_state(page, 1, "failed")
        searches_before_reload = sum(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"])
        page.reload()
        expect(page.get_by_text(re.compile(r"失败 1 项"))).to_be_visible()
        assert sum(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"]) == searches_before_reload
        no_writes(page)
        capture(page, "fixture-persisted-failure-1648.png")
        checks.append("search failure persists and reload does not retry or write")

        patch(page, failSearch=False)
        searches_before = sum(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"])
        page.get_by_role("button", name="重新核对此项", exact=True).click()
        wait_state(page, 1, "pending")
        assert sum(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"]) == searches_before
        page.get_by_role("button", name="检索来源", exact=True).click()
        expect(page.get_by_role("button", name="选择此候选", exact=True)).to_be_visible()
        no_writes(page)
        capture(page, "fixture-explicit-failure-retry-1648.png")
        page.get_by_role("button", name="暂缓此项", exact=True).click()
        wait_state(page, 1, "deferred")
        checks.append("failed item requires explicit recheck and a separate search; successful retry does not confirm")
        # Resume the remaining pending target explicitly through the saved list.
        searches_before_resume = sum(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"])
        page.goto(URL + "/tools")
        page.get_by_role("link", name="继续核对", exact=True).first.click()
        expect(page.get_by_label("影片名称", exact=True)).to_have_value("QA 核对媒体 03")
        assert sum(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"]) == searches_before_resume
        no_writes(page)
        page.get_by_role("button", name="检索来源", exact=True).click()
        page.get_by_role("button", name="选择此候选", exact=True).click()
        expect(page.get_by_role("heading", name="合并媒体元数据", exact=True)).to_be_visible()
        confirm_merge = page.get_by_role("button", name="确认合并（2 处变更）", exact=True)
        expect(confirm_merge).to_be_enabled()
        expect(page.get_by_role("button", name="预览合并", exact=True)).to_have_count(0)
        no_writes(page)
        page.locator(".app-main").evaluate("el => el.scrollTop = 0")
        capture(page, "fixture-live-merge-1648.png", full=True)
        sidebar_before = page.locator(".app-sidebar").bounding_box()
        settings_before = page.locator(".app-sidebar-bottom").bounding_box()
        page.locator(".app-main").hover()
        page.mouse.wheel(0, 900)
        page.wait_for_function("document.querySelector('.app-main').scrollTop > 0")
        assert page.locator(".app-sidebar").bounding_box() == sidebar_before
        assert page.locator(".app-sidebar-bottom").bounding_box() == settings_before
        assert page.evaluate("window.scrollY === 0 && document.documentElement.scrollHeight <= innerHeight")
        assert page.locator(".app-main").evaluate("el => el.scrollWidth <= el.clientWidth + 1")
        capture(page, "fixture-fixed-navigation-scroll-1648.png")
        page.locator(".app-main").evaluate("el => el.scrollTop = 0")
        checks.append("wheel scroll moves only right content; sidebar and bottom settings stay fixed while document scroll remains zero")
        page.get_by_role("tab", name="合并后", exact=True).focus()
        page.keyboard.press("ArrowRight")
        expect(page.get_by_role("tab", name="对比", exact=True)).to_be_focused()
        expect(page.get_by_role("tab", name="对比", exact=True)).to_have_attribute("aria-selected", "true")
        capture(page, "fixture-merge-compare-1648.png", full=True)
        page.keyboard.press("ArrowLeft")
        page.get_by_role("checkbox", name=re.compile(r"^简介 zh-CN")).uncheck()
        confirm_one = page.get_by_role("button", name="确认合并（1 处变更）", exact=True)
        expect(confirm_one).to_be_disabled()
        expect(confirm_one).to_be_enabled()
        page.get_by_role("checkbox", name="仅合并空缺字段和图片", exact=True).check()
        expect(page.get_by_text("当前选择没有变更。", exact=True)).to_be_visible()
        expect(page.get_by_role("button", name="确认合并（0 处变更）", exact=True)).to_be_disabled()
        page.get_by_role("button", name="全选未保护字段", exact=True).click()
        expect(confirm_one).to_be_enabled()
        capture(page, "fixture-merge-missing-only-1648.png", full=True)
        patch(page, failPreview=True)
        page.get_by_role("checkbox", name="仅合并空缺字段和图片", exact=True).uncheck()
        expect(page.get_by_role("alert")).to_be_visible()
        expect(confirm_merge).to_be_disabled()
        no_writes(page)
        capture(page, "fixture-merge-validation-error-1648.png", full=True)
        patch(page, failPreview=False)
        page.get_by_role("button", name="重新检查", exact=True).click()
        expect(confirm_merge).to_be_enabled()
        no_writes(page)
        checks.append("live merged view and keyboard comparison update with selection; missing-only and zero-change states; validation failure blocks confirmation until explicit retry")
        confirm_merge.click()
        wait_state(page, 2, "confirmed")
        commands = snapshot(page)["calls"]
        proof = [call["cmd"] for call in commands if call["cmd"] in {"scraper_preview", "review_queue_checkpoint", "scraper_confirm"}
                 or call["cmd"] == "review_queue_update" and call["input"].get("action") == "confirmed"]
        assert proof[-4:] == ["scraper_preview", "review_queue_checkpoint", "scraper_confirm", "review_queue_update"], proof
        assert not any(call["cmd"] == "scraper_output_execute" for call in commands)
        capture(page, "fixture-confirmed-proof-order-1648.png")
        checks.append("explicit confirmation uses preview then checkpoint then confirm then proven ledger update; no file execution")
        page.get_by_role("link", name="返回媒体清单", exact=True).click()
        assert all(parse_qs(urlparse(page.url).query).get(key) == [value] for key, value in filters.items())
        checks.append("return restores original server filters and sort")

        page.goto(URL + "/scraper?" + urlencode({"session": session, "media": uid(1)}))
        patch(page, expired=[uid(1)])
        page.reload()
        expect(page.get_by_text("之前的预览已失效，请重新核对。", exact=True)).to_be_visible()
        expect(page.get_by_role("button", name=re.compile(r"^确认合并"))).to_have_count(0)
        page.evaluate("__reviewFixture.clearCalls()")
        page.get_by_role("button", name="重新核对此项", exact=True).click()
        wait_state(page, 0, "pending")
        no_writes(page)
        assert not any(call["cmd"] == "scraper_search" for call in snapshot(page)["calls"])
        checks.append("expired checkpoint requires explicit recheck and never replays confirmation")
        patch(page, rootEnabled=False)
        page.reload()
        expect(page.get_by_text("此媒体已移除或所在目录不可用。恢复目录后刷新队列。", exact=True)).to_be_visible()
        expect(page.get_by_role("button", name="检索来源", exact=True)).to_have_count(0)
        capture(page, "fixture-disabled-root-1648.png")
        patch(page, rootEnabled=True, deleted=[uid(1)])
        page.reload()
        expect(page.get_by_text("此媒体已移除或所在目录不可用。恢复目录后刷新队列。", exact=True)).to_be_visible()
        checks.append("disabled directory and removed media block continuation")
        assert errors == [], errors

        bounded = make_context(browser).new_page()
        bounded.goto(URL + "/tools")
        bounded.get_by_role("checkbox", name="选择本页支持刮削的媒体", exact=True).check()
        expect(bounded.get_by_role("button", name="已选择 24 项", exact=True)).to_be_visible()
        bounded.get_by_role("button", name="下一页候选", exact=True).click()
        bounded.get_by_role("checkbox", name="选择本页支持刮削的媒体", exact=True).click()
        expect(bounded.get_by_role("button", name="已选择 24 项", exact=True)).to_be_visible()
        for index in range(24, 32):
            bounded.get_by_role("checkbox", name=f"选择刮削：QA 核对媒体 {index + 1:02}", exact=True).check()
        expect(bounded.get_by_role("button", name="已选择 32 项", exact=True)).to_be_visible()
        bounded.get_by_role("button", name="刮削所选", exact=True).click()
        bounded.wait_for_function("__reviewFixture.snapshot().queues[0]?.items.length===32")
        assert len([c for c in snapshot(bounded)["calls"] if c["cmd"] == "review_queue_create"]) == 1
        expect(bounded.get_by_role("combobox", name="当前条目", exact=True)).to_be_visible()
        expect(bounded.get_by_role("combobox", name="当前条目", exact=True).get_by_role("option")).to_have_count(32)
        expect(bounded.locator('[data-slot="badge"]')).to_have_count(1)
        expect(bounded.get_by_role("button", name=re.compile(r"\d+.*待核对"))).to_have_count(0)
        checks.append("32 items use a non-interactive shared Badge and compact navigation rather than status buttons")
        no_writes(bounded)
        checks.append("cross-page 32-item bound rejects whole overflowing page addition without truncation")

        mobile = make_context(browser, narrow=True).new_page()
        mobile.goto(URL + "/tools")
        patch(mobile, noPoster=True)
        create_selected(mobile, 2)
        expect(mobile.get_by_label("影片名称", exact=True)).to_have_value("QA 核对媒体 01")
        assert mobile.evaluate("matchMedia('(prefers-reduced-motion: reduce)').matches")
        button = mobile.get_by_role("button", name="暂缓此项", exact=True)
        button.focus()
        mobile.keyboard.press("Enter")
        wait_state(mobile, 0, "deferred")
        no_writes(mobile)
        capture(mobile, "fixture-keyboard-narrow-390.png", full=True)
        checks.append("390×844 no page overflow, reduced motion and keyboard defer without writes")
        mobile.get_by_role("button", name="检索来源", exact=True).click()
        mobile.get_by_role("button", name="选择此候选", exact=True).click()
        expect(mobile.get_by_role("button", name="确认合并（2 处变更）", exact=True)).to_be_enabled()
        no_writes(mobile)
        expect(mobile.get_by_text("暂无海报", exact=True)).to_be_visible()
        capture(mobile, "fixture-live-merge-narrow-390.png", full=True)
        checks.append("live merge stacks at 390×844 with no overflow, missing-poster fallback and no automatic confirmation")
        mobile.set_viewport_size({"width": 390, "height": 360})
        settings_before = mobile.locator(".app-sidebar-bottom").bounding_box()
        nav = mobile.locator(".app-nav")
        assert nav.evaluate("el => el.scrollHeight > el.clientHeight")
        nav.evaluate("el => el.scrollTop = el.scrollHeight")
        assert nav.evaluate("el => el.scrollTop > 0")
        assert mobile.locator(".app-sidebar-bottom").bounding_box() == settings_before
        assert settings_before["y"] + settings_before["height"] <= 360
        expect(mobile.get_by_role("link", name="设置", exact=True)).to_be_visible()
        capture(mobile, "fixture-fixed-navigation-short-390.png")
        checks.append("short 390×360 viewport scrolls only sidebar navigation items internally and keeps bottom settings visible")
        browser.close()

    report = {
        "date": "2026-11-04", "mode": "real Chromium + synthetic IPC only",
        "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
        "worktree_diff_sha256": hashlib.sha256(subprocess.check_output(["git", "diff", "HEAD"], cwd=ROOT)).hexdigest(),
        "queue_source_sha256": {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
                                for pattern in ["apps/desktop/src/components/ui/badge.tsx","apps/desktop/src/components/ui/tabs.tsx", "apps/desktop/src/components/shell/app-shell.tsx", "apps/desktop/src/components/shell/shell.css", "apps/desktop/src/features/scraper/*merge*.ts*", "apps/desktop/src/api/review-queue.ts", "apps/desktop/src/features/scraper/*queue*.ts*", "apps/desktop/src/features/scraper/page.tsx", "apps/desktop/src/features/scraper/use-scraper.ts", "apps/desktop/src/features/scraper/recovery-*.tsx", "apps/desktop/src/features/tools/ordinary-candidates.tsx", "apps/desktop/src/features/settings/page.tsx", "apps/desktop/src/features/providers/pages.tsx", "crates/infrastructure/src/review_queue*.rs", "crates/infrastructure/src/review_queue/*.rs", "crates/application/src/review_queue.rs", "scripts/desktop-review-queue-qa.py"]
                                for path in ROOT.glob(pattern) if path.is_file()},
        "checks": checks,
        "limits": ["No native Tauri, real provider, keyring or filesystem execution", "Browser fixture validates command ordering only; Rust service proof/persistence is separate"],
        "screenshots": {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(OUT.glob("*.png"))},
    }
    (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print("Durable review browser QA passed:", "; ".join(checks))


if __name__ == "__main__":
    main()
