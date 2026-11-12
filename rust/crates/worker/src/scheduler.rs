//! Ütemező (Task 9): idempotens job-generálás policy-eseményekből,
//! valamint claim / lease-visszasorolás / befejezés repo-wrapperek.
//!
//! A generálás az ops.job egyedi kulcsára támaszkodik
//! (source_slug, domain, entity_key, event_type, time_window):
//! ugyanarra az ablakra újra futtatva nem keletkezik duplikátum.

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Utc};
use football_domain::policy::{FetchJob, JobStatus, SourcePolicy};
use football_storage::repositories::worker::{self as repo, NewJob};
use sqlx::PgPool;
use uuid::Uuid;

/// Kickoff-ablak: a múltbeli 72 óra és a jövőbeli 24 óra közötti meccsek.
const KICKOFF_PAST_HOURS: i64 = 72;
const KICKOFF_FUTURE_HOURS: i64 = 24;
/// Ettől az interval-perctől (7 nap) az ablak ISO-hét, alatta UTC nap.
const WEEKLY_INTERVAL_MINUTES: i64 = 7 * 24 * 60;
/// A kickoff-közelség felső korlátja percben (24 óra).
const MAX_PROXIMITY_MINUTES: i64 = 1440;

/// Esemény-alapprioritás; kisebb = előbb. Az ismeretlen eseménytípus
/// konzervatívan a backfill szintjére kerül, hogy ne előzzön meg ismert munkát.
fn event_base(event_type: &str) -> i32 {
    match event_type {
        "discovery" => 0,
        "refresh" => 10,
        "canary" => 50,
        "drift" => 60,
        "backfill" => 100,
        _ => 100,
    }
}

/// Interval esemény időablaka: napi UTC dátum vagy ISO-hét ("2026-W46").
fn interval_window(now: DateTime<Utc>, interval_minutes: i64) -> String {
    if interval_minutes >= WEEKLY_INTERVAL_MINUTES {
        let iso = now.iso_week();
        format!("{}-W{:02}", iso.year(), iso.week())
    } else {
        now.date_naive().to_string()
    }
}

/// Jobok generálása az engedélyezett policy-k eseményeiből.
/// A policy-soroknak már létezniük kell az ops.source_policy-ban (FK);
/// a visszatérési érték a ténylegesen beszúrt új jobok száma.
pub async fn generate_jobs(
    pool: &PgPool,
    policies: &[SourcePolicy],
    now: DateTime<Utc>,
) -> Result<usize> {
    let enabled: Vec<&SourcePolicy> = policies.iter().filter(|p| p.enabled).collect();
    let needs_kickoff = enabled
        .iter()
        .any(|p| p.events.iter().any(|e| e.offset_minutes.is_some()));
    let kickoff_matches = if needs_kickoff {
        Some(
            repo::kickoff_window(
                pool,
                now - Duration::hours(KICKOFF_PAST_HOURS),
                now + Duration::hours(KICKOFF_FUTURE_HOURS),
            )
            .await?,
        )
    } else {
        None
    };

    let mut inserted = 0usize;
    for policy in enabled {
        for domain in &policy.domains {
            for event in &policy.events {
                if let Some(offset) = event.offset_minutes {
                    let Some(matches) = &kickoff_matches else {
                        continue;
                    };
                    for (match_id, kickoff_at, competition_priority) in matches {
                        let proximity = (*kickoff_at - now)
                            .num_minutes()
                            .clamp(0, MAX_PROXIMITY_MINUTES)
                            as i32;
                        let priority = event_base(&event.event_type)
                            .saturating_add(*competition_priority)
                            .saturating_add(proximity);
                        inserted += repo::insert_job_counted(
                            pool,
                            &NewJob {
                                source_slug: policy.source_slug.clone(),
                                domain: domain.clone(),
                                entity_key: match_id.to_string(),
                                event_type: event.event_type.clone(),
                                time_window: kickoff_at.date_naive().to_string(),
                                due_at: *kickoff_at + Duration::minutes(offset),
                                priority,
                            },
                        )
                        .await?;
                    }
                } else if let Some(interval) = event.interval_minutes {
                    // Interval job: forrás-szintű, nem entitás-specifikus;
                    // a közelség és a verseny-prioritás ilyenkor 0.
                    inserted += repo::insert_job_counted(
                        pool,
                        &NewJob {
                            source_slug: policy.source_slug.clone(),
                            domain: domain.clone(),
                            entity_key: "all".into(),
                            event_type: event.event_type.clone(),
                            time_window: interval_window(now, interval),
                            due_at: now,
                            priority: event_base(&event.event_type),
                        },
                    )
                    .await?;
                }
                // Sem offset, sem interval: érvénytelen sablon, kihagyjuk.
            }
        }
    }
    Ok(inserted)
}

/// Esedékes jobok igénylése lease-szel (repo wrapper).
pub async fn claim(
    pool: &PgPool,
    worker_id: Uuid,
    now: DateTime<Utc>,
    lease_secs: i64,
    limit: i64,
) -> Result<Vec<FetchJob>> {
    repo::claim_due_jobs(pool, worker_id, now, lease_secs, limit).await
}

/// Lejárt lease-ek visszasorolása pending állapotba (repo wrapper).
pub async fn requeue_expired(pool: &PgPool, now: DateTime<Utc>) -> Result<u64> {
    repo::requeue_expired_leases(pool, now).await
}

/// Job befejezése végleges állapottal (repo wrapper).
pub async fn complete(
    pool: &PgPool,
    job_id: Uuid,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<()> {
    repo::complete_job(pool, job_id, status, last_error).await
}
