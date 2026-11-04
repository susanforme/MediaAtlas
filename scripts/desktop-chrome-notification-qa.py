"""Real Chromium UI QA with explicitly synthetic IPC/window fixtures, never production code."""
import hashlib
import json
import os
import re
import runpy
import subprocess
from pathlib import Path

from playwright.sync_api import expect, sync_playwright
from select_support import select_option

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "screenshots/chrome-navigation-2026-11-04"
URL = os.environ.get("MEDIAATLAS_QA_URL", "http://127.0.0.1:1432")
ui = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))
CARDS = [dict(ui["CARDS"][i % 7], media=dict(ui["CARDS"][i % 7]["media"], id=f"qa-{i}", title=f"QA 媒体 {i+1:02}")) for i in range(21)]
WINDOW = r"""(platform) => {
  Object.defineProperty(navigator, 'platform', {get: () => platform, configurable:true});
  const native = window.__TAURI_INTERNALS__;
  native.metadata = {currentWindow:{label:'main'},currentWebview:{label:'main'}};
  const original = native.invoke;
  const callbacks = new Map();
  const events = new Map();
  let callbackId = 10;
  window.__qa.window = {maximized:false,focused:true,fail:false,calls:[]};
  native.transformCallback = callback => { const id = ++callbackId; callbacks.set(id, callback); return id; };
  native.unregisterCallback = id => callbacks.delete(id);
  window.__qa.emit = (event,payload) => {
    for (const id of events.get(event) || []) callbacks.get(id)?.({event,id,payload});
  };
  native.invoke = async (cmd,args={}) => {
    if (cmd === 'plugin:event|listen') { events.set(args.event,[...(events.get(args.event)||[]),args.handler]); return args.handler; }
    if (cmd === 'plugin:event|unlisten') return null;
    if (cmd.startsWith('plugin:window|')) {
      const state = window.__qa.window;
      state.calls.push({cmd,args});
      if (cmd.endsWith('|is_maximized')) return state.maximized;
      if (cmd.endsWith('|is_focused')) return state.focused;
      if (state.fail) throw new Error('Synthetic sensitive path /private/fixture must not render');
      if (cmd.endsWith('|toggle_maximize')) state.maximized = !state.maximized;
      if (cmd.endsWith('|minimize')) await new Promise(resolve=>setTimeout(resolve,200));
      return null;
    }
    if (cmd === 'scan_list_tasks') return {schema_version:1,data:[]};
    if (cmd === 'scraper_media_detail') return {schema_version:1,data:null};
    return original(cmd,args);
  };
  localStorage.setItem('mediaatlas.language','zh-CN');
}"""


def capture(page, name):
    ui["no_overflow"](page)
    assert page.locator('.app-main').evaluate('el=>el.scrollWidth<=el.clientWidth+1')
    page.screenshot(path=str(OUT / name))


def context(browser, platform):
    result = browser.new_context(viewport={"width":1648,"height":928}, reduced_motion="reduce")
    result.add_init_script("(" + ui["ADAPTER"] + ")(" + json.dumps(CARDS) + ");(" + WINDOW + ")(" + json.dumps(platform) + ");")
    return result


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    observations = {}
    errors = []
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get('MEDIAATLAS_CHROMIUM','/usr/bin/chromium'), headless=True, args=['--no-sandbox'])
        ctx = context(browser,'Win32')
        page = ctx.new_page()
        page.on('pageerror',lambda error:errors.append(str(error)))
        page.goto(URL+'/library')
        nav = page.get_by_role('navigation',name='本地工作区')
        parent = nav.get_by_role('link',name='媒体库',exact=True)
        movie = nav.get_by_role('link',name='电影',exact=True)
        tv = nav.get_by_role('link',name='电视剧',exact=True)
        anime = nav.get_by_role('link',name='动画',exact=True)
        controls = page.get_by_role('group',name='窗口控制')
        expect(controls).to_be_visible()
        expect(page.get_by_role('button',name='最大化窗口',exact=True)).to_be_enabled()
        expect(nav.get_by_role('link',name='成人视频',exact=True)).to_have_count(0)
        expect(parent).to_have_class(re.compile(r'\bactive\b'))
        expect(page.get_by_role('link',name=re.compile(r'QA 媒体 01')).first).to_be_visible()
        capture(page,'fixture-windows-library-1648.png')

        movie.click()
        expect(page).to_have_url(re.compile(r'kind=movie'))
        page.get_by_role('searchbox',name='搜索标题',exact=True).fill('QA')
        select_option(page, page.get_by_role('combobox', name='排序', exact=True), 'year_desc')
        page.get_by_role('button',name='缺海报',exact=True).click()
        expect(page).to_have_url(re.compile(r'health=missing_poster'))
        expect(parent).to_have_class(re.compile(r'\bactive\b'))
        expect(movie).to_have_class(re.compile(r'\bactive\b'))
        expect(tv).not_to_have_class(re.compile(r'\bactive\b'))
        capture(page,'fixture-library-filter-selection-1648.png')
        tv.click()
        expect(page).to_have_url(re.compile(r'kind=tv_series'))
        expect(tv).to_have_class(re.compile(r'\bactive\b'))
        anime.click()
        expect(page).to_have_url(re.compile(r'kind=anime'))
        expect(anime).to_have_class(re.compile(r'\bactive\b'))
        nav.get_by_role('link',name='音乐',exact=True).click()
        expect(page).to_have_url(URL+'/music')
        expect(parent).to_have_class(re.compile(r'\bactive\b'))
        parent.click()
        expect(page).to_have_url(URL+'/library')
        expect(page.get_by_role('combobox',name='媒体类型',exact=True)).to_have_attribute("value", '')
        expect(movie).not_to_have_class(re.compile(r'\bactive\b'))
        sidebar = page.locator('.app-sidebar')
        before = sidebar.bounding_box()
        page.locator('.app-main').evaluate('el=>el.scrollTop=el.scrollHeight')
        assert page.locator('.app-main').evaluate('el=>el.scrollTop') > 0
        assert sidebar.bounding_box() == before
        assert page.evaluate('window.scrollY') == 0
        capture(page,'fixture-fixed-sidebar-scroll-1648.png')
        page.locator('.app-main').evaluate('el=>el.scrollTop=0')

        toolbar_controls = page.locator('.app-toolbar-actions').bounding_box()
        window_bounds = controls.bounding_box()
        assert toolbar_controls['x'] + toolbar_controls['width'] <= window_bounds['x']
        assert page.locator('.app-toolbar').get_attribute('data-tauri-drag-region') == 'deep'
        assert page.locator('.app-toolbar-actions').get_attribute('data-tauri-drag-region') == 'false'
        assert page.locator('#library-toolbar-search').get_attribute('data-tauri-drag-region') == 'false'
        assert controls.get_attribute('data-tauri-drag-region') == 'false'
        maximum = page.get_by_role('button',name='最大化窗口',exact=True)
        maximum.click()
        expect(page.get_by_role('button',name='还原窗口',exact=True)).to_have_attribute('aria-pressed','true')
        page.get_by_role('button',name='还原窗口',exact=True).click()
        expect(page.get_by_role('button',name='最大化窗口',exact=True)).to_have_attribute('aria-pressed','false')
        minimum = page.get_by_role('button',name='最小化窗口',exact=True)
        minimum.evaluate('el=>{el.click();el.click()}')
        expect(minimum).to_be_enabled()
        assert page.evaluate("__qa.window.calls.filter(x=>x.cmd==='plugin:window|minimize').length") == 1
        page.get_by_role('button',name='关闭窗口',exact=True).click()
        expect(page.get_by_role('button',name='关闭窗口',exact=True)).to_be_enabled()
        assert page.evaluate("__qa.window.calls.filter(x=>x.cmd==='plugin:window|close').length") == 1
        page.evaluate("__qa.window.focused=false;__qa.emit('tauri://blur',null)")
        expect(controls).to_have_attribute('data-focused','false')
        page.evaluate("__qa.window.focused=true;__qa.emit('tauri://focus',null)")
        expect(controls).to_have_attribute('data-focused','true')
        page.evaluate('__qa.window.fail=true')
        minimum.click()
        expect(page.get_by_role('alert')).to_contain_text('窗口操作未完成')
        assert '/private/fixture' not in page.locator('body').inner_text()
        capture(page,'fixture-window-error-recovery-1648.png')
        page.evaluate('__qa.window.fail=false')
        page.get_by_role('button',name='重试窗口控制',exact=True).click()
        expect(page.get_by_role('alert')).to_have_count(0)
        expect(minimum).to_be_enabled()
        observations['windows_commands'] = page.evaluate('__qa.window.calls')

        # Real production NativeOpen mutation succeeds through synthetic IPC, showing one Sonner toast.
        page.get_by_role('link',name=re.compile(r'QA 媒体 01')).first.click()
        expect(parent).to_have_class(re.compile(r'\bactive\b'))
        page.get_by_role('button',name='使用系统应用打开',exact=True).click()
        dialog = page.get_by_role('dialog')
        expect(dialog).to_be_visible()
        dialog.get_by_role('button',name='QA-version-1.mkv',exact=True).click()
        expect(dialog).to_have_count(0)
        toast = page.locator('[data-sonner-toast]')
        expect(toast).to_have_count(1)
        expect(toast).to_contain_text('已提交系统打开请求')
        expect(page.locator('[data-sonner-toaster]')).to_have_count(1)
        expect(page.locator('p[role="status"]',has_text='已提交系统打开请求')).to_have_count(0)
        region = page.get_by_role('region',name=re.compile('通知'))
        expect(region).to_have_count(1)
        expect(region).to_have_attribute('aria-live','polite')
        expect(toast).to_be_visible()
        assert page.evaluate("matchMedia('(prefers-reduced-motion: reduce)').matches")
        observations['toast_transition'] = toast.evaluate('el=>getComputedStyle(el).transitionDuration')
        assert all(float(x.strip().removesuffix('s')) == 0 for x in observations['toast_transition'].split(',')), observations
        capture(page,'fixture-sonner-native-open-1648.png')
        dismiss = page.get_by_role('button',name='关闭通知',exact=True)
        dismiss.focus()
        page.keyboard.press('Enter')
        expect(toast).to_have_count(0)
        observations['native_open_requests'] = page.evaluate("__qa.calls.filter(x=>x.cmd==='library_open')")
        assert len(observations['native_open_requests']) == 1

        page.set_viewport_size({'width':390,'height':844})
        parent.click()
        expect(movie).to_be_visible()
        movie.focus()
        page.keyboard.press('Enter')
        expect(page).to_have_url(re.compile('kind=movie'))
        expect(movie).to_be_focused()
        expect(movie).to_have_class(re.compile(r'\bactive\b'))
        assert movie.evaluate("el=>el.matches(':focus-visible')")
        narrow_controls = controls.bounding_box()
        narrow_actions = page.locator('.app-toolbar-actions').bounding_box()
        assert narrow_controls['y']+narrow_controls['height'] <= narrow_actions['y']
        capture(page,'fixture-windows-narrow-keyboard-390.png')
        ctx.close()

        mac = context(browser,'MacIntel')
        page = mac.new_page()
        page.on('pageerror',lambda error:errors.append(str(error)))
        page.goto(URL+'/library')
        expect(page.locator('.app-shell')).to_have_attribute('data-window-platform','macos')
        expect(page.locator('.window-controls')).to_have_count(0)
        brand = page.locator('.app-brand')
        assert brand.evaluate("el=>getComputedStyle(el).paddingTop") == '32px'
        assert page.evaluate('__qa.window.calls.length') == 0
        expect(page.get_by_role('link',name=re.compile(r'QA 媒体 01')).first).to_be_visible()
        capture(page,'fixture-macos-overlay-spacing-1648.png')
        page.set_viewport_size({'width':390,'height':844})
        capture(page,'fixture-macos-narrow-390.png')
        mac.close()
        browser.close()
    assert not errors, errors
    sources = [
        'apps/desktop/src/components/shell/app-shell.tsx',
        'apps/desktop/src/components/shell/shell.css',
        'apps/desktop/src/components/shell/library-navigation.tsx',
        'apps/desktop/src/components/shell/use-window-controls.ts',
        'apps/desktop/src/components/shell/window-controls.tsx',
        'apps/desktop/src/components/shell/window-chrome.css',
        'apps/desktop/src/components/ui/sonner.tsx',
        'apps/desktop/src/components/ui/sonner.css',
        'apps/desktop/src/features/library/native-open.tsx',
    ]
    report = {
        'date':'2026-11-04', 'runtime':'Real Chromium browser, synthetic window/IPC and clearly labelled test artwork; no native system control is executed',
        'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
        'source_sha256':{name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in sources},
        'observations':observations,
        'checks':['clickable library parent clears category','movie/tv/anime links preserve active state with q/sort/health','adult category hidden by disabled setting','fixed sidebar while right main scrolls','Windows buttons dispatch real SDK commands into explicit mock; maximize/restore state, duplicate minimize suppression, focus, safe error/retry','drag opt-out on actual interactive regions','toolbar/control non-overlap at1648 and390','one Sonner toast on system-open request, closes version picker, no inline success duplicate, localized region/dismiss, keyboard dismissal','reduced motion removes toast transitions','Mac Overlay reserve only, no fabricated HTML traffic lights','no page errors or horizontal overflow'],
        'limits':['No native Windows/macOS host or OS interactions verified. Synthetic close/minimize do not change the browser window.','Mac screenshots show reserved space; system traffic lights exist only in the real Tauri Overlay window.','No real library, provider credentials, files, or native application were used.'],
        'screenshots':{path.name:hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(OUT.glob('*.png'))},
    }
    (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('Chrome/navigation/Sonner browser QA passed')


if __name__ == '__main__':
    main()
