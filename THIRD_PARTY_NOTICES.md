# Third-party notices for MediaAtlas beta

MediaAtlas includes third-party software. Its authors retain their original copyrights and license terms. MediaAtlas's own project source is licensed under the MIT License in `LICENSE.md`; third-party components remain subject to their own terms.

The machine-readable inventory is `docs/dependency-licenses.json`. It enumerates every registry package in both `Cargo.lock` and the multi-document `pnpm-lock.yaml`, including build tools, tests, optional platform tools, and dependencies that are not present in a particular application binary. This intentionally broad inventory is not a claim that every package is distributed or executed on every platform.

Original license, copyright, patent and NOTICE files are preserved byte for byte under `docs/third-party-licenses/texts/`. The inventory maps each dependency to its original filenames, source URLs, SHA-256 hashes, authors where declared, and complete license texts. Identical texts share one hash-named file. Each Release distributes this notice, the inventory, and the complete license directory as separate attachments, including `license-materials.zip`. Application installers and executables do not embed or carry these materials.

## Original-source omissions

Four upstream packages omit a separate license file in their published archive and inspected upstream repository: `fxhash 0.2.1`, `mac 0.1.1`, `@napi-rs/lzma-linux-x64-gnu 1.5.1`, and `stackback 0.0.2`. Their inventory records explicitly use `notice_provenance: upstream_metadata+standard_terms` and explain this limitation. Original available package metadata, source copyright headers and attribution declarations are retained together with the complete standard terms of the license alternative declared by upstream. These materials are identified as assembled attributions; they are not presented as original upstream license files. No copyright year or additional author has been invented. `fxhash` and `mac` use their declared Apache-2.0 alternative. The LZMA and stackback packages are development tooling, and their executables/source are not included in the application packages.

## Source-code availability and modifications

Unmodified MPL-2.0 covered Rust crate sources are supplied in `docs/third-party-licenses/sources/*.crate`. These are the original registry gzip/tar archives, verified against their `Cargo.lock` SHA-256 hashes; extract them with a tar-compatible archive tool. The inventory records the corresponding source for each covered crate. The optional/historical vendored libdbus source archive is also retained with its original license, without representing it as a currently distributed native library. They retain their original notices, including file-level notices. MediaAtlas has not modified those covered source files. These source materials and the full MPL license permit exercising the covered code's source rights without relying on a separate source-code offer.

The npm `lightningcss` MPL packages are development CSS compiler tooling; the compiler binaries and its source are not distributed inside the application. The npm `caniuse-lite` package is CC-BY-4.0 development browser-compatibility data; its original attribution/license is retained in the inventory. No compiler binary or browser-compatibility database is included as an application runtime. Inspect the inventory's source URLs for their exact versions.

## Desktop controls and native directory selection

The desktop shared controls use Radix primitives and their supporting React packages under the original MIT notices recorded in the inventory. Native directory selection uses the official Tauri dialog plugin and `rfd`; their original MIT/Apache license alternatives and attributions are retained. The npm dialog package ships an SPDX declaration rather than complete terms, so its declaration is preserved alongside original full upstream license files fetched from the exact npm version's recorded VCS commit. An SPDX declaration by itself does not pass the inventory's full-license check.

## Native components and platform packaging

- **Rust standard library/runtime 1.90.0**, linked into the application binaries, retains the official pinned toolchain's complete `COPYRIGHT-library.html` content and referenced original license texts under `native/rust/`. The HTML content is saved byte for byte with a `.txt` extension so it remains an immutable notice resource. This covers library/runtime attributions omitted from `Cargo.lock`; compiler/toolchain executables are not distributed.
- **SQLite 3.46.0**, through `libsqlite3-sys 0.30.1`, uses the bundled SQLite amalgamation. Its original public-domain blessing is preserved as `native/SQLite-PUBLIC-DOMAIN.txt`; the Rust binding's MIT license is independently retained. SQLCipher is not enabled or distributed as a separate runtime. The package's extra original notices may appear in the over-inclusive source inventory.
- **WebView2 SDK loader 1.0.3800.47**, through `webview2-com-sys 0.39.1`, retains Microsoft's original BSD-style `LICENSE.txt` and `NOTICE.txt` in `native/WebView2-*`. WebView2 Runtime itself is a separate Microsoft runtime. Runtime installation and use are subject to Microsoft's applicable terms; the SDK loader's BSD license must not be represented as licensing the whole runtime.
- **Windows NSIS installer** uses LZMA compression selected explicitly in the release configuration. `native/NSIS-COPYING.txt` preserves NSIS 3.13's full original copyright and component terms, including the LZMA module's CPL-1.0 terms and explicit NSIS linking exception, plus terms for other compression modules that are not selected. The Tauri `nsis_tauri_utils 0.5.3` plugin retains its original MIT/Apache license files.
- **Linux libdbus** is linked dynamically from the system `libdbus-1-3` package, using the selected system `pkg-config` dependency rather than the keyring `vendored` feature. The Rust binding license does not replace libdbus's own AFL-2.1/GPL-2.0-or-later dual licensing. The over-inclusive source materials also retain the original `libdbus-sys-0.2.7.crate`, including all unmodified optional vendored C sources, build configuration, original copyright headers and full COPYING text. The current release does not compile or redistribute that vendored library; no static LGPL library or OpenSSL is enabled through the credential/TLS graph.
- **Linux deb** contains the application binary, its icons and its notice/resources files. It depends on the operating system's GTK3/WebKitGTK, glibc and related shared libraries instead of redistributing those system libraries. The supported baseline is Ubuntu 22.04 / glibc 2.35 with the package manager resolving the declared runtime dependencies. Those separately installed libraries retain their own licenses and source-code availability provided by the Linux distribution. The application package adds no restriction on replacing compatible system libraries or debugging their modifications. Native package inspection must reject unexpected bundled shared libraries; the Debian package dependency list remains authoritative for installation prerequisites.
- **Linux AppImage is not distributed in this beta.** A future AppImage can bundle GTK/WebKitGTK and other native shared libraries and therefore requires a separate actual-file inventory, full original distribution copyright/license files, corresponding source access and any required relinking instructions. Those materials have not been completed or verified. Rust/npm lockfile coverage alone does not establish AppImage native-library compliance, so the format is excluded from this release instead of claiming that its native materials gate passed.
- **macOS** uses system frameworks/WebKit supplied by Apple. They are not re-licensed by the Rust binding crates. Distribution and system use remain subject to the applicable Apple terms.

The image pipeline uses the Rust `image-webp` implementation for WebP decoding, not a separately packaged native `libwebp` executable/library. Native code vendored by Rust packages, including `ring`/BoringSSL-derived code, retains nested original notices collected from those package archives.

`ffmpeg`, `ffprobe`, and media fixture generation tools are development/test tools only. They are not an application runtime dependency and are not included in release packages. MediaAtlas opens media with the operating system's associated application and does not bundle a player.

## Reproduce and verify

With Python 3.11+, the pinned Node/pnpm toolchain, `pnpm install --frozen-lockfile`, and `cargo fetch --locked` completed, run:

```sh
python3 scripts/licenses/generate.py
python3 scripts/licenses/verify.py
python3 scripts/licenses/verify-native-features.py --target x86_64-unknown-linux-gnu
```

The committed upstream notice cache permits offline regeneration for the existing lockfiles. After changing dependencies, `generate.py --fetch` may obtain missing notices from the exact upstream registry/VCS revision through the normal configured HTTPS proxy. Review newly omitted notices rather than assigning an invented copyright or project license. The verifier checks complete locked-package coverage, material hashes, MPL source archive hashes, and lockfile freshness. Formatting uses the repository's installed Biome executable.

In the source repository, the real dependency audit, remediation and remaining upstream warnings are recorded in `scripts/licenses/security-audit.json` and explained in `scripts/licenses/SECURITY-AUDIT.md`. A license inventory is not a guarantee that dependencies have no security vulnerabilities.
