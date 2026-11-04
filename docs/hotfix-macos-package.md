# macOS 安装包资源签名修复

## 已确认的问题

用户在 Apple Silicon 上安装 `MediaAtlas_0.1.0-beta.2_aarch64-apple-darwin.dmg` 后，执行完整性验证得到：

```text
code has no resources but signature indicates they must be present
```

这份文件的架构正确。实际从项目 Release 下载的 beta.2 ARM64 DMG 的 SHA-256 为 `200283ac34ca35f0ad5b630c9de58706510631f30e2fda622282d6bf3ce34ab0`，与其官方 `SHA256SUMS` 和发布 manifest 一致；解包后，`.app` 有 `Contents/Resources`，却没有 `Contents/_CodeSignature/CodeResources`。Mach-O 只有 linker 生成的 ad-hoc 签名（CodeDirectory flags `0x20002`、special slots 为 0），不能代替资源齐全后的完整应用封装签名。实际 beta.4 ARM64 DMG 同样没有 `CodeResources`。

旧 release overlay 没有配置 macOS signing identity。Tauri 未对完成的 `.app` 做最终签名；发布检查只执行 `codesign --display` 读取签名类型，错误地把“存在二进制签名”当成了足够的检查。该读取不会验证资源封装，故旧 manifest 中的 `ad-hoc` 不证明应用完整性。

## 修复与门槛

- release overlay 明确 `bundle.macOS.signingIdentity = "-"`，由 Tauri 在复制资源之后、创建 DMG 之前完整签署应用；不引入证书、账户或伪造公证。
- `stage` 对实际只读挂载 DMG 内的 `.app` 要求非空 `CodeResources`，执行 `codesign --verify --deep --strict --verbose=2`。任何失败阻止 staging，不在验收阶段修补或重新签署产物。
- manifest 记录 `macos_signature_verified: true`；聚合拒绝缺失、false 或非布尔值的验证记录，仍保留架构、版本、系统 WebKit 与全部许可证字节检查。
- main 的 macOS job 使用真实 clang、codesign 和 hdiutil 验证完整 ad-hoc 应用、缺资源封装、签名后篡改资源及只读挂载 DMG 的回归；合成原生 fixture 不等同最终 Tauri GUI 或下载隔离标记验收。

完整 ad-hoc 签名仍不等于 Developer ID 签名或 Apple 公证。修复后 macOS 仍可能要求在“系统设置 → 隐私与安全”明确允许打开；不能承诺像公证应用一样无提示安装。

## 已安装旧包的临时处理

只针对从项目 Release 获得的已安装应用，可在本机重新生成资源封装签名，随后验证，成功才清除该应用的下载隔离标记：

```bash
codesign --force --deep --sign - --timestamp=none "/Applications/MediaAtlas.app" &&
codesign --verify --deep --strict "/Applications/MediaAtlas.app" &&
xattr -dr com.apple.quarantine "/Applications/MediaAtlas.app"
```

若签名或验证报错，保留错误输出继续排查。新发布包的验收不能由这个用户端修复命令代替。

## 当前验证

Linux 本地发布工具回归：61 项中 55 项执行通过，6 项因缺少原生工具未执行（4 项 macOS、1 项 Rust exporter、1 项 NSIS）。CI 工具 7 项通过。macOS 原生回归和最终 DMG 的实际结果需分别由新 main CI 与包含本修复的新 tag 发布流水线记录，旧包编译成功不能代替。

修复提交 `0fdfae812a2a02fbdb1953b96a6e76a52512d589` 的 [真实 macOS job](https://github.com/susanforme/MediaAtlas/actions/runs/41766592580/job/126366280323) 在新增原生步骤中执行 8 项测试，全部通过（含 clang 本机程序执行、删除资源封装拒绝、签名后篡改拒绝、只读挂载 DMG 验签）。该 job 随后在相邻前端异步测试失败，不能称整个 job 已成功；相关页面测试已另行同步修复。本地使用现有 Rust 编译器补跑后，发布工具 61 项中 56 项通过，5 项平台工具测试未在 Linux 执行。最终发布版本为 beta.6，实际 Tauri DMG 仍须通过新 tag 的严格验签门槛。
