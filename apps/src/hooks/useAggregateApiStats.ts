"use client";

import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { serviceClient } from "@/lib/api/service-client";
import { AggregateApi } from "@/types";

interface UseAggregateApiStatsOptions {
  aggregateApis: AggregateApi[];
  enabled: boolean;
  active: boolean;
}

export function useAggregateApiStats({
  aggregateApis,
  enabled,
  active,
}: UseAggregateApiStatsOptions) {
  const queryEnabled = enabled && active;

  const { data: summary, isLoading: isSummaryLoading } = useQuery({
    queryKey: ["aggregate-api-dashboard", "summary"],
    queryFn: () =>
      serviceClient.getRequestLogSummary({
        aggregateOnly: true,
        query: "",
        statusFilter: "all",
      }),
    enabled: queryEnabled,
    refetchInterval: 5000,
    retry: 1,
  });

  const { data: today, isLoading: isTodayLoading } = useQuery({
    queryKey: ["aggregate-api-dashboard", "today"],
    queryFn: () =>
      serviceClient.getTodaySummary({
        aggregateOnly: true,
      }),
    enabled: queryEnabled,
    refetchInterval: 5000,
    retry: 1,
  });

  const connectivity = useMemo(() => {
    const total = aggregateApis.length;
    const success = aggregateApis.filter(
      (item) => item.lastTestStatus === "success",
    ).length;
    const failed = aggregateApis.filter(
      (item) => item.lastTestStatus === "failed",
    ).length;
    const untested = Math.max(0, total - success - failed);

    return {
      total,
      success,
      failed,
      untested,
    };
  }, [aggregateApis]);

  return {
    stats: {
      totalApis: connectivity.total,
      connectedApis: connectivity.success,
      failedApis: connectivity.failed,
      untestedApis: connectivity.untested,
      totalRequests: summary?.filteredCount ?? 0,
      successRequests: summary?.successCount ?? 0,
      failedRequests: summary?.errorCount ?? 0,
      totalTokens: summary?.totalTokens ?? 0,
      todayTokens: today?.todayTokens ?? 0,
      todayCost: today?.estimatedCost ?? 0,
      cachedTokens: today?.cachedInputTokens ?? 0,
      reasoningTokens: today?.reasoningOutputTokens ?? 0,
    },
    isLoading: (queryEnabled && isSummaryLoading) || (queryEnabled && isTodayLoading),
  };
}
