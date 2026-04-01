# scripts/tests 说明

## 分类规则

### 本地手工验证探针

这些脚本主要用于人工联调、连真实服务验证：

- `chat_tools_hit_probe.ps1`
- `chat_tools_hit_probe.cmd`
- `codex_stream_probe.ps1`
- `gateway_regression_suite.ps1`
- `web_runtime_probe.ps1`
- `web_ui_smoke.ps1`

特点：

- 大多数脚本需要本地 service 已启动
- 大多数脚本需要真实 `Base` / `ApiKey` / `Model`
- 结果更偏 smoke / compatibility probe，而不是纯离线单元测试
- `web_ui_smoke.ps1` 例外：它使用本地 mock Web 运行壳验证页面级兼容，不依赖真实 service

### 可进入 CI 的脚本测试

- `assert-release-version.test.ps1`
- `gateway_regression_suite.test.ps1`
- `rebuild.test.ps1`
- `web_runtime_probe.test.ps1`

特点：

- 不依赖真实 OpenAI 上游
- 更适合验证参数解析、串联关系、版本约束与脚本返回行为
- `assert-release-version.test.ps1` 会额外覆盖 workspace / 前端包 / Tauri 版本一致性约束

## 推荐执行顺序

1. 改脚本参数或流程：先跑对应 `.test.ps1`
2. 改协议适配或转发：再跑 `gateway_regression_suite.ps1`
3. 改 tools/tool_calls：至少补跑 `chat_tools_hit_probe.ps1` 与 `-Stream`
4. 改 responses/chat stream：补跑 `codex_stream_probe.ps1`
5. 改 Web 运行壳、代理或部署方式：补跑 `web_runtime_probe.ps1`
6. 改 Web 页面兼容、弹窗交互或运行时降级：补跑 `web_ui_smoke.ps1`

## 示例

```powershell
pwsh -NoLogo -NoProfile -File scripts/tests/gateway_regression_suite.ps1 `
  -Base http://localhost:48760 -ApiKey <key> -Model gpt-5.3-codex
```

```powershell
pwsh -NoLogo -NoProfile -File scripts/tests/web_runtime_probe.ps1 `
  -Base http://localhost:48761
```

```powershell
pwsh -NoLogo -NoProfile -File scripts/tests/web_ui_smoke.ps1 -SkipBuild
```

## 维护约定

- 新增真实联调探针时，优先放在本目录并明确参数依赖
- 若脚本可以脱离真实服务运行，应补对应 `.test.ps1`
- 不要把 CI 断言和真实联调逻辑塞进同一个脚本里
