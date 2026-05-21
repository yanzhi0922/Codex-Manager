"use client";

import Link from "next/link";
import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Activity,
  ArrowRight,
  Boxes,
  CheckCircle2,
  CircleDashed,
  Compass,
  Database,
  FileClock,
  Gauge,
  KeyRound,
  Layers3,
  Network,
  Radar,
  ShieldCheck,
  TerminalSquare,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button, buttonVariants } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useDeferredDesktopActivation } from "@/hooks/useDeferredDesktopActivation";
import { usePageTransitionReady } from "@/hooks/usePageTransitionReady";
import { platformsClient } from "@/lib/api/platforms-client";
import { useAppStore } from "@/lib/store/useAppStore";
import { cn } from "@/lib/utils";
import type { PlatformDiscoveryItem, PlatformDiscoveryResult } from "@/types";

type CapabilityStatus = "live" | "next" | "research";

type CapabilityArea = {
  title: string;
  subtitle: string;
  status: CapabilityStatus;
  icon: LucideIcon;
  metrics: string[];
  strengths: string[];
  nextMoves: string[];
  href?: string;
};

type PlatformSignal = {
  name: string;
  state: "已内置" | "可复用" | "调研中";
  detail: string;
};

type RenderedPlatformSignal = {
  name: string;
  state: string;
  detail: string;
  path?: string | null;
};

const STATUS_META: Record<
  CapabilityStatus,
  { label: string; className: string; icon: LucideIcon }
> = {
  live: {
    label: "已落地",
    className: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-400",
    icon: CheckCircle2,
  },
  next: {
    label: "下一阶段",
    className: "bg-blue-500/10 text-blue-600 dark:text-blue-400",
    icon: CircleDashed,
  },
  research: {
    label: "调研中",
    className: "bg-amber-500/10 text-amber-600 dark:text-amber-400",
    icon: Radar,
  },
};

const CAPABILITY_AREAS: CapabilityArea[] = [
  {
    title: "账号池与配额",
    subtitle: "账号导入、计划识别、额度聚合、可用性分层",
    status: "live",
    icon: Gauge,
    href: "/accounts/",
    metrics: ["账号健康", "5小时/7天额度", "Plan 类型"],
    strengths: ["当前账号与推荐账号已在仪表盘聚合", "账号列表支持筛选、批量导入、批量清理"],
    nextMoves: ["增加账号组视角", "把低配额、封禁、过期授权做成统一事件流"],
  },
  {
    title: "本地网关",
    subtitle: "OpenAI 兼容入口、账号轮换、聚合 API 与请求观测",
    status: "live",
    icon: Network,
    href: "/logs/",
    metrics: ["路由策略", "失败回退", "请求日志"],
    strengths: ["已有账号优先级、聚合 API、请求日志和 token 统计", "保留本地服务/桌面/Web 三种运行入口"],
    nextMoves: ["补一张路由决策时间线", "把账号失败原因汇总到网关策略面板"],
  },
  {
    title: "Codex 会话",
    subtitle: "会话扫描、迁移、导出、备份、诊断与索引修复",
    status: "live",
    icon: FileClock,
    href: "/sessions/",
    metrics: ["Provider", "会话索引", "备份快照"],
    strengths: ["原 migrator 能力已并入 Rust service", "支持安全预览后迁移，默认创建备份"],
    nextMoves: ["增加工作区时间线", "把会话与请求日志按项目关联"],
  },
  {
    title: "平台密钥",
    subtitle: "面向外部客户端的密钥、模型、上游和用量控制",
    status: "live",
    icon: KeyRound,
    href: "/apikeys/",
    metrics: ["Key 状态", "模型限制", "用量统计"],
    strengths: ["平台密钥可绑定模型、服务层级和自定义上游", "密钥用量可独立统计"],
    nextMoves: ["增加密钥级别限额", "把密钥关联到具体项目/实例"],
  },
  {
    title: "多实例工作区",
    subtitle: "借鉴多 AI IDE 的实例隔离：独立目录、启动参数、生命周期",
    status: "next",
    icon: Boxes,
    metrics: ["实例目录", "启动命令", "绑定账号"],
    strengths: ["已有账号池、会话目录和桌面壳基础", "Codex CLI/桌面会话天然适合按工作区隔离"],
    nextMoves: ["定义 Codex 实例模型", "做启动/停止/打开目录三件套"],
  },
  {
    title: "跨工具平台矩阵",
    subtitle: "把支持状态、数据路径、登录方式和能力差异显式化",
    status: "research",
    icon: Layers3,
    metrics: ["支持状态", "凭证路径", "配额来源"],
    strengths: ["Codex 域已经具备账号、会话、网关三条主线", "后续可渐进接入其他 AI IDE，而不是一次性重写"],
    nextMoves: ["先做只读探测矩阵", "再决定是否实现账号注入和多开"],
  },
];

const PLATFORM_SIGNALS: PlatformSignal[] = [
  {
    name: "Codex",
    state: "已内置",
    detail: "账号池、网关、会话迁移、导出和索引修复已经在主应用内闭环。",
  },
  {
    name: "GitHub Copilot / VS Code",
    state: "调研中",
    detail: "适合先做实例目录、窗口启动和账号状态探测，不直接操作用户凭证。",
  },
  {
    name: "Cursor / Windsurf / Kiro",
    state: "调研中",
    detail: "可借鉴实例管理模型，但需要逐一确认本地数据格式和更新风险。",
  },
  {
    name: "Gemini CLI / Zed / Trae / Qoder",
    state: "调研中",
    detail: "优先做只读能力矩阵与导入/导出边界，避免破坏原客户端状态。",
  },
  {
    name: "Release / CI",
    state: "可复用",
    detail: "已补充 CI；后续可以继续增强 release preflight、版本同步和构建说明。",
  },
];

function formatDiscoveryState(status: string) {
  switch (status) {
    case "ready":
      return "可用";
    case "detected":
      return "已探测";
    case "planned":
      return "规划中";
    default:
      return "未发现";
  }
}

function platformDiscoveryToSignal(item: PlatformDiscoveryItem): RenderedPlatformSignal {
  const firstNote = item.notes[0] || "只读探测本地数据目录，不写入第三方客户端状态。";
  return {
    name: item.name,
    state: formatDiscoveryState(item.status),
    detail: firstNote,
    path: item.primaryPath,
  };
}

function StatusBadge({ status }: { status: CapabilityStatus }) {
  const meta = STATUS_META[status];
  const Icon = meta.icon;
  return (
    <Badge variant="secondary" className={cn("gap-1", meta.className)}>
      <Icon className="h-3 w-3" />
      {meta.label}
    </Badge>
  );
}

function CapabilityCard({ area }: { area: CapabilityArea }) {
  const Icon = area.icon;
  return (
    <Card className="glass-card border-none shadow-md">
      <CardHeader className="space-y-3">
        <div className="flex items-start justify-between gap-4">
          <div className="flex min-w-0 items-start gap-3">
            <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <Icon className="h-5 w-5" />
            </div>
            <div className="min-w-0">
              <CardTitle className="text-base">{area.title}</CardTitle>
              <p className="mt-1 text-xs text-muted-foreground">{area.subtitle}</p>
            </div>
          </div>
          <StatusBadge status={area.status} />
        </div>
        <div className="flex flex-wrap gap-2">
          {area.metrics.map((item) => (
            <Badge key={item} variant="outline">
              {item}
            </Badge>
          ))}
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-2 rounded-lg bg-muted/30 p-3">
            <div className="flex items-center gap-2 text-xs font-medium">
              <ShieldCheck className="h-3.5 w-3.5 text-emerald-500" />
              现有基础
            </div>
            <ul className="space-y-1.5 text-xs text-muted-foreground">
              {area.strengths.map((item) => (
                <li key={item} className="flex gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" />
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </div>
          <div className="space-y-2 rounded-lg bg-muted/30 p-3">
            <div className="flex items-center gap-2 text-xs font-medium">
              <Wrench className="h-3.5 w-3.5 text-blue-500" />
              下一步
            </div>
            <ul className="space-y-1.5 text-xs text-muted-foreground">
              {area.nextMoves.map((item) => (
                <li key={item} className="flex gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-blue-500" />
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
        {area.href ? (
          <Link
            href={area.href}
            className={cn(buttonVariants({ variant: "outline", size: "sm" }), "w-fit")}
          >
            打开相关页面
            <ArrowRight className="h-3.5 w-3.5" />
          </Link>
        ) : null}
      </CardContent>
    </Card>
  );
}

function SignalRow({ signal }: { signal: RenderedPlatformSignal }) {
  const tone =
    signal.state === "已内置" || signal.state === "可用"
      ? "text-emerald-600 dark:text-emerald-400"
      : signal.state === "可复用" || signal.state === "已探测"
        ? "text-blue-600 dark:text-blue-400"
        : "text-amber-600 dark:text-amber-400";

  return (
    <div className="grid gap-3 border-b border-border/50 px-4 py-3 last:border-b-0 md:grid-cols-[180px_110px_1fr]">
      <div className="text-sm font-medium">{signal.name}</div>
      <div className={cn("text-xs font-semibold", tone)}>{signal.state}</div>
      <div className="space-y-1 text-xs leading-5 text-muted-foreground">
        <div>{signal.detail}</div>
        {signal.path ? <div className="font-mono text-[11px]">{signal.path}</div> : null}
      </div>
    </div>
  );
}

export default function CockpitPage() {
  const { serviceStatus } = useAppStore();
  const canActivate = useDeferredDesktopActivation(serviceStatus.connected);
  const { data: discovery } = useQuery({
    queryKey: ["platforms", "discovery"],
    queryFn: async () => {
      const result = await platformsClient.discover();
      return result as PlatformDiscoveryResult;
    },
    enabled: serviceStatus.connected && canActivate,
    refetchInterval: 60_000,
  });
  const platformSignals = useMemo<RenderedPlatformSignal[]>(() => {
    if (discovery?.items?.length) {
      return discovery.items.map(platformDiscoveryToSignal);
    }
    return PLATFORM_SIGNALS.map((item) => ({
      name: item.name,
      state: item.state,
      detail: item.detail,
      path: null,
    }));
  }, [discovery]);
  usePageTransitionReady("/cockpit/", true);

  return (
    <div className="space-y-6 animate-in fade-in duration-500">
      <section className="rounded-xl border border-border/50 bg-card/50 p-5 shadow-sm">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="space-y-2">
            <div className="flex items-center gap-2 text-sm font-medium text-primary">
              <Compass className="h-4 w-4" />
              Codex-Copilot 能力驾驶舱
            </div>
            <h2 className="text-xl font-semibold tracking-normal">把账号、网关、会话和未来实例管理放到同一张产品地图里</h2>
            <p className="max-w-3xl text-sm leading-6 text-muted-foreground">
              参考 cockpit-tools 的产品分层方式，但不复制其代码：我们把可复用的是“平台能力矩阵、实例生命周期、配额状态总览”这些产品模型，
              并保持 Codex-Copilot 以 Codex 网关和会话管理为核心。
            </p>
          </div>
          <div className="grid w-full grid-cols-3 gap-2 text-center lg:w-auto lg:min-w-[260px]">
            <div className="rounded-lg bg-muted/40 p-3">
              <div className="text-lg font-bold">
                {discovery ? discovery.totals.ready + discovery.totals.detected : 4}
              </div>
              <div className="text-[11px] text-muted-foreground">
                {discovery ? "本机命中" : "已落地区域"}
              </div>
            </div>
            <div className="rounded-lg bg-muted/40 p-3">
              <div className="text-lg font-bold">2</div>
              <div className="text-[11px] text-muted-foreground">规划方向</div>
            </div>
            <div className="rounded-lg bg-muted/40 p-3">
              <div className="text-lg font-bold">0</div>
              <div className="text-[11px] text-muted-foreground">外部代码复制</div>
            </div>
          </div>
        </div>
      </section>

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {[
          { title: "账号视角", icon: Activity, text: "从账号状态和额度出发，决定路由和推荐。" },
          { title: "实例视角", icon: TerminalSquare, text: "下一阶段按工作区隔离 Codex 运行环境。" },
          { title: "平台视角", icon: Database, text: "显式记录每类 AI 工具的数据路径和风险边界。" },
          { title: "运维视角", icon: ShieldCheck, text: "把 CI、发布、备份和诊断纳入产品质量面板。" },
        ].map((item) => (
          <Card key={item.title} className="glass-card border-none shadow-md">
            <CardContent className="flex gap-3 p-4">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                <item.icon className="h-4 w-4" />
              </div>
              <div>
                <div className="text-sm font-semibold">{item.title}</div>
                <p className="mt-1 text-xs leading-5 text-muted-foreground">{item.text}</p>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <section className="grid gap-4 xl:grid-cols-2">
        {CAPABILITY_AREAS.map((area) => (
          <CapabilityCard key={area.title} area={area} />
        ))}
      </section>

      <section className="grid gap-4 xl:grid-cols-[1fr_360px]">
        <Card className="glass-card border-none shadow-md">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Layers3 className="h-4 w-4" />
              平台能力矩阵
            </CardTitle>
          </CardHeader>
          <CardContent className="p-0">
            <div className="border-t border-border/50 px-4 py-3 text-xs text-muted-foreground">
              {discovery
                ? "当前显示本机只读探测结果，不会写入任何第三方客户端数据。"
                : "服务未连接时显示产品规划矩阵；连接服务后会切换为本机只读探测结果。"}
            </div>
            <div className="border-t border-border/50">
              {platformSignals.map((signal) => (
                <SignalRow key={signal.name} signal={signal} />
              ))}
            </div>
          </CardContent>
        </Card>

        <Card className="glass-card border-none shadow-md">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base">
              <Compass className="h-4 w-4" />
              借鉴边界
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-muted-foreground">
            <p>
              cockpit-tools 的许可证限制商业使用和再分发，所以这里不引入其源码、资源或 UI 片段。
            </p>
            <p>
              可借鉴的是产品结构：Dashboard 聚合状态、账号与实例拆分、平台能力矩阵、配额和唤醒任务的独立视图。
            </p>
            <Button variant="outline" size="sm" className="w-fit" onClick={() => window.open("https://github.com/jlcodes99/cockpit-tools", "_blank", "noopener,noreferrer")}>
              查看参考项目
              <ArrowRight className="h-3.5 w-3.5" />
            </Button>
          </CardContent>
        </Card>
      </section>
    </div>
  );
}
