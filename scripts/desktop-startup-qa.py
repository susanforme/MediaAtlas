"""Measure the actual production frontend and smoke-test deferred routes.

Run after pnpm build. Optionally pass --baseline <earlier dist directory> to
compare cold browser contexts. This measures local HTTP Chromium rendering, not
native WebView creation, SQLite startup, operating-system caching or real IPC.
Requires Python Playwright and a system Chromium installation.
"""

import argparse
from contextlib import contextmanager
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import hashlib
from pathlib import Path
import statistics
import subprocess
import threading
from urllib.parse import urlparse

from playwright.sync_api import expect, sync_playwright


ROOT = Path(__file__).resolve().parents[1]


class AppHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        # Production SPA navigation resolves to the same packaged entry file.
        if not Path(urlparse(self.path).path).suffix:
            self.path = "/index.html"
        super().do_GET()

    def log_message(self, *_args):
        pass


@contextmanager
def serve(directory):
    server = ThreadingHTTPServer(
        ("127.0.0.1", 0), partial(AppHandler, directory=str(directory))
    )
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_port}"
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def cold_home(browser, address, directory):
    context = browser.new_context(viewport={"width": 1648, "height": 928})
    page = context.new_page()
    client = context.new_cdp_session(page)
    client.send("Emulation.setCPUThrottlingRate", {"rate": 4})
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.add_init_script("""(() => {
      window.__startup = {};
      const observer = new MutationObserver(() => {
        if (!__startup.shell && document.querySelector('.app-sidebar'))
          __startup.shell = performance.now();
        if (!__startup.home && document.querySelector('.home-page'))
          __startup.home = performance.now();
      });
      observer.observe(document, {childList:true, subtree:true});
    })();""")
    page.goto(address, wait_until="networkidle")
    expect(page.locator(".home-page")).to_be_visible()
    expect(page.get_by_role("link", name="音乐", exact=True)).to_be_visible()
    expect(page.get_by_role("link", name="音乐工具", exact=True)).to_be_visible()
    measurement = page.evaluate("""() => ({
      shell_ms: __startup.shell,
      home_ms: __startup.home,
      first_contentful_paint_ms:
        performance.getEntriesByName('first-contentful-paint')[0]?.startTime,
      scripts: performance.getEntriesByType('resource')
        .filter(entry => new URL(entry.name).pathname.endsWith('.js'))
        .map(entry => new URL(entry.name).pathname),
    })""")
    assert not errors, errors
    scripts = sorted(set(measurement["scripts"]))
    measurement["scripts"] = scripts
    measurement["javascript_bytes"] = sum(
        (directory / path.lstrip("/")).stat().st_size for path in scripts
    )
    context.close()
    return measurement


def deferred_routes(browser, address, screenshots):
    context = browser.new_context(viewport={"width": 1648, "height": 928})
    page = context.new_page()
    errors = []
    page.on("pageerror", lambda error: errors.append(str(error)))
    page.goto(address, wait_until="networkidle")
    if screenshots:
        screenshots.mkdir(parents=True, exist_ok=True)
        page.screenshot(path=str(screenshots / "startup-browser-home-1648.png"))
    initial_scripts = page.evaluate("""() => performance.getEntriesByType('resource')
      .filter(entry => new URL(entry.name).pathname.endsWith('.js')).length""")
    # Use in-app navigation: loading a new chunk must preserve the shell.
    for path in ["/providers", "/network", "/settings", "/roots", "/library",
                 "/scraper", "/tools", "/music", "/diagnostics"]:
        page.locator(f'.app-sidebar a[href="{path}"]').click()
        page.wait_for_url(f"{address}{path}")
        expect(page.locator(".app-sidebar")).to_be_visible()
        expect(page.locator("main h1")).to_be_visible()
    final_scripts = page.evaluate("""() => performance.getEntriesByType('resource')
      .filter(entry => new URL(entry.name).pathname.endsWith('.js')).length""")
    assert final_scripts > initial_scripts, "Routes were not deferred"
    assert not errors, errors
    context.close()
    return {"initial_script_requests": initial_scripts,
            "after_navigation_script_requests": final_scripts,
            "routes_checked": 9, "page_errors": errors}


def bootstrap_without_script(browser, address, screenshots):
    context = browser.new_context(
        viewport={"width": 1648, "height": 928}, java_script_enabled=False
    )
    page = context.new_page()
    page.goto(address, wait_until="load")
    expect(page.get_by_role("status")).to_have_text("MediaAtlas")
    expect(page.locator("body")).to_have_css("background-color", "rgb(16, 16, 22)")
    if screenshots:
        page.screenshot(path=str(screenshots / "startup-bootstrap-scripts-disabled-1648.png"))
    context.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--current", type=Path, default=ROOT / "apps/desktop/dist")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--output", type=Path, default=ROOT / "work/startup-report.json")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--baseline-ref", default="unspecified")
    parser.add_argument("--screenshots", type=Path)
    args = parser.parse_args()
    assert args.samples > 0
    if args.screenshots:
        args.screenshots = args.screenshots.resolve()
    report = {
        "environment": "Chromium production build on local HTTP; 4x CPU slowdown; cold browser contexts",
        "scope": "No desktop host or real IPC. Timings are local samples, not native launch benchmarks.",
        "baseline_ref": args.baseline_ref,
        "current_source": {
            "git_head": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
            "uncommitted_changes": bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()),
        },
        "builds": {},
    }
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(
            executable_path="/usr/bin/chromium", args=["--no-sandbox"]
        )
        for name, path in [("baseline", args.baseline), ("current", args.current)]:
            if path is None:
                continue
            path = path.resolve()
            assert (path / "index.html").is_file(), path
            with serve(path) as address:
                samples = [cold_home(browser, address, path) for _ in range(args.samples)]
                report["builds"][name] = {
                    "samples": samples,
                    "median": {key: statistics.median(sample[key] for sample in samples)
                               for key in ["shell_ms", "home_ms", "first_contentful_paint_ms", "javascript_bytes"]},
                }
                if name == "current":
                    report["navigation"] = deferred_routes(browser, address, args.screenshots)
                    bootstrap_without_script(browser, address, args.screenshots)
                    report["bootstrap_without_javascript"] = "passed; native startup not measured"
                    report["current_entry_sha256"] = {
                        str(asset.relative_to(path)): hashlib.sha256(asset.read_bytes()).hexdigest()
                        for asset in path.glob("assets/index-*.js")
                    }
        browser.close()
    if args.screenshots:
        report["screenshots"] = {
            str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in args.screenshots.glob("startup-*.png")
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"report": str(args.output), "builds": {
        name: value["median"] for name, value in report["builds"].items()
    }, "navigation": report["navigation"]}, indent=2))


if __name__ == "__main__":
    main()
