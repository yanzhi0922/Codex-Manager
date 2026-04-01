# scripts 目录说明

## 分类

### 开发

- `bump-version.ps1`：统一修改版本号
- `rebuild.ps1`：Windows 本地桌面构建，也可触发全平台 release workflow
- `rebuild-linux.sh`：Linux 本地桌面构建
- `rebuild-macos.sh`：macOS 本地桌面构建

### 测试

- `tests/chat_tools_hit_probe.ps1`：`/v1/chat/completions` tools 命中探针
- `tests/codex_stream_probe.ps1`：chat / responses 流式探针
- `tests/gateway_regression_suite.ps1`：协议回归统一入口
- `tests/web_runtime_probe.ps1`：Web 运行壳最小冒烟探针
- `tests/web_ui_smoke.ps1`：Web 管理页页面级冒烟脚本（本地 mock 运行壳）
- `tests/*.test.ps1`：脚本级回归测试

### 发布

- `release/assert-release-version.ps1`
- `release/disable-tauri-before-build.ps1`
- `.github/actions/build-tauri-with-retry`
- `.github/actions/stage-service-package`
- `.github/actions/publish-github-release`

### 仅 CI / workflow 间接调用

以下脚本通常由 workflow 或 composite action 调用，不建议作为日常手工入口：

- `release/assert-release-version.ps1`
- `.github/actions/build-tauri-with-retry`
- `.github/actions/stage-service-package`
- `.github/actions/publish-github-release`

## 使用建议

1. 本地开发优先用顶层入口脚本，不要直接调用过深的 release 辅助脚本
2. 协议验证优先走 `tests/gateway_regression_suite.ps1`
3. Web 代理、部署或运行壳改动，优先补跑 `tests/web_runtime_probe.ps1`
4. Web 页面兼容或交互降级改动，补跑 `tests/web_ui_smoke.ps1`
5. 若脚本只服务 CI，尽量通过 README 或 workflow 注释说明，不要让它伪装成本地通用入口

## 相关文档

- 测试探针说明：[tests/README.md](tests/README.md)
- 构建发布说明：[../docs/release/20260310122606851_构建发布与脚本说明.md](../docs/release/20260310122606851_构建发布与脚本说明.md)
- 职责对照与盘点：[../docs/report/20260309195735631_脚本与发布职责对照.md](../docs/report/20260309195735631_脚本与发布职责对照.md)
