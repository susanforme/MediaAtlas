"""Shared native-select visual and keyboard QA, with explicitly synthetic IPC."""
import hashlib
import json
import os
import re
from pathlib import Path
import runpy
import subprocess
from playwright.sync_api import expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "screenshots/shared-select-2026-11-04"
URL = os.environ.get("MEDIAATLAS_QA_URL", "http://127.0.0.1:1432")
ui = runpy.run_path(str(ROOT / "scripts/desktop-ui-qa.py"))


def capture(page, name):
    ui["no_overflow"](page)
    assert page.locator(".app-main").evaluate("el => el.scrollWidth <= el.clientWidth + 1")
    page.screenshot(path=str(OUT / name))


def styles(control):
    return control.evaluate("""el => { const s = getComputedStyle(el); return {
      appearance:s.appearance,webkitAppearance:s.webkitAppearance,
      backgroundImage:s.backgroundImage,backgroundSize:s.backgroundSize,
      backgroundColor:s.backgroundColor,paddingRight:s.paddingRight,
      cursor:s.cursor,opacity:s.opacity,fontSize:s.fontSize,
      borderColor:s.borderColor,boxShadow:s.boxShadow
    }; }""")


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    observations = {}
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=os.environ.get("MEDIAATLAS_CHROMIUM", "/usr/bin/chromium"), headless=True, args=["--no-sandbox"])
        context = browser.new_context(viewport={"width":1648,"height":928}, reduced_motion="reduce")
        context.add_init_script("(" + ui["ADAPTER"] + ")(" + json.dumps(ui["CARDS"]) + ");__qa.mode=localStorage.getItem('qa-select-mode')||'content';")
        context.add_init_script("localStorage.setItem('mediaatlas.language','zh-CN')")
        page = context.new_page()
        errors = []
        page.on("pageerror", lambda error: errors.append(str(error)))
        page.goto(URL + "/library")
        kind = page.get_by_role("combobox", name="媒体类型", exact=True)
        sort = page.get_by_role("combobox", name="排序", exact=True)
        language = page.locator("select.app-language")
        expect(kind).to_be_visible()
        expect(page.get_by_role("link", name="QA 媒体 01", exact=False).first).to_be_visible()
        for name, control in [("kind", kind), ("sort", sort), ("language", language)]:
            value = styles(control)
            assert value["appearance"] == "none" and value["webkitAppearance"] == "none", value
            assert value["backgroundImage"].count("linear-gradient") == 2, value
            assert all(size.strip() == "6px 6px" for size in value["backgroundSize"].split(",")), value
            observations[name] = value
        assert observations["kind"]["fontSize"] == "14px", observations["kind"]
        assert observations["language"]["fontSize"] == "12px", observations["language"]
        capture(page, "fixture-shared-select-library-1648.png")

        kind.focus()
        kind.press("ArrowDown")
        kind.press("Enter")
        kind.press("Escape")
        expect(kind).to_have_value("movie")
        expect(page).to_have_url(re.compile(r"[?&]kind=movie"))
        kind.press("Tab")
        page.keyboard.press("Shift+Tab")
        expect(kind).to_be_focused()
        assert kind.evaluate("el => el.matches(':focus-visible')")
        assert styles(kind)["boxShadow"] != "none"
        capture(page, "fixture-shared-select-focus-1648.png")

        # Native DOM state fixtures exercise the shared styles, not business errors.
        kind.evaluate("el => el.setAttribute('aria-invalid', 'true')")
        sort.evaluate("el => { el.disabled = true; }")
        assert styles(sort)["cursor"] == "not-allowed", styles(sort)
        assert float(styles(sort)["opacity"]) < 0.6
        expect(sort).to_be_disabled()
        assert styles(kind)["borderColor"] != observations["kind"]["borderColor"]
        capture(page, "fixture-shared-select-states-1648.png")
        kind.evaluate("el => el.removeAttribute('aria-invalid')")
        sort.evaluate("el => { el.disabled = false; }")

        page.evaluate("localStorage.setItem('qa-select-mode', 'empty')")
        page.goto(URL + "/library")
        expect(page.locator(".library-poster-card")).to_have_count(0)
        capture(page, "fixture-shared-select-empty-1648.png")
        language.select_option("en-US")
        expect(page.get_by_role("heading", name="Media library", exact=True)).to_be_visible()
        assert styles(language)["backgroundImage"].count("linear-gradient") == 2
        language.select_option("zh-CN")
        page.set_viewport_size({"width": 390, "height": 844})
        expect(language).to_be_visible()
        assert float(styles(language)["paddingRight"].removesuffix("px")) >= 30
        assert language.bounding_box()["width"] >= 90
        assert page.evaluate("matchMedia('(prefers-reduced-motion: reduce)').matches")
        capture(page, "fixture-shared-select-narrow-390.png")
        assert not errors, errors
        browser.close()

    source = ["apps/desktop/src/components/ui/select.tsx", "apps/desktop/src/components/ui/select.css", "apps/desktop/src/styles.css", "apps/desktop/src/components/shell/shell.css", "apps/desktop/src/features/library/library.css"]
    report = {"date":"2026-11-04", "mode":"real Chromium + synthetic IPC; disabled/invalid are explicit DOM style fixtures", "source_commit":subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip(),
              "source_sha256":{name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in source}, "observations":observations,
              "checks":["appearance and WebKit appearance reset on library and language controls", "single theme chevron and consistent typography", "keyboard selection updates real library URL", "keyboard focus visible", "disabled cursor/opacity and invalid border", "empty/content, language switching, 390px and reduced motion, no page or right-content overflow"],
              "limits":["Chromium browser QA; native macOS WKWebView and OS popup appearance are not verified here", "Native selection semantics and change events retained; popup menu remains platform-managed"],
              "screenshots":{p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(OUT.glob('*.png'))}}
    (OUT/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('Shared Select browser QA passed')


if __name__ == '__main__':
    main()
