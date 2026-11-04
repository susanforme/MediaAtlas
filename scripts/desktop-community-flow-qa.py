"""Verify contextual metadata recovery against the actual UI using synthetic IPC."""
import hashlib
import json
import os
from pathlib import Path
import runpy
from urllib.parse import parse_qs, urlparse

from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
SUPPORT = runpy.run_path(str(ROOT / 'scripts/desktop-library-wall-qa.py'))
URL = SUPPORT['URL']
OUT = Path(os.environ.get('MEDIAATLAS_QA_OUTPUT_DIR', ROOT / 'screenshots/community-flow-2026-11-04'))

DETAIL = r'''(() => {
const previous = __TAURI_INTERNALS__.invoke;
__TAURI_INTERNALS__.invoke=async(cmd,args={})=>{
 const value=await previous(cmd,args);
 if(cmd==='library_get_media') value.data.health=HEALTH;
 return value;
};
})();'''


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    errors = []
    files = []
    def capture(page, name):
        SUPPORT['BASE']['no_overflow'](page)
        page.screenshot(path=str(OUT / name))
        files.append(OUT / name)
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path='/usr/bin/chromium', args=['--no-sandbox'])
        def page_for(health=(), narrow=False):
            ctx = SUPPORT['fixture_context'](browser, narrow=narrow, extra=DETAIL.replace('HEALTH', json.dumps(health)))
            page = ctx.new_page()
            page.on('pageerror', lambda error: errors.append(str(error)))
            return page
        page = page_for()
        page.goto(URL + '/library?root=qa-root&kind=movie&q=QA&sort=title_asc&cursor=old&page=2')
        expect(page.locator('.poster-card')).to_have_count(7)
        # The URL and outbound IPC both retain scope and reset obsolete pagination.
        for label, health in [('待核对', 'metadata_needs_review'), ('来源未配置', 'metadata_unconfigured'), ('资料失败', 'metadata_failed'), ('缺海报', 'missing_poster')]:
            page.get_by_role('button', name=label, exact=True).click()
            query = parse_qs(urlparse(page.url).query)
            assert query['health'] == [health] and query['root'] == ['qa-root'] and query['q'] == ['QA']
            assert 'cursor' not in query and query.get('page', ['1']) == ['1']
            expect(page.get_by_role('button', name=label, exact=True)).to_have_attribute('aria-pressed','true')
        page.get_by_role('button', name='全部', exact=True).click()
        assert 'health' not in parse_qs(urlparse(page.url).query)
        capture(page, 'fixture-quick-repair-filters-1648.png')
        for health, name in [(['metadata_unconfigured'], 'fixture-detail-configure-source-1648.png'), (['metadata_pending'], 'fixture-detail-identifying-1648.png')]:
            page = page_for(health)
            page.goto(URL + '/media/qa-0')
            expect(page.get_by_role('heading', name='QA 媒体 01',exact=True)).to_be_visible()
            if health == ['metadata_unconfigured']:
                action = page.get_by_role('link', name='配置来源并继续',exact=True)
                expect(action).to_be_visible()
                assert 'root=qa-root' in action.get_attribute('href')
                assert page.get_by_role('link', name='更正识别',exact=True).count() == 0
                capture(page, name)
                action.click()
                expect(page.get_by_role('tab',name='来源认证',exact=True)).to_have_attribute('aria-selected','true')
                assert parse_qs(urlparse(page.url).query)['root'] == ['qa-root']
                assert not any(c['cmd'] in ('provider_set_enabled','provider_auth_save','library_sync') for c in page.evaluate('__qa.calls'))
            else:
                expect(page.get_by_role('status').filter(has_text='正在识别')).to_be_visible()
                assert page.get_by_role('link', name='配置来源并继续',exact=True).count() == 0
                capture(page, name)
        page = page_for(narrow=True)
        page.goto(URL + '/library')
        expect(page.locator('.poster-card')).to_have_count(7)
        capture(page, 'fixture-quick-repair-filters-390.png')
        for state, code, expected in [('failed','invalid_root','目录不可用。'), ('cancelled',None,'已收录内容和已保存资料保留')]:
            task = {**SUPPORT['UNCONFIGURED_TASK'], 'state':state, 'error_code':code, 'unconfigured_count':0, 'review_count':0}
            ctx = SUPPORT['fixture_context'](browser, extra='__qa.tasks='+json.dumps([task])+';')
            page = ctx.new_page()
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.goto(URL+'/scanner?root=qa-root&task=qa-unconfigured')
            expect(page.get_by_text(expected,exact=False)).to_be_visible()
            if state == 'failed':
                expect(page.get_by_role('link',name='管理目录',exact=True)).to_be_visible()
            else:
                page.get_by_role('button',name='重新更新目录',exact=True).click()
                expect(page.get_by_text('正在获取媒体资料',exact=True).first).to_be_visible()
                assert sum(call['cmd']=='library_sync' for call in page.evaluate('__qa.calls'))==1
            capture(page, 'fixture-scan-recovery-'+state+'-1648.png')
        assert errors == [], errors
        browser.close()
    report = {'scope':'Actual Chromium UI; explicit synthetic IPC, metadata and labeled artwork; no native/live provider acceptance', 'page_errors':errors, 'screenshots':{file.name:hashlib.sha256(file.read_bytes()).hexdigest() for file in files}}
    (OUT/'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n')
    print('Community recovery UI passed: scoped filters, pagination reset, contextual source setup, pending state, narrow reduced motion.')


if __name__ == '__main__':
    main()
