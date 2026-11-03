"""Exercise directory -> scan review -> scrape using test-only IPC fixtures.

The OS folder window and real provider/filesystem services are NOT simulated
as proof of native execution. These are real browser UI screenshots with an
explicit fixture transport, including picker cancellation and unavailable auth.
"""

import json
import os
from pathlib import Path
import re
import runpy

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
support = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))
URL = support["URL"]
cards = support["CARDS"]
capture = support["capture"]

WORKFLOW = r"""
(() => {
  const previous = window.__TAURI_INTERNALS__.invoke;
  const wrap = data => ({schema_version: 1, data});
  const stamp = '2026-11-03T00:00:00Z';
  Object.assign(__qa, { roots: [], tasks: [], pickerResult: '/QA/Movies', authConfigured: true });
  const identity = {provider_id:'tmdb', remote_id:'qa-remote', title:'QA 媒体 01',
    original_title:null,year:2020,aliases:[],external_ids:[{namespace:'tmdb',value:'qa-remote'}],
    source_url:'https://example.invalid/qa-film',language:'zh-CN'};
  const fields = [{key:'title',value:'QA 媒体 01',language:'zh-CN'},
    {key:'overview',value:'明确标注的浏览器流程测试资料。',language:'zh-CN'}];
  window.__TAURI_INTERNALS__.invoke = async (cmd, args = {}) => {
    const input = args.input || {};
    const commands = ['plugin:dialog|open','library_list_roots','library_save_root',
      'scan_start','scan_list_tasks','scan_get_preview','scan_confirm','provider_snapshot',
      'scraper_history','library_tools_detail','provider_auth_status','scraper_search',
      'scraper_detail','scraper_preview','scraper_confirm','scraper_output_preview',
      'scraper_output_discard','organization_history','scraper_media_detail','provider_auth_save'];
    if (!commands.includes(cmd)) return previous(cmd, args);
    __qa.calls.push({cmd, input: cmd === 'plugin:dialog|open' ? args : input});
    if (cmd === 'plugin:dialog|open') return __qa.pickerResult;
    if (cmd === 'library_list_roots') return wrap(__qa.roots);
    if (cmd === 'library_save_root') {
      const root = {...input,id:input.id || 'qa-root',path_style:'posix',
        created_at:stamp,updated_at:stamp,last_scan_at:null};
      __qa.roots = [root];
      return wrap(root);
    }
    if (cmd === 'scan_start') {
      const task = {id:'qa-scan',root_id:input.root_id,state:'waiting_for_user',
        visited_files:1,skipped_files:0,candidate_count:1,imported_count:0,
        warnings:0,error_code:null,created_at:stamp,updated_at:stamp};
      __qa.tasks = [task];
      return wrap(task);
    }
    if (cmd === 'scan_list_tasks') return wrap(__qa.tasks);
    if (cmd === 'scan_get_preview') return wrap({task_id:'qa-scan',next_cursor:null,items:[{
      id:'qa-candidate',root_id:'qa-root',relative_path:'QA media 01.mkv',size_bytes:1024,
      modified_at:null,kind:'movie',title:'QA 媒体 01',year:2020,part:null,
      series_title:null,season_number:null,episode_number:null,absolute_number:null,
      catalog_number:null,needs_review:false,warnings:[],existing_media_id:null,
      status:__qa.tasks[0]?.state === 'succeeded' ? 'imported' : 'pending'}]});
    if (cmd === 'scan_confirm') {
      __qa.tasks = __qa.tasks.map(task => ({...task,state:'succeeded',imported_count:1}));
      return wrap({imported:1,unchanged:0,needs_review:0});
    }
    if (cmd === 'provider_snapshot') return wrap({schema_version:1,tasks:[],requests:[],providers:[{
      descriptor:{id:'tmdb',name:'TMDB',description:'QA provider fixture',website:null,
        terms_url:null,privacy_url:null,documentation_url:null,engine_types:['api'],
        requires_api_key:true,requires_login:false,rate_limit_policy:'QA',attribution:'QA',
        data_usage_notes:'Test only',license:'QA',media_kinds:['movie','tv_series'],
        languages:['zh-CN','en-US'],capabilities:['search'],fixture:false},
      config:{enabled:true,revision:1,timeout_ms:10000,concurrency:1,image_concurrency:1,
        interval_ms:250,max_response_bytes:2097152,max_retries:1,credential_ref:null},health:'ready'}]});
    if (cmd === 'provider_auth_save') __qa.authConfigured = true;
    if (cmd === 'provider_auth_status' || cmd === 'provider_auth_save') return wrap({provider_id:input.provider_id,
      supported_methods:['bearer','api_key'],method:'bearer',storage:'system',
      configured:__qa.authConfigured,status:__qa.authConfigured ? 'configured' : 'not_configured'});
    if (cmd === 'library_tools_detail') return previous('library_get_media', args);
    if (cmd === 'scraper_history' || cmd === 'organization_history') return wrap([]);
    if (cmd === 'scraper_media_detail') return wrap(null);
    if (cmd === 'scraper_search') return wrap({schema_version:1,run_id:'qa-run',failures:[],
      candidates:[{candidate:{kind:'movie',data:identity},reasons:['title_exact'],needs_review:false,duplicate_of:null}]});
    if (cmd === 'scraper_detail') return wrap({kind:'movie',data:{identity,fields,artwork:[],runtime_minutes:90}});
    if (cmd === 'scraper_preview') return wrap({schema_version:1,preview_id:'qa-merge',media_item_id:'qa-0',
      fields:fields.map(field => ({key:field.key,language:'zh-CN',old:null,new:field,provider_id:'tmdb',
        source_url:identity.source_url,protected:false,needs_review:false,selected:true})),artwork:[],confirmed:false});
    if (cmd === 'scraper_confirm') return wrap({schema_version:1,preview_id:'qa-merge',applied_fields:2,already_applied:false});
    if (cmd === 'scraper_output_preview') return wrap({schema_version:1,preview_id:'qa-output',media_item_id:'qa-0',
      strategy:'preserve',state:'previewed',files:[{destination:'/QA/Movies/QA media 01.nfo',kind:'nfo',old_text:null,
        new_text:'<movie><title>QA 媒体 01</title></movie>',expected_hash:null,content_hash:'qa-hash',protected:false}]});
    if (cmd === 'scraper_output_discard') return wrap(null);
  };
})();
"""


def context(browser, missing_auth=False, narrow=False):
    result = browser.new_context(viewport={"width": 390, "height": 844} if narrow else {"width": 1648, "height": 928},
                                 reduced_motion="reduce", timezone_id="Asia/Shanghai")
    script = "(" + support["ADAPTER"] + ")(" + json.dumps(cards) + ");" + WORKFLOW
    if missing_auth:
        script += "__qa.authConfigured = false;"
    result.add_init_script(script)
    return result


with sync_playwright() as p:
    browser = p.chromium.launch(executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"),
                                headless=True, args=["--no-sandbox"])
    page = context(browser).new_page()
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.goto(URL + "/roots")
    page.get_by_role("button", name="添加目录", exact=True).click()
    expect(page.get_by_label("绝对路径", exact=True)).to_have_count(0)
    page.evaluate("__qa.pickerResult = null")
    page.get_by_role("button", name="选择文件夹", exact=True).click()
    expect(page.get_by_label("目录名称", exact=True)).to_have_value("")
    assert not any(c["cmd"] == "library_save_root" for c in page.evaluate("__qa.calls"))
    page.evaluate("__qa.pickerResult = '/QA/Movies'")
    page.get_by_role("button", name="选择文件夹", exact=True).click()
    expect(page.get_by_label("目录名称", exact=True)).to_have_value("Movies")
    capture(page, "usability-fixture-directory-1648.png")
    page.get_by_role("button", name="保存并扫描", exact=True).click()
    expect(page).to_have_url(re.compile(r"/scanner\?.*task=qa-scan"))
    expect(page.get_by_text("QA media 01.mkv", exact=True)).to_be_visible()
    calls = page.evaluate("__qa.calls")
    assert len([c for c in calls if c["cmd"] == "library_save_root"]) == 1
    assert len([c for c in calls if c["cmd"] == "scan_start"]) == 1
    assert not any(c["cmd"] == "scan_confirm" for c in calls)
    capture(page, "usability-fixture-scan-review-1648.png")
    page.get_by_role("checkbox", name="选择本页待入库项", exact=True).check()
    page.get_by_role("button", name="确认所选入库", exact=True).click()
    expect(page.get_by_role("dialog", name="确认入库预览")).to_be_visible()
    assert not any(c["cmd"] == "scan_confirm" for c in page.evaluate("__qa.calls"))
    page.get_by_role("button", name="确认入库", exact=True).click()
    expect(page.get_by_role("link", name="浏览已入库媒体", exact=True)).to_be_visible()
    page.goto(URL + "/tools?media=qa-0")
    page.get_by_role("link", name="刮削此媒体", exact=True).click()
    expect(page).to_have_url(URL + "/scraper?media=qa-0")
    expect(page.get_by_label("影片名称", exact=True)).to_have_value("QA 媒体 01")
    expect(page.get_by_label("发行年份（可选）", exact=True)).to_have_value("2020")
    expect(page.get_by_role("button", name="检索来源", exact=True)).to_be_enabled()
    capture(page, "usability-fixture-scrape-context-1648.png")
    page.get_by_role("button", name="检索来源", exact=True).click()
    page.get_by_role("button", name="选择此候选", exact=True).click()
    page.get_by_role("button", name="预览合并", exact=True).click()
    expect(page.get_by_role("button", name="确认所选元数据", exact=True)).to_be_visible()
    assert not any(c["cmd"] == "scraper_confirm" for c in page.evaluate("__qa.calls"))
    capture(page, "usability-fixture-merge-preview-1648.png")
    page.get_by_role("button", name="确认所选元数据", exact=True).click()
    page.get_by_role("button", name="预览 NFO 与海报输出", exact=True).click()
    expect(page.get_by_text("/QA/Movies/QA media 01.nfo", exact=True)).to_be_visible()
    assert not any(c["cmd"] == "scraper_output_execute" for c in page.evaluate("__qa.calls"))
    page.get_by_role("button", name="取消输出预览", exact=True).click()
    expect(page.get_by_text("/QA/Movies/QA media 01.nfo", exact=True)).to_have_count(0)
    assert errors == [], errors

    missing = context(browser, missing_auth=True).new_page()
    missing.goto(URL + "/scraper?media=qa-0")
    expect(missing.get_by_text("TMDB (需要配置凭据)", exact=True)).to_be_visible()
    expect(missing.get_by_role("button", name="检索来源", exact=True)).to_be_disabled()
    expect(missing.get_by_role("link", name="配置凭据", exact=True)).to_have_attribute("href", re.compile(r"/settings\?.*from=scraper.*media=qa-0"))
    capture(missing, "usability-fixture-missing-credentials-1648.png")
    missing.get_by_role("link", name="配置凭据", exact=True).click()
    expect(missing.get_by_role("link", name="返回刮削", exact=True)).to_have_attribute("href", "/scraper?media=qa-0")
    missing.get_by_label("凭据", exact=True).fill("synthetic-qa-credential")
    missing.get_by_role("button", name="保存认证", exact=True).click()
    expect(missing.get_by_label("凭据", exact=True)).to_have_value("")
    expect(missing.get_by_text("认证已保存，来源启用状态保持不变。")).to_be_visible()
    missing.get_by_role("link", name="返回刮削", exact=True).click()
    expect(missing).to_have_url(URL + "/scraper?media=qa-0")
    expect(missing.get_by_label("影片名称", exact=True)).to_have_value("QA 媒体 01")
    expect(missing.get_by_role("button", name="检索来源", exact=True)).to_be_enabled()
    capture(missing, "usability-fixture-return-from-settings-1648.png")

    mobile = context(browser, narrow=True).new_page()
    mobile.goto(URL + "/roots")
    mobile.get_by_role("button", name="添加目录", exact=True).click()
    mobile.get_by_role("button", name="选择文件夹", exact=True).click()
    expect(mobile.get_by_label("目录名称", exact=True)).to_have_value("Movies")
    capture(mobile, "usability-fixture-directory-narrow.png")
    mobile.goto(URL + "/scraper?media=qa-0")
    expect(mobile.get_by_role("button", name="检索来源", exact=True)).to_be_enabled()
    capture(mobile, "usability-fixture-scrape-context-narrow.png")
    browser.close()

print("Usability browser QA passed: picker cancel/autoname, save-and-scan, explicit scan confirmation, contextual scrape, merge/output separation, missing credentials, narrow layout.")
