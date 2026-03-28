"use client";

import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Activity,
  AlertTriangle,
  ArrowUp,
  CheckCircle2,
  Copy,
  Database,
  DollarSign,
  Eye,
  EyeOff,
  MoreVertical,
  Plus,
  RefreshCw,
  Settings2,
  ShieldCheck,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";
import { AggregateApiModal } from "@/components/modals/aggregate-api-modal";
import { ConfirmDialog } from "@/components/modals/confirm-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
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
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { accountClient } from "@/lib/api/account-client";
import { copyTextToClipboard } from "@/lib/utils/clipboard";
import { formatCompactNumber, formatTsFromSeconds } from "@/lib/utils/usage";
import { useAppStore } from "@/lib/store/useAppStore";
import { useAggregateApiStats } from "@/hooks/useAggregateApiStats";
import { useDesktopPageActive } from "@/hooks/useDesktopPageActive";
import { useDeferredDesktopActivation } from "@/hooks/useDeferredDesktopActivation";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { useRuntimeCapabilities } from "@/hooks/useRuntimeCapabilities";
import { AggregateApi } from "@/types";

const AGGREGATE_API_PROVIDER_LABELS: Record<string, string> = {
  codex: "Codex",
  claude: "Claude",
};

const AGGREGATE_API_PROVIDER_FILTER_LABELS: Record<string, string> = {
  all: "全部类型",
  codex: "Codex",
  claude: "Claude",
};

function getTestBadge(api: AggregateApi) {
  if (api.lastTestStatus === "success") {
    return (
      <Badge className="border-green-500/20 bg-green-500/10 text-green-500">
        已连通
      </Badge>
    );
  }
  if (api.lastTestStatus === "failed") {
    return (
      <Badge className="border-red-500/20 bg-red-500/10 text-red-500">
        失败
      </Badge>
    );
  }
  return <Badge variant="secondary">未测试</Badge>;
}

function AggregateMetricCard({
  title,
  value,
  description,
  icon: Icon,
  toneClass,
}: {
  title: string;
  value: string;
  description: string;
  icon: LucideIcon;
  toneClass: string;
}) {
  return (
    <Card className="glass-card border-none shadow-sm backdrop-blur-md transition-all hover:-translate-y-0.5">
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-1.5">
        <CardTitle className="text-[13px] font-medium text-muted-foreground">
          {title}
        </CardTitle>
        <div
          className={`flex h-8 w-8 items-center justify-center rounded-xl ${toneClass}`}
        >
          <Icon className="h-3.5 w-3.5" />
        </div>
      </CardHeader>
      <CardContent className="space-y-0.5">
        <div className="text-[2rem] leading-none font-semibold tracking-tight">
          {value}
        </div>
        <p className="text-[11px] text-muted-foreground">{description}</p>
      </CardContent>
    </Card>
  );
}

export default function AggregateApiPage() {
  const queryClient = useQueryClient();
  const serviceStatus = useAppStore((state) => state.serviceStatus);
  const { canAccessManagementRpc } = useRuntimeCapabilities();
  const isServiceReady = canAccessManagementRpc && serviceStatus.connected;
  const isPageActive = useDesktopPageActive("/aggregate-api/");
  const isQueryEnabled = useDeferredDesktopActivation(isServiceReady);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [providerFilter, setProviderFilter] = useState("all");
  const [revealedSecrets, setRevealedSecrets] = useState<
    Record<string, string>
  >({});
  const [loadingSecretId, setLoadingSecretId] = useState<string | null>(null);
  const [testingApiId, setTestingApiId] = useState<string | null>(null);

  const { data: aggregateApis = [], isLoading } = useQuery({
    queryKey: ["aggregate-apis"],
    queryFn: () => accountClient.listAggregateApis(),
    enabled: isQueryEnabled,
    retry: 1,
  });

  const { stats: aggregateStats, isLoading: isStatsLoading } =
    useAggregateApiStats({
      aggregateApis,
      enabled: isQueryEnabled && isServiceReady,
      active: isPageActive,
    });

  usePageTransitionReady(
    "/aggregate-api/",
    !isServiceReady || (!isLoading && !isStatsLoading),
  );

  useEffect(() => {
    if (isPageActive) return;
    setModalOpen(false);
    setEditingId(null);
    setDeleteId(null);
  }, [isPageActive]);

  const editingApi = useMemo(
    () => aggregateApis.find((item) => item.id === editingId) || null,
    [aggregateApis, editingId],
  );

  const filteredAggregateApis = useMemo(() => {
    if (providerFilter === "all") {
      return aggregateApis;
    }
    return aggregateApis.filter((api) => api.providerType === providerFilter);
  }, [aggregateApis, providerFilter]);

  const defaultCreateSort = useMemo(() => {
    const maxSort = aggregateApis.reduce(
      (max, api) => Math.max(max, Number(api.sort) || 0),
      0,
    );
    return maxSort + 5;
  }, [aggregateApis]);

  const renderTestStatus = (api: AggregateApi) => {
    const badge = getTestBadge(api);
    if (api.lastTestStatus !== "failed" || !api.lastTestError) {
      return badge;
    }

    return (
      <Tooltip>
        <TooltipTrigger render={<div />} className="inline-flex cursor-help">
          {badge}
        </TooltipTrigger>
        <TooltipContent className="max-w-sm whitespace-pre-wrap break-words">
          {api.lastTestError}
        </TooltipContent>
      </Tooltip>
    );
  };

  const testMutation = useMutation({
    mutationFn: (apiId: string) =>
      accountClient.testAggregateApiConnection(apiId),
    onMutate: async (apiId) => {
      setTestingApiId(apiId);
    },
    onSuccess: async (result) => {
      toast.success(
        result.ok
          ? "连通性测试成功"
          : `连通性测试失败: ${result.message || result.statusCode || ""}`,
      );
    },
    onSettled: async (_result, _error, apiId) => {
      await queryClient.invalidateQueries({ queryKey: ["aggregate-apis"] });
      setTestingApiId((current) => (current === apiId ? null : current));
    },
    onError: (error: unknown) => {
      toast.error(
        `测试失败: ${error instanceof Error ? error.message : String(error)}`,
      );
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (apiId: string) => accountClient.deleteAggregateApi(apiId),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["aggregate-apis"] });
      await queryClient.invalidateQueries({ queryKey: ["apikeys"] });
      await queryClient.invalidateQueries({ queryKey: ["startup-snapshot"] });
      toast.success("聚合 API 已删除");
    },
    onError: (error: unknown) => {
      toast.error(
        `删除失败: ${error instanceof Error ? error.message : String(error)}`,
      );
    },
  });

  const prioritizeMutation = useMutation({
    mutationFn: async (api: AggregateApi) => {
      const currentMinSort = aggregateApis.reduce(
        (min, item) => Math.min(min, Number(item.sort) || 0),
        Number(api.sort) || 0,
      );
      const nextSort =
        (Number(api.sort) || 0) <= currentMinSort ? currentMinSort : currentMinSort - 5;

      if ((Number(api.sort) || 0) === nextSort) {
        return false;
      }

      await accountClient.updateAggregateApi(api.id, {
        providerType: api.providerType,
        supplierName: api.supplierName || "",
        sort: nextSort,
        url: api.url,
        key: null,
      });
      return true;
    },
    onSuccess: async (changed) => {
      if (!changed) {
        toast.info("当前聚合 API 已是优先渠道");
        return;
      }
      await queryClient.invalidateQueries({ queryKey: ["aggregate-apis"] });
      toast.success("已设为优先渠道");
    },
    onError: (error: unknown) => {
      toast.error(
        `设置优先失败: ${error instanceof Error ? error.message : String(error)}`,
      );
    },
  });

  const openCreateModal = () => {
    setEditingId(null);
    setModalOpen(true);
  };

  const openEditModal = (apiId: string) => {
    setEditingId(apiId);
    setModalOpen(true);
  };

  const ensureSecretLoaded = async (apiId: string) => {
    if (revealedSecrets[apiId]) {
      return revealedSecrets[apiId];
    }
    setLoadingSecretId(apiId);
    try {
      const secret = await accountClient.readAggregateApiSecret(apiId);
      if (!secret) {
        throw new Error("后端未返回密钥明文");
      }
      setRevealedSecrets((current) => ({ ...current, [apiId]: secret }));
      return secret;
    } finally {
      setLoadingSecretId(null);
    }
  };

  const toggleSecret = async (apiId: string) => {
    if (revealedSecrets[apiId]) {
      setRevealedSecrets((current) => {
        const next = { ...current };
        delete next[apiId];
        return next;
      });
      return;
    }
    try {
      await ensureSecretLoaded(apiId);
    } catch (error: unknown) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  const copySecret = async (apiId: string) => {
    try {
      const secret = await ensureSecretLoaded(apiId);
      await copyTextToClipboard(secret);
      toast.success("已复制到剪贴板");
    } catch (error: unknown) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in duration-500">
      {!isServiceReady ? (
        <Card className="glass-card border-none shadow-sm">
          <CardContent className="pt-6 text-sm text-muted-foreground">
            服务未连接，聚合 API 暂不可用；连接恢复后会自动继续加载。
          </CardContent>
        </Card>
      ) : null}

      <div>
        <div>
          <p className="mt-1 text-sm text-muted-foreground">
            管理上游聚合地址与密钥，并测试连通性
          </p>
        </div>
      </div>

      <div className="space-y-3">
        <Card className="glass-card border-none shadow-md backdrop-blur-md">
          <CardContent className="pt-4">
            <div className="flex flex-col gap-1">
              <p className="text-sm font-medium">聚合 API 总览</p>
              <p className="text-xs text-muted-foreground">
                仅统计通过聚合 API 转发的请求，不包含官方账号池直连请求。
              </p>
            </div>
          </CardContent>
        </Card>

        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {isStatsLoading ? (
            Array.from({ length: 8 }).map((_, index) => (
              <Skeleton key={index} className="h-32 w-full rounded-3xl" />
            ))
          ) : (
            <>
              <AggregateMetricCard
                title="聚合通道"
                value={`${aggregateStats.totalApis}`}
                description={`已连通 ${aggregateStats.connectedApis} / 失败 ${aggregateStats.failedApis}`}
                icon={Database}
                toneClass="bg-primary/12 text-primary"
              />
              <AggregateMetricCard
                title="历史请求"
                value={`${aggregateStats.totalRequests}`}
                description="全部聚合 API 请求累计"
                icon={Activity}
                toneClass="bg-blue-500/12 text-blue-500"
              />
              <AggregateMetricCard
                title="成功请求"
                value={`${aggregateStats.successRequests}`}
                description="状态码 2xx"
                icon={CheckCircle2}
                toneClass="bg-green-500/12 text-green-500"
              />
              <AggregateMetricCard
                title="异常请求"
                value={`${aggregateStats.failedRequests}`}
                description="4xx / 5xx 或显式错误"
                icon={AlertTriangle}
                toneClass="bg-red-500/12 text-red-500"
              />
              <AggregateMetricCard
                title="累计令牌"
                value={formatCompactNumber(aggregateStats.totalTokens, "0")}
                description="聚合 API 历史 total tokens"
                icon={ShieldCheck}
                toneClass="bg-amber-500/12 text-amber-500"
              />
              <AggregateMetricCard
                title="今日令牌"
                value={formatCompactNumber(aggregateStats.todayTokens, "0")}
                description="今日输入 + 输出合计"
                icon={RefreshCw}
                toneClass="bg-cyan-500/12 text-cyan-500"
              />
              <AggregateMetricCard
                title="缓存令牌"
                value={formatCompactNumber(aggregateStats.cachedTokens, "0")}
                description="今日缓存命中"
                icon={Copy}
                toneClass="bg-indigo-500/12 text-indigo-500"
              />
              <AggregateMetricCard
                title="今日费用"
                value={`$${Number(aggregateStats.todayCost || 0).toFixed(2)}`}
                description="按请求日志估算"
                icon={DollarSign}
                toneClass="bg-emerald-500/12 text-emerald-500"
              />
            </>
          )}
        </div>
      </div>

      <div className="space-y-4">
        <Card className="glass-card border-none shadow-xl backdrop-blur-md">
          <CardContent className="px-4 ">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <span className="text-sm text-muted-foreground">查询</span>
                <Select
                  value={providerFilter}
                  onValueChange={(value) => setProviderFilter(value || "all")}
                >
                  <SelectTrigger className="w-[160px]">
                    <SelectValue>
                      {(value) =>
                        AGGREGATE_API_PROVIDER_FILTER_LABELS[
                          String(value || "")
                        ] || "全部类型"
                      }
                    </SelectValue>
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="all">全部类型</SelectItem>
                    <SelectItem value="codex">Codex</SelectItem>
                    <SelectItem value="claude">Claude</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              <div className="flex items-center gap-3">
                <div className="text-xs text-muted-foreground">
                  共 {filteredAggregateApis.length} 条
                </div>
                <Button
                  className="h-10 gap-2 shadow-lg shadow-primary/20"
                  onClick={openCreateModal}
                  disabled={!isServiceReady}
                >
                  <Plus className="h-4 w-4" /> 新建聚合 API
                </Button>
              </div>
            </div>
          </CardContent>
        </Card>

        <Card className="glass-card overflow-hidden border-none py-0 shadow-xl backdrop-blur-md">
          <CardContent className="p-0">
            <Table className="w-full table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead className="max-w-[220px]">供应商 / URL</TableHead>
                  <TableHead className="w-[84px] text-center">类型</TableHead>
                  <TableHead className="w-[148px]">密钥</TableHead>
                  <TableHead className="w-[64px] text-center">顺序</TableHead>
                  <TableHead className="w-[130px]">测试连通性</TableHead>
                  <TableHead className="text-center">操作</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {isLoading ? (
                  Array.from({ length: 3 }).map((_, index) => (
                    <TableRow key={index}>
                      <TableCell>
                        <Skeleton className="h-4 w-24" />
                      </TableCell>
                      <TableCell>
                        <Skeleton className="h-6 w-12 rounded-full" />
                      </TableCell>
                      <TableCell>
                        <Skeleton className="h-4 w-28" />
                      </TableCell>
                      <TableCell>
                        <Skeleton className="mx-auto h-4 w-12" />
                      </TableCell>
                      <TableCell>
                        <Skeleton className="h-6 w-20 rounded-full" />
                      </TableCell>
                      <TableCell className="text-center">
                        <Skeleton className="mx-auto h-8 w-8" />
                      </TableCell>
                    </TableRow>
                  ))
                ) : filteredAggregateApis.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} className="h-48 text-center">
                      <div className="flex flex-col items-center justify-center gap-2 text-muted-foreground">
                        <ShieldCheck className="h-8 w-8 opacity-20" />
                        <p>
                          {providerFilter === "all"
                            ? "暂无聚合 API，点击右上角新建"
                            : `暂无 ${AGGREGATE_API_PROVIDER_LABELS[providerFilter] || providerFilter} 聚合 API`}
                        </p>
                      </div>
                    </TableCell>
                  </TableRow>
                ) : (
                  filteredAggregateApis.map((api) => {
                    const revealed = revealedSecrets[api.id];
                    const createdTimeText = formatTsFromSeconds(
                      api.createdAt,
                      "未知时间",
                    );

                    return (
                      <TableRow key={api.id} className="group">
                        <TableCell className="overflow-hidden">
                          <Tooltip>
                            <TooltipTrigger
                              render={<div />}
                              className="block cursor-help text-left"
                            >
                              <div className="grid gap-0.5 overflow-hidden">
                                <span className="block truncate text-xs font-medium text-foreground">
                                  {api.supplierName || "-"}
                                </span>
                                <span className="block truncate font-mono text-[11px] text-muted-foreground">
                                  {api.url}
                                </span>
                              </div>
                            </TooltipTrigger>
                            <TooltipContent className="max-w-sm whitespace-pre-wrap break-words">
                              <div className="grid gap-1">
                                <div className="text-[11px] font-medium">
                                  {api.supplierName || "-"}
                                </div>
                                <div className="break-all text-xs">
                                  {api.url}
                                </div>
                                <div className="text-[11px] opacity-80">
                                  创建时间: {createdTimeText}
                                </div>
                              </div>
                            </TooltipContent>
                          </Tooltip>
                        </TableCell>
                        <TableCell className="text-center">
                          <div className="flex justify-center">
                            <Badge
                              variant="secondary"
                              className="w-fit text-[10px] font-normal"
                            >
                              {AGGREGATE_API_PROVIDER_LABELS[
                                api.providerType
                              ] || api.providerType}
                            </Badge>
                          </div>
                        </TableCell>
                        <TableCell className="overflow-hidden">
                          <div className="flex min-w-0 items-center gap-1.5 overflow-hidden">
                            <Tooltip>
                              <TooltipTrigger
                                render={<div />}
                                className="block min-w-0 cursor-help"
                              >
                                <code className="block min-w-0 flex-1 truncate rounded border border-primary/5 bg-muted/50 px-2 py-1 font-mono text-[10px] leading-4 text-primary">
                                  {revealed
                                    ? revealed
                                    : loadingSecretId === api.id
                                      ? "读取中..."
                                      : api.id}
                                </code>
                              </TooltipTrigger>
                              <TooltipContent className="max-w-sm whitespace-pre-wrap break-words">
                                {revealed || api.id}
                              </TooltipContent>
                            </Tooltip>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-7 w-7 text-muted-foreground hover:text-primary"
                              disabled={!isServiceReady}
                              onClick={() => void toggleSecret(api.id)}
                            >
                              {revealed ? (
                                <EyeOff className="h-3.5 w-3.5" />
                              ) : (
                                <Eye className="h-3.5 w-3.5" />
                              )}
                            </Button>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-7 w-7 text-muted-foreground hover:text-primary"
                              disabled={!isServiceReady}
                              onClick={() => void copySecret(api.id)}
                            >
                              <Copy className="h-3.5 w-3.5" />
                            </Button>
                          </div>
                        </TableCell>
                        <TableCell className="text-center font-mono text-xs text-muted-foreground">
                          {api.sort}
                        </TableCell>
                        <TableCell className="whitespace-nowrap align-middle">
                          <div className="flex flex-col items-start gap-1">
                            <div className="flex items-center gap-2">
                              {renderTestStatus(api)}
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-7 gap-1.5 px-2 text-xs"
                                disabled={
                                  !isServiceReady || testingApiId === api.id
                                }
                                onClick={() => testMutation.mutate(api.id)}
                              >
                                <RefreshCw
                                  className={
                                    testingApiId === api.id
                                      ? "h-3.5 w-3.5 animate-spin"
                                      : "h-3.5 w-3.5"
                                  }
                                />
                                测试
                              </Button>
                            </div>
                          </div>
                          {api.lastTestAt ? (
                            <p className="mt-1 text-[10px] text-muted-foreground">
                              {formatTsFromSeconds(api.lastTestAt, "未知时间")}
                            </p>
                          ) : null}
                        </TableCell>
                        <TableCell>
                          <div className="table-action-cell gap-1">
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-muted-foreground transition-colors hover:text-primary"
                              disabled={!isServiceReady}
                              onClick={() => openEditModal(api.id)}
                              title="编辑配置"
                            >
                              <Settings2 className="h-4 w-4" />
                            </Button>
                            <DropdownMenu>
                              <DropdownMenuTrigger>
                                <Button
                                  variant="ghost"
                                  size="icon"
                                  className="h-8 w-8"
                                  render={<span />}
                                  nativeButton={false}
                                  disabled={!isServiceReady}
                                >
                                  <MoreVertical className="h-4 w-4" />
                                </Button>
                              </DropdownMenuTrigger>
                              <DropdownMenuContent align="end">
                                <DropdownMenuItem
                                  className="gap-2"
                                  disabled={!isServiceReady}
                                  onClick={() => openEditModal(api.id)}
                                >
                                  编辑聚合 API
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  className="gap-2"
                                  disabled={
                                    !isServiceReady || prioritizeMutation.isPending
                                  }
                                  onClick={() => prioritizeMutation.mutate(api)}
                                >
                                  <ArrowUp className="h-4 w-4" /> 设为优先
                                </DropdownMenuItem>
                                <DropdownMenuItem
                                  className="gap-2 text-red-500"
                                  disabled={!isServiceReady}
                                  onClick={() => setDeleteId(api.id)}
                                >
                                  <Trash2 className="h-4 w-4" /> 删除聚合 API
                                </DropdownMenuItem>
                              </DropdownMenuContent>
                            </DropdownMenu>
                          </div>
                        </TableCell>
                      </TableRow>
                    );
                  })
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>

      <AggregateApiModal
        open={modalOpen}
        onOpenChange={setModalOpen}
        aggregateApi={editingApi}
        defaultSort={defaultCreateSort}
      />

      <ConfirmDialog
        open={Boolean(deleteId)}
        onOpenChange={(open) => !open && setDeleteId(null)}
        title="删除聚合 API"
        description="删除后将无法继续用于平台密钥轮转，是否确认删除？"
        confirmText="删除"
        cancelText="取消"
        onConfirm={() => {
          if (!deleteId) return;
          deleteMutation.mutate(deleteId);
          setDeleteId(null);
        }}
      />
    </div>
  );
}
