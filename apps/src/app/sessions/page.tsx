"use client";

import { useCallback, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Database,
  Download,
  FolderOpen,
  HardDrive,
  RefreshCw,
  Search,
  Activity,
  Shuffle,
  Stethoscope,
  Wrench,
} from "lucide-react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { sessionClient, type SessionSelectionInput } from "@/lib/api/session-client";
import { useDesktopPageActive } from "@/hooks/useDesktopPageActive";
import { useDeferredDesktopActivation } from "@/hooks/useDeferredDesktopActivation";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { useAppStore } from "@/lib/store/useAppStore";
import { cn } from "@/lib/utils";
import type {
  SessionDashboardResult,
  SessionDetailResult,
  SessionDoctorResult,
  SessionExportResult,
  SessionListItem,
  SessionMigrationPreviewResult,
  SessionMigrationResult,
  SessionProviderSummary,
  SessionRepairResult,
} from "@/types";

const PAGE_SIZE_OPTIONS = [20, 50, 100];

function SummaryCard({
  title,
  value,
  description,
  icon: Icon,
}: {
  title: string;
  value: string | number;
  description?: string;
  icon: React.ElementType;
}) {
  return (
    <Card className="glass-card border-none shadow-md backdrop-blur-md">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="text-sm font-medium">{title}</CardTitle>
        <Icon className="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value}</div>
        {description && (
          <p className="text-xs text-muted-foreground">{description}</p>
        )}
      </CardContent>
    </Card>
  );
}

function ProviderPill({
  provider,
  isSelected,
  onClick,
}: {
  provider: SessionProviderSummary;
  isSelected: boolean;
  onClick: () => void;
}) {
  return (
    <Badge
      variant={isSelected ? "default" : "secondary"}
      className={cn(
        "cursor-pointer px-3 py-1 text-sm transition-colors",
        isSelected && "bg-primary text-primary-foreground"
      )}
      onClick={onClick}
    >
      {provider.name} ({provider.count})
    </Badge>
  );
}

export default function SessionsPage() {
  const { serviceStatus } = useAppStore();
  const isServiceReady = serviceStatus.connected;
  const canActivate = useDeferredDesktopActivation(isServiceReady);
  const isPageActive = useDesktopPageActive("/sessions/");
  usePageTransitionReady("/sessions/", isServiceReady);

  // Local state.
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [searchQuery, setSearchQuery] = useState("");
  const [providerFilter, setProviderFilter] = useState<string>("all");
  const [selectedSession, setSelectedSession] = useState<SessionDetailResult | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);
  const [migrateOpen, setMigrateOpen] = useState(false);
  const [targetProvider, setTargetProvider] = useState("");
  const [targetSource, setTargetSource] = useState("vscode");
  const [migrationPreview, setMigrationPreview] = useState<SessionMigrationPreviewResult | null>(null);
  const [doctorOpen, setDoctorOpen] = useState(false);
  const [doctorResult, setDoctorResult] = useState<SessionDoctorResult | null>(null);
  const [exportFormat, setExportFormat] = useState("markdown");
  const [operationLoading, setOperationLoading] = useState<string | null>(null);

  // Dashboard query.
  const { data: dashboard, isLoading, refetch } = useQuery({
    queryKey: ["sessions", "dashboard", page, pageSize, searchQuery, providerFilter],
    queryFn: async () => {
      const result = await sessionClient.getDashboard({
        page,
        pageSize,
        query: searchQuery || undefined,
        provider: providerFilter !== "all" ? providerFilter : undefined,
        includePreview: true,
      });
      return result as SessionDashboardResult;
    },
    enabled: isServiceReady && canActivate,
    refetchInterval: 30_000,
  });

  // Session detail query.
  const { data: sessionDetail } = useQuery({
    queryKey: ["sessions", "detail", selectedSession?.filePath],
    queryFn: async () => {
      if (!selectedSession) return null;
      const result = await sessionClient.getSessionDetail({
        path: selectedSession.filePath,
      });
      return result as SessionDetailResult;
    },
    enabled: !!selectedSession && isServiceReady,
  });

  const overview = dashboard?.overview;
  const sessions = dashboard?.sessions;
  const providers = overview?.providers ?? [];

  const handleProviderClick = useCallback((name: string) => {
    setProviderFilter((prev) => (prev === name ? "all" : name));
    setPage(1);
  }, []);

  const handleSearch = useCallback((value: string) => {
    setSearchQuery(value);
    setPage(1);
  }, []);

  const handleRowClick = useCallback((item: SessionListItem) => {
    setSelectedSession(item as unknown as SessionDetailResult);
    setDetailOpen(true);
  }, []);

  const totalPages = sessions
    ? Math.ceil(sessions.total / pageSize)
    : 0;

  const buildCurrentSelection = useCallback((): SessionSelectionInput => {
    const selection: SessionSelectionInput = {};
    if (providerFilter !== "all") {
      selection.provider = providerFilter;
    }
    if (searchQuery.trim()) {
      selection.query = searchQuery.trim();
    }
    if (!selection.provider && !selection.query) {
      selection.allowAll = true;
    }
    return selection;
  }, [providerFilter, searchQuery]);

  const selectionLabel = useMemo(() => {
    const parts = [];
    if (providerFilter !== "all") {
      parts.push(`Provider: ${providerFilter}`);
    }
    if (searchQuery.trim()) {
      parts.push(`搜索: ${searchQuery.trim()}`);
    }
    return parts.length ? parts.join(" · ") : "全部会话";
  }, [providerFilter, searchQuery]);

  const downloadExport = useCallback((result: SessionExportResult) => {
    if (typeof window === "undefined") {
      return;
    }
    const blob = new Blob([result.content], { type: result.mimeType });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = result.fileName;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }, []);

  const handlePreviewMigration = useCallback(async () => {
    if (!targetProvider.trim()) {
      toast.error("请填写目标 Provider");
      return;
    }
    setOperationLoading("migrate-preview");
    try {
      const result = await sessionClient.previewMigration({
        selection: buildCurrentSelection(),
        targetProvider: targetProvider.trim(),
        targetSource: targetSource.trim() || "vscode",
      });
      setMigrationPreview(result as SessionMigrationPreviewResult);
    } catch (error) {
      toast.error(`迁移预览失败: ${String(error)}`);
    } finally {
      setOperationLoading(null);
    }
  }, [buildCurrentSelection, targetProvider, targetSource]);

  const handleRunMigration = useCallback(async () => {
    if (!targetProvider.trim()) {
      toast.error("请填写目标 Provider");
      return;
    }
    setOperationLoading("migrate");
    try {
      const result = await sessionClient.migrateSessions({
        selection: buildCurrentSelection(),
        targetProvider: targetProvider.trim(),
        targetSource: targetSource.trim() || "vscode",
      }) as SessionMigrationResult;
      if (result.ok) {
        toast.success(`已迁移 ${result.migrated} 个会话，跳过 ${result.skipped} 个`);
      } else {
        toast.error(`迁移完成但有 ${result.errors.length} 个错误`);
      }
      setMigrateOpen(false);
      setMigrationPreview(null);
      void refetch();
    } catch (error) {
      toast.error(`迁移失败: ${String(error)}`);
    } finally {
      setOperationLoading(null);
    }
  }, [buildCurrentSelection, refetch, targetProvider, targetSource]);

  const handleExport = useCallback(async () => {
    setOperationLoading("export");
    try {
      const result = await sessionClient.exportSessions({
        selection: buildCurrentSelection(),
        format: exportFormat,
        filePrefix: "codex-copilot-session-export",
      }) as SessionExportResult;
      downloadExport(result);
      toast.success(`已导出 ${result.sessionCount} 个会话`);
    } catch (error) {
      toast.error(`导出失败: ${String(error)}`);
    } finally {
      setOperationLoading(null);
    }
  }, [buildCurrentSelection, downloadExport, exportFormat]);

  const handleDoctor = useCallback(async () => {
    setOperationLoading("doctor");
    try {
      const result = await sessionClient.runDoctor() as SessionDoctorResult;
      setDoctorResult(result);
      setDoctorOpen(true);
    } catch (error) {
      toast.error(`诊断失败: ${String(error)}`);
    } finally {
      setOperationLoading(null);
    }
  }, []);

  const handleRepair = useCallback(async () => {
    setOperationLoading("repair");
    try {
      const result = await sessionClient.repairIndex() as SessionRepairResult;
      toast.success(
        `已修复索引：session_index ${result.writtenEntries}/${result.totalSessions}，threads +${result.threadsInserted}/${result.threadsUpdated}`
      );
      void refetch();
    } catch (error) {
      toast.error(`修复失败: ${String(error)}`);
    } finally {
      setOperationLoading(null);
    }
  }, [refetch]);

  if (isLoading) {
    return (
      <div className="space-y-6 p-6">
        <div className="grid gap-4 md:grid-cols-4">
          {[1, 2, 3, 4].map((i) => (
            <Skeleton key={i} className="h-28 rounded-xl" />
          ))}
        </div>
        <Skeleton className="h-10 rounded-lg" />
        <Skeleton className="h-96 rounded-lg" />
      </div>
    );
  }

  return (
    <div className="space-y-6 p-6">
      {/* Summary cards. */}
      <div className="grid gap-4 md:grid-cols-4">
        <SummaryCard
          title="会话总数"
          value={overview?.totals.sessions ?? 0}
          description={overview?.totals.bytesDisplay}
          icon={FolderOpen}
        />
        <SummaryCard
          title="Provider 数"
          value={overview?.totals.providers ?? 0}
          icon={Database}
        />
        <SummaryCard
          title="备份数"
          value={overview?.totals.backups ?? 0}
          icon={HardDrive}
        />
        <SummaryCard
          title="最近活动"
          value={overview?.latestSessionAtDisplay ?? "-"}
          icon={Activity}
        />
      </div>

      {/* Provider filter pills. */}
      {providers.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {providers.map((p) => (
            <ProviderPill
              key={p.name}
              provider={p}
              isSelected={providerFilter === p.name}
              onClick={() => handleProviderClick(p.name)}
            />
          ))}
        </div>
      )}

      {/* Search + controls. */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            placeholder="搜索会话 ID、Provider、工作区..."
            value={searchQuery}
            onChange={(e) => handleSearch(e.target.value)}
            className="pl-9"
          />
        </div>
        <Select
          value={String(pageSize)}
          onValueChange={(v) => {
            setPageSize(Number(v));
            setPage(1);
          }}
        >
          <SelectTrigger className="w-24">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {PAGE_SIZE_OPTIONS.map((s) => (
              <SelectItem key={s} value={String(s)}>
                {s} 条/页
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <Select
          value={exportFormat}
          onValueChange={(value) => {
            if (value) {
              setExportFormat(value);
            }
          }}
        >
          <SelectTrigger className="w-28">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="markdown">Markdown</SelectItem>
            <SelectItem value="html">HTML</SelectItem>
            <SelectItem value="json">JSON</SelectItem>
            <SelectItem value="jsonl">JSONL</SelectItem>
            <SelectItem value="csv">CSV</SelectItem>
            <SelectItem value="txt">TXT</SelectItem>
          </SelectContent>
        </Select>
        <Button
          variant="outline"
          size="sm"
          className="gap-2"
          disabled={!isServiceReady || operationLoading === "export"}
          onClick={() => void handleExport()}
        >
          <Download className="h-4 w-4" />
          导出
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="gap-2"
          disabled={!isServiceReady}
          onClick={() => {
            setMigrationPreview(null);
            setMigrateOpen(true);
          }}
        >
          <Shuffle className="h-4 w-4" />
          迁移
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="gap-2"
          disabled={!isServiceReady || operationLoading === "doctor"}
          onClick={() => void handleDoctor()}
        >
          <Stethoscope className="h-4 w-4" />
          诊断
        </Button>
        <Button
          variant="outline"
          size="sm"
          className="gap-2"
          disabled={!isServiceReady || operationLoading === "repair"}
          onClick={() => void handleRepair()}
        >
          <Wrench className="h-4 w-4" />
          修复索引
        </Button>
        <Button
          variant="outline"
          size="icon"
          onClick={() => refetch()}
          title="刷新"
        >
          <RefreshCw className="h-4 w-4" />
        </Button>
      </div>

      {/* Results info. */}
      {sessions && (
        <div className="text-sm text-muted-foreground">
          共 {sessions.totals.all} 个会话
          {sessions.totals.filtered !== sessions.totals.all &&
            `，已筛选 ${sessions.totals.filtered} 个`}
          <span className="ml-2">操作范围：{selectionLabel}</span>
        </div>
      )}

      {/* Session table. */}
      <div className="glass-card border-none shadow-md backdrop-blur-md rounded-xl overflow-hidden">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-40">时间</TableHead>
              <TableHead className="w-28">Provider</TableHead>
              <TableHead className="w-24">来源</TableHead>
              <TableHead>工作区</TableHead>
              <TableHead>预览</TableHead>
              <TableHead className="w-20">大小</TableHead>
              <TableHead className="w-16">状态</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {sessions?.items.length === 0 && (
              <TableRow>
                <TableCell colSpan={7} className="text-center text-muted-foreground py-8">
                  没有找到会话
                </TableCell>
              </TableRow>
            )}
            {sessions?.items.map((item) => (
              <TableRow
                key={item.id + item.filePath}
                className="cursor-pointer hover:bg-muted/50"
                onClick={() => handleRowClick(item)}
              >
                <TableCell className="text-xs text-muted-foreground">
                  {item.timestampDisplay || "-"}
                </TableCell>
                <TableCell>
                  <Badge variant="secondary" className="text-xs">
                    {item.provider || "unknown"}
                  </Badge>
                </TableCell>
                <TableCell className="text-xs">{item.source}</TableCell>
                <TableCell
                  className="max-w-48 truncate text-xs"
                  title={item.cwd ?? undefined}
                >
                  {item.cwd || "-"}
                </TableCell>
                <TableCell className="max-w-64 truncate text-xs">
                  {item.preview || "-"}
                </TableCell>
                <TableCell className="text-xs text-muted-foreground">
                  {item.sizeDisplay}
                </TableCell>
                <TableCell>
                  {item.archived ? (
                    <Badge variant="outline" className="text-xs">
                      已归档
                    </Badge>
                  ) : (
                    <Badge className="border-green-500/20 bg-green-500/10 text-green-500 text-xs">
                      活跃
                    </Badge>
                  )}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>

      {/* Pagination. */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between">
          <div className="text-sm text-muted-foreground">
            第 {page} / {totalPages} 页
          </div>
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={page <= 1}
              onClick={() => setPage((p) => Math.max(1, p - 1))}
            >
              上一页
            </Button>
            <Button
              variant="outline"
              size="sm"
              disabled={page >= totalPages}
              onClick={() => setPage((p) => p + 1)}
            >
              下一页
            </Button>
          </div>
        </div>
      )}

      {/* Session detail dialog. */}
      <Dialog
        open={isPageActive && detailOpen}
        onOpenChange={(open) => {
          if (!open) {
            setDetailOpen(false);
            setSelectedSession(null);
          }
        }}
      >
        <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>会话详情</DialogTitle>
          </DialogHeader>
          {sessionDetail && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 gap-3 text-sm">
                <div>
                  <span className="text-muted-foreground">ID:</span>{" "}
                  <code className="text-xs break-all">{sessionDetail.id}</code>
                </div>
                <div>
                  <span className="text-muted-foreground">Provider:</span>{" "}
                  <Badge variant="secondary">{sessionDetail.provider}</Badge>
                </div>
                <div>
                  <span className="text-muted-foreground">来源:</span>{" "}
                  {sessionDetail.source}
                </div>
                <div>
                  <span className="text-muted-foreground">时间:</span>{" "}
                  {sessionDetail.timestampDisplay}
                </div>
                <div className="col-span-2">
                  <span className="text-muted-foreground">工作区:</span>{" "}
                  <code className="text-xs break-all">{sessionDetail.cwd || sessionDetail.latestCwd || "-"}</code>
                </div>
                {sessionDetail.latestModel && (
                  <div>
                    <span className="text-muted-foreground">模型:</span>{" "}
                    {sessionDetail.latestModel}
                  </div>
                )}
                <div>
                  <span className="text-muted-foreground">大小:</span>{" "}
                  {sessionDetail.sizeDisplay}
                </div>
              </div>

              {/* Recent prompts. */}
              {sessionDetail.recentPrompts.length > 0 && (
                <div>
                  <h4 className="text-sm font-medium mb-2">最近 Prompt</h4>
                  <div className="space-y-2">
                    {sessionDetail.recentPrompts.map((prompt, i) => (
                      <div
                        key={i}
                        className="rounded-lg bg-muted/50 p-3 text-xs whitespace-pre-wrap break-all"
                      >
                        {prompt}
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* File path. */}
              <div>
                <span className="text-muted-foreground text-sm">文件路径:</span>{" "}
                <code className="text-xs break-all text-muted-foreground">
                  {sessionDetail.relativePath}
                </code>
              </div>
            </div>
          )}
        </DialogContent>
      </Dialog>

      <Dialog open={migrateOpen} onOpenChange={setMigrateOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>迁移会话 Provider</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="rounded-lg border bg-muted/40 p-3 text-xs text-muted-foreground">
              当前操作范围：{selectionLabel}
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="space-y-2">
                <div className="text-xs font-medium text-muted-foreground">目标 Provider</div>
                <Input
                  placeholder="例如 openai / anthropic / copilot"
                  value={targetProvider}
                  onChange={(event) => setTargetProvider(event.target.value)}
                />
              </div>
              <div className="space-y-2">
                <div className="text-xs font-medium text-muted-foreground">可见性来源</div>
                <Input
                  placeholder="vscode"
                  value={targetSource}
                  onChange={(event) => setTargetSource(event.target.value)}
                />
              </div>
            </div>
            {migrationPreview && (
              <div className="rounded-lg border">
                <div className="border-b px-3 py-2 text-sm">
                  预览：选中 {migrationPreview.totalSelected} 个，可迁移{" "}
                  {migrationPreview.actionable} 个，跳过 {migrationPreview.skipped} 个
                </div>
                <div className="max-h-56 overflow-y-auto p-2">
                  {migrationPreview.items.slice(0, 20).map((item) => (
                    <div
                      key={item.filePath}
                      className="flex items-center justify-between gap-3 rounded-md px-2 py-1.5 text-xs"
                    >
                      <span className="min-w-0 flex-1 truncate" title={item.relativePath}>
                        {item.relativePath}
                      </span>
                      <span className="shrink-0 text-muted-foreground">
                        {item.from || "unknown"} → {item.to}
                      </span>
                      <Badge variant={item.skipped ? "secondary" : "default"} className="shrink-0">
                        {item.skipped ? "跳过" : "迁移"}
                      </Badge>
                    </div>
                  ))}
                  {migrationPreview.items.length > 20 && (
                    <div className="px-2 py-1 text-xs text-muted-foreground">
                      仅显示前 20 条
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setMigrateOpen(false)}>
              取消
            </Button>
            <Button
              variant="outline"
              disabled={operationLoading === "migrate-preview"}
              onClick={() => void handlePreviewMigration()}
            >
              预览
            </Button>
            <Button
              disabled={operationLoading === "migrate" || !targetProvider.trim()}
              onClick={() => void handleRunMigration()}
            >
              执行迁移
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={doctorOpen} onOpenChange={setDoctorOpen}>
        <DialogContent className="max-w-2xl max-h-[80vh] overflow-y-auto">
          <DialogHeader>
            <DialogTitle>会话健康诊断</DialogTitle>
          </DialogHeader>
          {doctorResult && (
            <div className="space-y-4">
              <div className="grid gap-3 sm:grid-cols-3">
                <SummaryCard
                  title="文件数"
                  value={doctorResult.summary.totalFiles}
                  icon={FolderOpen}
                />
                <SummaryCard
                  title="异常元数据"
                  value={doctorResult.summary.invalidMetaCount}
                  icon={Database}
                />
                <SummaryCard
                  title="缺失工作区"
                  value={doctorResult.summary.missingWorkspaceCount}
                  icon={HardDrive}
                />
              </div>
              <div className="space-y-2">
                {doctorResult.issues.length === 0 ? (
                  <div className="rounded-lg border bg-muted/40 p-3 text-sm text-muted-foreground">
                    未发现阻断性问题
                  </div>
                ) : (
                  doctorResult.issues.slice(0, 50).map((issue, index) => (
                    <div key={index} className="rounded-lg border p-3 text-xs">
                      <div className="mb-1 flex items-center gap-2">
                        <Badge variant={issue.severity === "error" ? "destructive" : "secondary"}>
                          {issue.severity}
                        </Badge>
                        <span className="font-medium">{issue.issueType}</span>
                      </div>
                      <div className="text-muted-foreground">{issue.message}</div>
                      {issue.relativePath && (
                        <code className="mt-1 block break-all text-muted-foreground">
                          {issue.relativePath}
                        </code>
                      )}
                    </div>
                  ))
                )}
                {doctorResult.issues.length > 50 && (
                  <div className="text-xs text-muted-foreground">仅显示前 50 条问题</div>
                )}
              </div>
            </div>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={() => setDoctorOpen(false)}>
              关闭
            </Button>
            <Button onClick={() => void handleRepair()} disabled={operationLoading === "repair"}>
              修复 session_index
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
