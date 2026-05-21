export type AvailabilityLevel = "ok" | "warn" | "bad" | "unknown";

export type RuntimeMode = "desktop-tauri" | "web-gateway" | "unsupported-web";

export interface RuntimeCapabilities {
  mode: RuntimeMode;
  rpcBaseUrl: string;
  canManageService: boolean;
  canSelfUpdate: boolean;
  canCloseToTray: boolean;
  canOpenLocalDir: boolean;
  canUseBrowserFileImport: boolean;
  canUseBrowserDownloadExport: boolean;
  unsupportedReason?: string | null;
}

export interface ServiceStatus {
  connected: boolean;
  version: string;
  uptime: number;
  addr: string;
}

export interface PlatformDiscoveryTotals {
  ready: number;
  detected: number;
  missing: number;
  planned: number;
}

export interface PlatformDiscoveryItem {
  id: string;
  name: string;
  category: string;
  status: string;
  primaryPath: string | null;
  detectedPaths: string[];
  signals: string[];
  notes: string[];
}

export interface PlatformDiscoveryResult {
  generatedAt: number;
  totals: PlatformDiscoveryTotals;
  items: PlatformDiscoveryItem[];
}

export interface AccountUsage {
  accountId: string;
  availabilityStatus: string;
  usedPercent: number | null;
  windowMinutes: number | null;
  resetsAt: number | null;
  secondaryUsedPercent: number | null;
  secondaryWindowMinutes: number | null;
  secondaryResetsAt: number | null;
  creditsJson: string | null;
  capturedAt: number | null;
}

export interface Account {
  id: string;
  name: string;
  group: string;
  priority: number;
  label: string;
  groupName: string;
  sort: number;
  status: string;
  statusReason: string;
  planType: string | null;
  planTypeRaw: string | null;
  note: string | null;
  tags: string[];
  isAvailable: boolean;
  isLowQuota: boolean;
  lastRefreshAt: number | null;
  availabilityText: string;
  availabilityLevel: AvailabilityLevel;
  primaryRemainPercent: number | null;
  secondaryRemainPercent: number | null;
  usage: AccountUsage | null;
}

export interface AccountListResult {
  items: Account[];
  total: number;
  page: number;
  pageSize: number;
}

export interface UsageAggregateSummary {
  primaryBucketCount: number;
  primaryKnownCount: number;
  primaryUnknownCount: number;
  primaryRemainPercent: number | null;
  secondaryBucketCount: number;
  secondaryKnownCount: number;
  secondaryUnknownCount: number;
  secondaryRemainPercent: number | null;
}

export interface ApiKey {
  id: string;
  name: string;
  model: string;
  modelSlug: string;
  reasoningEffort: string;
  serviceTier: string;
  rotationStrategy: string;
  aggregateApiId: string | null;
  aggregateApiUrl: string | null;
  protocol: string;
  clientType: string;
  authScheme: string;
  upstreamBaseUrl: string;
  staticHeadersJson: string;
  status: string;
  createdAt: number | null;
  lastUsedAt: number | null;
}

export interface ApiKeyCreateResult {
  id: string;
  key: string;
}

export interface AggregateApi {
  id: string;
  providerType: string;
  supplierName: string | null;
  sort: number;
  url: string;
  status: string;
  createdAt: number | null;
  updatedAt: number | null;
  lastTestAt: number | null;
  lastTestStatus: string | null;
  lastTestError: string | null;
}

export interface AggregateApiCreateResult {
  id: string;
  key: string;
}

export interface AggregateApiTestResult {
  id: string;
  ok: boolean;
  statusCode: number | null;
  message: string | null;
  testedAt: number;
  latencyMs: number;
}

export interface ApiKeyUsageStat {
  keyId: string;
  totalTokens: number;
}

export interface ModelOption {
  slug: string;
  displayName: string;
}

export interface RequestLog {
  id: string;
  traceId: string;
  keyId: string;
  accountId: string;
  initialAccountId: string;
  attemptedAccountIds: string[];
  initialAggregateApiId: string;
  attemptedAggregateApiIds: string[];
  requestPath: string;
  originalPath: string;
  adaptedPath: string;
  method: string;
  path: string;
  model: string;
  reasoningEffort: string;
  responseAdapter: string;
  upstreamUrl: string;
  aggregateApiSupplierName: string | null;
  aggregateApiUrl: string | null;
  statusCode: number | null;
  inputTokens: number | null;
  cachedInputTokens: number | null;
  outputTokens: number | null;
  totalTokens: number | null;
  reasoningOutputTokens: number | null;
  estimatedCostUsd: number | null;
  durationMs: number | null;
  error: string;
  createdAt: number | null;
}

export interface RequestLogListResult {
  items: RequestLog[];
  total: number;
  page: number;
  pageSize: number;
}

export interface RequestLogFilterSummary {
  totalCount: number;
  filteredCount: number;
  successCount: number;
  errorCount: number;
  totalTokens: number;
}

export interface LoginStatusResult {
  status: string;
  error: string;
}

export interface RequestLogTodaySummary {
  inputTokens: number;
  cachedInputTokens: number;
  outputTokens: number;
  reasoningOutputTokens: number;
  todayTokens: number;
  estimatedCost: number;
}

export interface DeviceAuthInfo {
  userCodeUrl: string;
  tokenUrl: string;
  verificationUrl: string;
  redirectUri: string;
}

export interface LoginStartResult {
  authUrl: string;
  loginId: string;
  loginType: string;
  issuer: string;
  clientId: string;
  redirectUri: string;
  warning: string;
  device: DeviceAuthInfo | null;
}

export interface CurrentAccessTokenAccount {
  type: string;
  accountId: string;
  email: string;
  planType: string;
  planTypeRaw?: string | null;
  chatgptAccountId: string | null;
  workspaceId: string | null;
  status: string;
}

export interface CurrentAccessTokenAccountReadResult {
  account: CurrentAccessTokenAccount | null;
  authMode: string | null;
  requiresOpenaiAuth: boolean;
}

export interface ChatgptAuthTokensRefreshResult {
  accountId: string;
  accessToken: string;
  chatgptAccountId: string;
  chatgptPlanType: string | null;
  chatgptPlanTypeRaw?: string | null;
}

export interface EnvOverrideCatalogItem {
  key: string;
  label: string;
  defaultValue: string;
  scope: string;
  applyMode: string;
}

export interface BackgroundTaskSettings {
  usagePollingEnabled: boolean;
  usagePollIntervalSecs: number;
  gatewayKeepaliveEnabled: boolean;
  gatewayKeepaliveIntervalSecs: number;
  tokenRefreshPollingEnabled: boolean;
  tokenRefreshPollIntervalSecs: number;
  usageRefreshWorkers: number;
  httpWorkerFactor: number;
  httpWorkerMin: number;
  httpStreamWorkerFactor: number;
  httpStreamWorkerMin: number;
}

export interface AppSettings {
  updateAutoCheck: boolean;
  closeToTrayOnClose: boolean;
  closeToTraySupported: boolean;
  lowTransparency: boolean;
  lightweightModeOnCloseToTray: boolean;
  webAccessPasswordConfigured: boolean;
  serviceAddr: string;
  serviceListenMode: string;
  serviceListenModeOptions: string[];
  routeStrategy: string;
  routeStrategyOptions: string[];
  freeAccountMaxModel: string;
  freeAccountMaxModelOptions: string[];
  requestCompressionEnabled: boolean;
  gatewayOriginator: string;
  gatewayUserAgentVersion: string;
  gatewayResidencyRequirement: string;
  gatewayResidencyRequirementOptions: string[];
  upstreamProxyUrl: string;
  upstreamStreamTimeoutMs: number;
  sseKeepaliveIntervalMs: number;
  backgroundTasks: BackgroundTaskSettings;
  envOverrides: Record<string, string>;
  envOverrideCatalog: EnvOverrideCatalogItem[];
  envOverrideReservedKeys: string[];
  envOverrideUnsupportedKeys: string[];
  theme: string;
  appearancePreset: string;
  [key: string]: unknown;
}

export interface ServiceInitializationResult {
  serverName: string;
  version: string;
  userAgent: string;
}

export interface StartupSnapshot {
  accounts: Account[];
  usageSnapshots: AccountUsage[];
  usageAggregateSummary: UsageAggregateSummary;
  apiKeys: ApiKey[];
  apiModelOptions: ModelOption[];
  manualPreferredAccountId: string;
  requestLogTodaySummary: RequestLogTodaySummary;
  requestLogs: RequestLog[];
}

// ── Session management types ──────────────────────────────────────

export interface SessionProviderSummary {
  name: string;
  count: number;
}

export interface SessionListItem {
  id: string;
  filePath: string;
  relativePath: string;
  provider: string;
  source: string;
  timestamp: string | null;
  timestampDisplay: string;
  cwd: string | null;
  originator: string | null;
  cliVersion: string | null;
  preview: string | null;
  recentPrompts: string[];
  size: number;
  sizeDisplay: string;
  archived: boolean;
}

export interface SessionListResult {
  items: SessionListItem[];
  total: number;
  page: number;
  pageSize: number;
  sessionsDir: string;
  providers: SessionProviderSummary[];
  totals: { all: number; filtered: number };
}

export interface SessionOverviewResult {
  sessionsDir: string;
  totals: { sessions: number; providers: number; backups: number; bytes: number; bytesDisplay: string };
  providers: SessionProviderSummary[];
  latestSessionAt: string | null;
  latestSessionAtDisplay: string;
}

export interface SessionDashboardResult {
  overview: SessionOverviewResult;
  sessions: SessionListResult;
}

export interface SessionDetailResult {
  id: string;
  filePath: string;
  relativePath: string;
  provider: string;
  source: string;
  timestamp: string | null;
  timestampDisplay: string;
  cwd: string | null;
  originator: string | null;
  cliVersion: string | null;
  size: number;
  sizeDisplay: string;
  preview: string | null;
  recentPrompts: string[];
  latestCwd: string | null;
  latestModel: string | null;
  archived: boolean;
}

export interface SessionDoctorIssue {
  severity: string;
  issueType: string;
  relativePath: string | null;
  message: string;
}

export interface SessionDoctorSummary {
  totalFiles: number;
  invalidMetaCount: number;
  missingProviderCount: number;
  missingWorkspaceCount: number;
  workspaceReadyCount: number;
  duplicateIdCount: number;
}

export interface SessionDoctorResult {
  ok: boolean;
  sessionsDir: string;
  summary: SessionDoctorSummary;
  issues: SessionDoctorIssue[];
}

export interface SessionMigrationPreviewItem {
  id: string;
  filePath: string;
  relativePath: string;
  timestamp: string | null;
  timestampDisplay: string;
  cwd: string | null;
  preview: string | null;
  from: string;
  fromSource: string;
  to: string;
  toSource: string;
  skipped: boolean;
}

export interface SessionMigrationPreviewResult {
  sessionsDir: string;
  targetProvider: string;
  targetSource: string;
  totalSelected: number;
  actionable: number;
  skipped: number;
  items: SessionMigrationPreviewItem[];
}

export interface SessionActionError {
  filePath: string;
  message: string;
}

export interface SessionMigrationResult {
  ok: boolean;
  dryRun: boolean;
  sessionsDir: string;
  targetProvider: string;
  targetSource: string;
  backupId: string | null;
  backupDir: string | null;
  totalSelected: number;
  migrated: number;
  skipped: number;
  errors: SessionActionError[];
  items: SessionMigrationPreviewItem[];
}

export interface SessionExportResult {
  ok: boolean;
  sessionsDir: string;
  format: string;
  fileName: string;
  filePath: string;
  mimeType: string;
  content: string;
  sessionCount: number;
  exportedAt: string;
}

export interface SessionRepairResult {
  ok: boolean;
  sessionsDir: string;
  sessionIndexPath: string;
  sessionIndexBackupPath: string | null;
  totalSessions: number;
  writtenEntries: number;
  stateDatabaseCount: number;
  threadsInserted: number;
  threadsUpdated: number;
  issues: SessionDoctorIssue[];
}

export interface SessionBackupSummary {
  backupId: string;
  backupDir: string;
  createdAt: string;
  label: string;
  reason: string | null;
  sourceProvider: string | null;
  targetProvider: string | null;
  entryCount: number;
}

export interface SessionBackupListResult {
  sessionsDir: string;
  backups: SessionBackupSummary[];
}
