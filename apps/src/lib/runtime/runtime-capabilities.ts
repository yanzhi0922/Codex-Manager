import type { RuntimeCapabilities, RuntimeMode } from "@/types";

export const DEFAULT_WEB_RPC_BASE_URL = "/api/rpc";
export const DEFAULT_UNSUPPORTED_WEB_REASON =
  "当前页面缺少 Codex-Copilot Web 运行壳，无法访问管理 RPC。请通过 codexmanager-web 打开，或在反向代理中转发 /api/rpc。";

export type RuntimeCapabilityView = {
  runtimeCapabilities: RuntimeCapabilities | null;
  mode: RuntimeMode;
  isDesktopRuntime: boolean;
  isUnsupportedWebRuntime: boolean;
  canAccessManagementRpc: boolean;
  canManageService: boolean;
  canSelfUpdate: boolean;
  canCloseToTray: boolean;
  canOpenLocalDir: boolean;
  canUseBrowserFileImport: boolean;
  canUseBrowserDownloadExport: boolean;
};

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

function asString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function asBoolean(value: unknown, fallback = false): boolean {
  return typeof value === "boolean" ? value : fallback;
}

export function normalizeRpcBaseUrl(value: string | null | undefined): string {
  const normalized = String(value || "").trim();
  if (!normalized) {
    return "";
  }
  return normalized.endsWith("/")
    ? normalized.replace(/\/+$/, "") || DEFAULT_WEB_RPC_BASE_URL
    : normalized;
}

export function isRuntimeMode(value: string): value is RuntimeMode {
  return (
    value === "desktop-tauri" ||
    value === "web-gateway" ||
    value === "unsupported-web"
  );
}

export function buildDesktopRuntimeCapabilities(): RuntimeCapabilities {
  return {
    mode: "desktop-tauri",
    rpcBaseUrl: DEFAULT_WEB_RPC_BASE_URL,
    canManageService: true,
    canSelfUpdate: true,
    canCloseToTray: true,
    canOpenLocalDir: true,
    canUseBrowserFileImport: true,
    canUseBrowserDownloadExport: true,
    unsupportedReason: null,
  };
}

export function buildWebGatewayRuntimeCapabilities(
  rpcBaseUrl = DEFAULT_WEB_RPC_BASE_URL
): RuntimeCapabilities {
  return {
    mode: "web-gateway",
    rpcBaseUrl: normalizeRpcBaseUrl(rpcBaseUrl) || DEFAULT_WEB_RPC_BASE_URL,
    canManageService: false,
    canSelfUpdate: false,
    canCloseToTray: false,
    canOpenLocalDir: false,
    canUseBrowserFileImport: true,
    canUseBrowserDownloadExport: true,
    unsupportedReason: null,
  };
}

export function buildUnsupportedWebCapabilities(
  reason = DEFAULT_UNSUPPORTED_WEB_REASON,
  rpcBaseUrl = DEFAULT_WEB_RPC_BASE_URL
): RuntimeCapabilities {
  return {
    mode: "unsupported-web",
    rpcBaseUrl: normalizeRpcBaseUrl(rpcBaseUrl) || DEFAULT_WEB_RPC_BASE_URL,
    canManageService: false,
    canSelfUpdate: false,
    canCloseToTray: false,
    canOpenLocalDir: false,
    canUseBrowserFileImport: false,
    canUseBrowserDownloadExport: false,
    unsupportedReason: reason,
  };
}

export function normalizeRuntimeCapabilities(
  payload: unknown,
  fallbackRpcBaseUrl = DEFAULT_WEB_RPC_BASE_URL
): RuntimeCapabilities {
  const source = asRecord(payload) ?? {};
  const modeValue = asString(source.mode);
  const mode: RuntimeMode = isRuntimeMode(modeValue) ? modeValue : "web-gateway";
  const defaultCapabilities =
    mode === "desktop-tauri"
      ? buildDesktopRuntimeCapabilities()
      : mode === "unsupported-web"
        ? buildUnsupportedWebCapabilities(undefined, fallbackRpcBaseUrl)
        : buildWebGatewayRuntimeCapabilities(fallbackRpcBaseUrl);

  return {
    mode,
    rpcBaseUrl:
      normalizeRpcBaseUrl(asString(source.rpcBaseUrl)) ||
      defaultCapabilities.rpcBaseUrl,
    canManageService: asBoolean(
      source.canManageService,
      defaultCapabilities.canManageService
    ),
    canSelfUpdate: asBoolean(
      source.canSelfUpdate,
      defaultCapabilities.canSelfUpdate
    ),
    canCloseToTray: asBoolean(
      source.canCloseToTray,
      defaultCapabilities.canCloseToTray
    ),
    canOpenLocalDir: asBoolean(
      source.canOpenLocalDir,
      defaultCapabilities.canOpenLocalDir
    ),
    canUseBrowserFileImport: asBoolean(
      source.canUseBrowserFileImport,
      defaultCapabilities.canUseBrowserFileImport
    ),
    canUseBrowserDownloadExport: asBoolean(
      source.canUseBrowserDownloadExport,
      defaultCapabilities.canUseBrowserDownloadExport
    ),
    unsupportedReason:
      asString(source.unsupportedReason) || defaultCapabilities.unsupportedReason || null,
  };
}

export function resolveRuntimeCapabilityView(
  runtimeCapabilities: RuntimeCapabilities | null,
  desktopFallback: boolean
): RuntimeCapabilityView {
  const resolvedCapabilities = runtimeCapabilities ??
    (desktopFallback
      ? buildDesktopRuntimeCapabilities()
      : buildUnsupportedWebCapabilities());
  const mode = resolvedCapabilities.mode;
  const isDesktopRuntime = mode === "desktop-tauri";

  return {
    runtimeCapabilities,
    mode,
    isDesktopRuntime,
    isUnsupportedWebRuntime: mode === "unsupported-web",
    canAccessManagementRpc: mode !== "unsupported-web",
    canManageService: resolvedCapabilities.canManageService,
    canSelfUpdate: resolvedCapabilities.canSelfUpdate,
    canCloseToTray: resolvedCapabilities.canCloseToTray,
    canOpenLocalDir: resolvedCapabilities.canOpenLocalDir,
    canUseBrowserFileImport: resolvedCapabilities.canUseBrowserFileImport,
    canUseBrowserDownloadExport: resolvedCapabilities.canUseBrowserDownloadExport,
  };
}
