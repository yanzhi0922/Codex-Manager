use codexmanager_core::storage::{Account, Storage, Token, UsageSnapshotRecord};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum CandidateSkipReason {
    Cooldown,
    Inflight,
}

pub(crate) fn prepare_gateway_candidates(
    storage: &Storage,
    _request_model: Option<&str>,
) -> Result<Vec<(Account, Token)>, String> {
    let candidates = super::super::super::collect_gateway_candidates(storage)?;
    Ok(rank_gateway_candidates(storage, candidates))
}

fn rank_gateway_candidates(
    storage: &Storage,
    candidates: Vec<(Account, Token)>,
) -> Vec<(Account, Token)> {
    if candidates.len() <= 1 {
        return candidates;
    }

    let usage_snapshots = match storage.latest_usage_snapshots_by_account() {
        Ok(snapshots) => snapshots,
        Err(err) => {
            log::warn!("gateway candidate ranking skipped usage snapshots: {err}");
            Vec::new()
        }
    };
    let usage_by_account = usage_snapshots
        .into_iter()
        .map(|snapshot| (snapshot.account_id.clone(), snapshot))
        .collect::<HashMap<_, _>>();
    let mut ranked = candidates.into_iter().enumerate().collect::<Vec<_>>();
    ranked.sort_by(
        |(left_idx, (left_account, _)), (right_idx, (right_account, _))| {
            let left_score = gateway_candidate_score(
                left_account,
                usage_by_account.get(left_account.id.as_str()),
            );
            let right_score = gateway_candidate_score(
                right_account,
                usage_by_account.get(right_account.id.as_str()),
            );
            right_score
                .cmp(&left_score)
                .then(left_account.sort.cmp(&right_account.sort))
                .then(right_account.updated_at.cmp(&left_account.updated_at))
                .then(left_idx.cmp(right_idx))
        },
    );
    ranked.into_iter().map(|(_, candidate)| candidate).collect()
}

fn gateway_candidate_score(account: &Account, usage: Option<&UsageSnapshotRecord>) -> i32 {
    let mut score = usage_headroom_basis_points(usage);
    score -= super::super::super::account_inflight_count(account.id.as_str()) as i32 * 250;
    if super::super::super::is_account_in_cooldown(account.id.as_str()) {
        score -= 1_500;
    }
    score
}

fn usage_headroom_basis_points(usage: Option<&UsageSnapshotRecord>) -> i32 {
    usage
        .and_then(usage_remaining_percent)
        .map(|remaining| (remaining * 10.0).round() as i32)
        .unwrap_or(500)
}

fn usage_remaining_percent(usage: &UsageSnapshotRecord) -> Option<f64> {
    let mut remaining = Vec::new();
    if let Some(used) = usage.used_percent {
        remaining.push((100.0 - used).clamp(0.0, 100.0));
    }
    if let Some(used) = usage.secondary_used_percent {
        remaining.push((100.0 - used).clamp(0.0, 100.0));
    }
    remaining.into_iter().reduce(f64::min)
}

pub(in super::super) fn free_account_model_override(
    storage: &Storage,
    account: &Account,
    token: &Token,
) -> Option<String> {
    if !crate::account_plan::is_free_or_single_window_account(storage, account.id.as_str(), token) {
        return None;
    }
    let configured = super::super::super::current_free_account_max_model();
    if configured.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(configured)
    }
}

pub(in super::super) fn candidate_skip_reason_for_proxy(
    account_id: &str,
    idx: usize,
    candidate_count: usize,
    account_max_inflight: usize,
) -> Option<CandidateSkipReason> {
    // 中文注释：当用户手动“切到当前”后，首候选应持续优先命中；
    // 仅在真实请求失败时由上游流程自动清除手动锁定，再回退常规轮转。
    let is_manual_preferred_head = idx == 0
        && super::super::super::manual_preferred_account()
            .as_deref()
            .is_some_and(|manual_id| manual_id == account_id);
    if is_manual_preferred_head {
        return None;
    }

    let has_more_candidates = idx + 1 < candidate_count;
    if super::super::super::is_account_in_cooldown(account_id) && has_more_candidates {
        super::super::super::record_gateway_failover_attempt();
        return Some(CandidateSkipReason::Cooldown);
    }

    if account_max_inflight > 0
        && super::super::super::account_inflight_count(account_id) >= account_max_inflight
        && has_more_candidates
    {
        // 中文注释：并发上限是软约束，最后一个候选仍要尝试，避免把可恢复抖动直接放大成全局不可用。
        super::super::super::record_gateway_failover_attempt();
        return Some(CandidateSkipReason::Inflight);
    }

    None
}
#[cfg(test)]
mod tests {
    use super::{free_account_model_override, rank_gateway_candidates};
    use codexmanager_core::storage::{now_ts, Account, Storage, Token, UsageSnapshotRecord};

    fn insert_candidate_account(storage: &Storage, account_id: &str, sort: i64, now: i64) {
        storage
            .insert_account(&Account {
                id: account_id.to_string(),
                label: account_id.to_string(),
                issuer: "issuer".to_string(),
                chatgpt_account_id: None,
                workspace_id: None,
                group_name: None,
                sort,
                status: "active".to_string(),
                created_at: now,
                updated_at: now + sort,
            })
            .expect("insert account");
        storage
            .insert_token(&Token {
                account_id: account_id.to_string(),
                id_token: "header.payload.sig".to_string(),
                access_token: "header.payload.sig".to_string(),
                refresh_token: "refresh".to_string(),
                api_key_access_token: None,
                last_refresh: now,
            })
            .expect("insert token");
    }

    fn insert_usage_snapshot(
        storage: &Storage,
        account_id: &str,
        used_percent: Option<f64>,
        secondary_used_percent: Option<f64>,
        now: i64,
    ) {
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: account_id.to_string(),
                used_percent,
                window_minutes: Some(300),
                resets_at: None,
                secondary_used_percent,
                secondary_window_minutes: Some(10_080),
                secondary_resets_at: None,
                credits_json: None,
                captured_at: now,
            })
            .expect("insert usage");
    }

    #[test]
    fn free_account_model_override_uses_configured_model_for_free_account() {
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_account(&Account {
                id: "acc-free".to_string(),
                label: "acc-free".to_string(),
                issuer: "issuer".to_string(),
                chatgpt_account_id: None,
                workspace_id: None,
                group_name: None,
                sort: 0,
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            })
            .expect("insert account");
        let token = Token {
            account_id: "acc-free".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        };
        storage.insert_token(&token).expect("insert token");
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: "acc-free".to_string(),
                used_percent: Some(10.0),
                window_minutes: Some(300),
                resets_at: None,
                secondary_used_percent: Some(20.0),
                secondary_window_minutes: Some(10_080),
                secondary_resets_at: None,
                credits_json: Some(r#"{"planType":"free"}"#.to_string()),
                captured_at: now,
            })
            .expect("insert usage");

        let original = crate::gateway::current_free_account_max_model();
        crate::gateway::set_free_account_max_model("gpt-5.2").expect("set free model");

        let account = Account {
            id: "acc-free".to_string(),
            label: "acc-free".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };
        let actual = free_account_model_override(&storage, &account, &token);

        let _ = crate::gateway::set_free_account_max_model(&original);

        assert_eq!(actual.as_deref(), Some("gpt-5.2"));
    }

    #[test]
    fn free_account_model_override_accepts_single_window_weekly_account() {
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_account(&Account {
                id: "acc-weekly".to_string(),
                label: "acc-weekly".to_string(),
                issuer: "issuer".to_string(),
                chatgpt_account_id: None,
                workspace_id: None,
                group_name: None,
                sort: 0,
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            })
            .expect("insert account");
        let token = Token {
            account_id: "acc-weekly".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        };
        storage.insert_token(&token).expect("insert token");
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: "acc-weekly".to_string(),
                used_percent: Some(10.0),
                window_minutes: Some(10_080),
                resets_at: None,
                secondary_used_percent: None,
                secondary_window_minutes: None,
                secondary_resets_at: None,
                credits_json: None,
                captured_at: now,
            })
            .expect("insert usage");

        let original = crate::gateway::current_free_account_max_model();
        crate::gateway::set_free_account_max_model("gpt-5.2").expect("set free model");

        let account = Account {
            id: "acc-weekly".to_string(),
            label: "acc-weekly".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };
        let actual = free_account_model_override(&storage, &account, &token);

        let _ = crate::gateway::set_free_account_max_model(&original);

        assert_eq!(actual.as_deref(), Some("gpt-5.2"));
    }

    #[test]
    fn free_account_model_override_skips_rewrite_when_configured_auto() {
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        storage
            .insert_account(&Account {
                id: "acc-auto".to_string(),
                label: "acc-auto".to_string(),
                issuer: "issuer".to_string(),
                chatgpt_account_id: None,
                workspace_id: None,
                group_name: None,
                sort: 0,
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            })
            .expect("insert account");
        let token = Token {
            account_id: "acc-auto".to_string(),
            id_token: "header.payload.sig".to_string(),
            access_token: "header.payload.sig".to_string(),
            refresh_token: "refresh".to_string(),
            api_key_access_token: None,
            last_refresh: now,
        };
        storage.insert_token(&token).expect("insert token");
        storage
            .insert_usage_snapshot(&UsageSnapshotRecord {
                account_id: "acc-auto".to_string(),
                used_percent: Some(10.0),
                window_minutes: Some(300),
                resets_at: None,
                secondary_used_percent: Some(20.0),
                secondary_window_minutes: Some(10_080),
                secondary_resets_at: None,
                credits_json: Some(r#"{"planType":"free"}"#.to_string()),
                captured_at: now,
            })
            .expect("insert usage");

        let original = crate::gateway::current_free_account_max_model();
        crate::gateway::set_free_account_max_model("auto").expect("set free model");

        let account = Account {
            id: "acc-auto".to_string(),
            label: "acc-auto".to_string(),
            issuer: "issuer".to_string(),
            chatgpt_account_id: None,
            workspace_id: None,
            group_name: None,
            sort: 0,
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        };
        let actual = free_account_model_override(&storage, &account, &token);

        let _ = crate::gateway::set_free_account_max_model(&original);

        assert_eq!(actual, None);
    }

    #[test]
    fn prepare_gateway_candidates_prefers_lower_load_and_more_headroom() {
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        insert_candidate_account(&storage, "acc-load-a", 0, now);
        insert_candidate_account(&storage, "acc-load-b", 1, now);
        insert_candidate_account(&storage, "acc-load-c", 2, now);

        insert_usage_snapshot(&storage, "acc-load-a", Some(80.0), Some(80.0), now);
        insert_usage_snapshot(&storage, "acc-load-b", Some(10.0), Some(10.0), now);
        insert_usage_snapshot(&storage, "acc-load-c", Some(5.0), Some(5.0), now);

        let _busy = crate::gateway::acquire_account_inflight("acc-load-b");

        let raw_candidates = storage
            .list_gateway_candidates()
            .expect("list gateway candidates");
        let candidates = rank_gateway_candidates(&storage, raw_candidates);
        let ids = candidates
            .into_iter()
            .map(|(account, _)| account.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["acc-load-c", "acc-load-b", "acc-load-a"]);
    }

    #[test]
    fn prepare_gateway_candidates_pushes_cooldown_accounts_to_the_end() {
        let storage = Storage::open_in_memory().expect("open");
        storage.init().expect("init");
        let now = now_ts();
        insert_candidate_account(&storage, "acc-cool-a", 0, now);
        insert_candidate_account(&storage, "acc-cool-b", 1, now);

        crate::gateway::mark_account_cooldown(
            "acc-cool-a",
            crate::gateway::CooldownReason::RateLimited,
        );
        let raw_candidates = storage
            .list_gateway_candidates()
            .expect("list gateway candidates");
        let candidates = rank_gateway_candidates(&storage, raw_candidates);
        crate::gateway::clear_account_cooldown("acc-cool-a");

        let ids = candidates
            .into_iter()
            .map(|(account, _)| account.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["acc-cool-b", "acc-cool-a"]);
    }
}
