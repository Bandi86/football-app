//! Ütemező (Task 9): idempotens job-generálás policy-eseményekből,
//! valamint claim / lease-visszasorolás / befejezés repo-wrapperek.
//!
//! A generálás az ops.job egyedi kulcsára támaszkodik
//! (source_slug, domain, entity_key, event_type, time_window):
//! ugyanarra az ablakra újra futtatva nem keletkezik duplikátum.

use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Utc};
use football_domain::policy::{JobLease, JobStatus, LeasedJob, SourcePolicy};
use football_storage::repositories::worker::{self as repo, NewJob};
use sqlx::PgPool;
use uuid::Uuid;

/// Kickoff-ablak: a múltbeli 72 óra és a jövőbeli 24 óra közötti meccsek.
const KICKOFF_PAST_HOURS: i64 = 72;
const KICKOFF_FUTURE_HOURS: i64 = 24;
/// Ettől az interval-perctől (7 nap) az ablak ISO-hét, alatta UTC nap.
const WEEKLY_INTERVAL_MINUTES: i64 = 7 * 24 * 60;
/// A napi interval percben; ennél kisebb, 1440-et osztó intervalok
/// napon belüli frissítési sávokat jelentenek (S36b).
const DAILY_INTERVAL_MINUTES: i64 = 24 * 60;
/// A napon belüli sávok legkisebb engedett intervalja percben.
const MIN_SLOT_INTERVAL_MINUTES: i64 = 30;
/// A kickoff-közelség felső korlátja percben (24 óra).
const MAX_PROXIMITY_MINUTES: i64 = 1440;
/// S45: a research forrás visszamenőleges (eredmény-pótló) frissítésének
/// ablaka napokban. A research projekt napi 08:00-s köre az előző napokat is
/// újratölti, ezért a v2-nek is naponta egyszer újra be kell olvasnia őket.
const RESEARCH_CATCHUP_DAYS: i64 = 7;
/// S136: a catch-up nem indulhat azonnal a daemon indításakor (07:31), mert
/// a research napi fájlja csak kb. 07:38-kor frissül — a job így elavult
/// `scheduled` státuszokat olvasott be, és naponta egyszeri idempotencia
/// miatt nem futott újra. Egy órás késleltetés biztosítja a friss forrást.
const RESEARCH_CATCHUP_DELAY_MINUTES: i64 = 60;

/// Engedett interval: napi (1440), heti (10080), vagy a napot egyenlő
/// sávokra osztó napon belüli frissítés (pl. 240 → napi 6 sáv).
fn supported_interval(interval: i64) -> bool {
    interval == DAILY_INTERVAL_MINUTES
        || interval == WEEKLY_INTERVAL_MINUTES
        || ((MIN_SLOT_INTERVAL_MINUTES..DAILY_INTERVAL_MINUTES).contains(&interval)
            && DAILY_INTERVAL_MINUTES % interval == 0)
}

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
                if !matches!(
                    event.event_type.as_str(),
                    "discovery" | "refresh" | "canary" | "drift" | "backfill"
                ) {
                    tracing::warn!(
                        source = %policy.source_slug,
                        event_type = %event.event_type,
                        "ismeretlen eseménytípus, a sablon kihagyva"
                    );
                    continue;
                }
                if let Some(interval) = event.interval_minutes
                    && !supported_interval(interval)
                {
                    tracing::warn!(
                        source = %policy.source_slug,
                        interval,
                        "nem engedett interval_minutes (1440/10080 vagy a napot osztó 30..1439), a sablon kihagyva"
                    );
                    continue;
                }
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
                } else if let Some(interval) = event.interval_minutes
                    && interval < DAILY_INTERVAL_MINUTES
                {
                    // Napon belüli frissítés (S36b): a napot 1440/interval
                    // egyenlő sávra osztjuk; sávonként egy forrás-szintű job,
                    // a sáv kezdetére ütemezve. A jövőbeli sávok a due_at-ig
                    // pending-ben várnak, a claim csak az esedékeset viszi.
                    let midnight = now
                        .date_naive()
                        .and_hms_opt(0, 0, 0)
                        .expect("éjfél mindig létezik")
                        .and_utc();
                    let slots = (DAILY_INTERVAL_MINUTES / interval) as usize;
                    for slot in 0..slots {
                        inserted += repo::insert_job_counted(
                            pool,
                            &NewJob {
                                source_slug: policy.source_slug.clone(),
                                domain: domain.clone(),
                                entity_key: format!("slot-{slot:02}"),
                                event_type: event.event_type.clone(),
                                time_window: now.date_naive().to_string(),
                                due_at: midnight + Duration::minutes(slot as i64 * interval),
                                priority: event_base(&event.event_type).saturating_add(slot as i32),
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
            // S45: a helyi research forrásnál a napi 08:00-s kör után a
            // korábbi napok eredményei is frissülnek; ezért naponta egyszer
            // visszamenőleges „catch-up" refresh jobot adunk az elmúlt hétre.
            // A (entity_key, time_window) pár naponta új, így naponta egy új
            // refresh fut le naponként, de egy napon belül nem duplikálódik.
            if policy.source_slug == "research" {
                for days_back in 1..=RESEARCH_CATCHUP_DAYS {
                    let Some(target) = now
                        .date_naive()
                        .checked_sub_signed(Duration::days(days_back))
                    else {
                        continue;
                    };
                    inserted += repo::insert_job_counted(
                        pool,
                        &NewJob {
                            source_slug: policy.source_slug.clone(),
                            domain: "day".into(),
                            entity_key: format!("catchup-{target}"),
                            event_type: "refresh".into(),
                            time_window: now.date_naive().to_string(),
                            due_at: now + Duration::minutes(RESEARCH_CATCHUP_DELAY_MINUTES),
                            priority: event_base("backfill") + days_back as i32,
                        },
                    )
                    .await?;
                }
            }
            // S192: a Tipplap részletes piacai egy kérés/meccs. Csak a mai
            // napon megfigyelt ÉS kanonikus meccshez linkelt event ID-kból
            // generálunk jobot — a review-ra várók kimaradnak (fail-closed).
            // A domain nem szerepel a policy `domains` listájában, így a
            // fenti sablonok nem generálnak érvénytelen `all`/`slot-*` jobot.
            if policy.source_slug == "tipplap" {
                let ids = repo::linked_tipplap_event_ids(pool, "tipplap", now.date_naive()).await?;
                for (index, event_id) in ids.into_iter().enumerate() {
                    inserted += repo::insert_job_counted(
                        pool,
                        &NewJob {
                            source_slug: policy.source_slug.clone(),
                            domain: "markets".into(),
                            entity_key: format!("markets-{event_id}"),
                            event_type: "refresh".into(),
                            time_window: now.date_naive().to_string(),
                            // Apró szétütemezés, hogy egy tick ne induljon
                            // több tucat kéréssel egyszerre.
                            due_at: now + Duration::seconds((index % 60) as i64 * 5),
                            priority: event_base("refresh"),
                        },
                    )
                    .await?;
                }
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
) -> Result<Vec<LeasedJob>> {
    repo::claim_due_jobs(pool, worker_id, now, lease_secs, limit).await
}

/// Lejárt lease-ek visszasorolása pending állapotba (repo wrapper).
pub async fn requeue_expired(pool: &PgPool, now: DateTime<Utc>) -> Result<u64> {
    repo::requeue_expired_leases(pool, now).await
}

/// Job befejezése végleges állapottal (repo wrapper).
pub async fn complete(
    pool: &PgPool,
    lease: &JobLease,
    status: JobStatus,
    last_error: Option<&str>,
) -> Result<bool> {
    repo::complete_leased(pool, lease, status, last_error).await
}
