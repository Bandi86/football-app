//! Scheduler integrációs tesztek (Task 9).
//! Determinisztikus idő: fix bázis-időpont, nincs élő hálózat.
//! A core.match seed minimális, de FK-érvényes (country/entity/competition/season/team/match).

use chrono::{DateTime, Duration, TimeZone, Utc};
use football_domain::policy::{EventTemplate, JobStatus, SourceMode, SourcePolicy, Transport};
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::scheduler;
use sqlx::PgPool;
use uuid::Uuid;

/// 2026-11-12 12:00 UTC, csütörtök — ISO-hét: 2026-W46.
fn base_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 12, 12, 0, 0).unwrap()
}

fn policy_with(events: Vec<EventTemplate>, enabled: bool) -> SourcePolicy {
    SourcePolicy {
        source_slug: "demo-synthetic".into(),
        transport: Transport::Fixture,
        mode: SourceMode::Shadow,
        daily_budget: 100,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.0".into(),
        parser_version: "1.0".into(),
        domains: vec!["match".into()],
        events,
        enabled,
        max_bytes: 5_242_880,
    }
}

fn interval_event(event_type: &str, interval_minutes: i64) -> EventTemplate {
    EventTemplate {
        event_type: event_type.into(),
        offset_minutes: None,
        interval_minutes: Some(interval_minutes),
    }
}

fn kickoff_event(event_type: &str, offset_minutes: i64) -> EventTemplate {
    EventTemplate {
        event_type: event_type.into(),
        offset_minutes: Some(offset_minutes),
        interval_minutes: None,
    }
}

/// Minimális, FK-érvényes meccs-seed saját versenyország-kiszolgálással.
/// Minden hívás külön ország/verseny/csapat sorokat hoz létre, így a helper
/// többször is hívható egy teszben ütközés nélkül.
async fn seed_match(pool: &PgPool, kickoff: DateTime<Utc>, competition_priority: i32) -> Uuid {
    let match_id = Uuid::now_v7();
    let country_id = Uuid::now_v7();
    let competition_id = Uuid::now_v7();
    let season_id = Uuid::now_v7();
    let home_id = Uuid::now_v7();
    let away_id = Uuid::now_v7();
    // UUIDv7 előtagja időalapú (azonos ms-ben ütközne), ezért a véletlen farokból képzünk kódot.
    let country_code = format!("T{}", &country_id.simple().to_string()[26..]);

    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,'Tesztország')")
        .bind(country_id)
        .bind(&country_code)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Teszt Liga')",
    )
    .bind(competition_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1,$1,$2,'Teszt Liga',1,$3)",
    )
    .bind(competition_id)
    .bind(country_id)
    .bind(competition_priority)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/2027')")
        .bind(season_id)
        .bind(competition_id)
        .execute(pool)
        .await
        .unwrap();
    for (team_id, name) in [(home_id, "Hazai FC"), (away_id, "Vendég FC")] {
        sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
            .bind(team_id)
            .bind(name)
            .execute(pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO core.team (id, entity_id, country_id, name, gender)
             VALUES ($1,$1,$2,$3,'unknown')",
        )
        .bind(team_id)
        .bind(country_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO core.match
         (id, competition_id, season_id, home_team_id, away_team_id,
          kickoff_at, competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6,'UTC','scheduled')",
    )
    .bind(match_id)
    .bind(competition_id)
    .bind(season_id)
    .bind(home_id)
    .bind(away_id)
    .bind(kickoff)
    .execute(pool)
    .await
    .unwrap();
    match_id
}

async fn job_rows(pool: &PgPool) -> Vec<(String, String, String, i32)> {
    sqlx::query_as(
        "SELECT event_type, entity_key, time_window, priority FROM ops.job
         ORDER BY priority, event_type",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn generate_is_idempotent_and_prioritized() {
    let pool = test_pool().await;
    let now = base_time();
    let policy = policy_with(
        vec![
            interval_event("discovery", 1440),
            kickoff_event("refresh", -60),
        ],
        true,
    );
    repo::upsert_policy(&pool, &policy).await.unwrap();
    let kickoff = now + Duration::hours(20);
    let match_id = seed_match(&pool, kickoff, 10).await;

    let n1 = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    assert_eq!(n1, 2); // 1 discovery + 1 refresh
    let n2 = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    assert_eq!(n2, 0); // egyedi kulcs: nincs duplikátum

    // A kickoff idején mindkét job esedékes; a discovery (base 0) a refresh előtt jön.
    let claimed = scheduler::claim(&pool, Uuid::now_v7(), kickoff, 60, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].event_type, "discovery");
    assert_eq!(claimed[0].priority, 0); // interval: közelség 0, verseny-prioritás 0
    assert_eq!(claimed[0].time_window, "2026-11-12");
    assert_eq!(claimed[0].due_at, now);
    assert_eq!(claimed[1].event_type, "refresh");
    // 10 base + 10 verseny + 1200 közelség (generáláskor 20 óra van a kickoffig).
    assert_eq!(claimed[1].priority, 1220);
    assert_eq!(claimed[1].entity_key, match_id.to_string());
    assert_eq!(claimed[1].time_window, "2026-11-13");
    assert_eq!(claimed[1].due_at, kickoff - Duration::minutes(60));
}

#[tokio::test]
async fn interval_windows_daily_and_iso_weekly() {
    let pool = test_pool().await;
    let now = base_time();
    let policy = policy_with(
        vec![
            interval_event("discovery", 1440),
            interval_event("backfill", 10_080),
        ],
        true,
    );
    repo::upsert_policy(&pool, &policy).await.unwrap();

    let n = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    assert_eq!(n, 2);
    let rows = job_rows(&pool).await;
    assert_eq!(
        rows,
        vec![
            ("discovery".into(), "all".into(), "2026-11-12".into(), 0),
            ("backfill".into(), "all".into(), "2026-W46".into(), 100),
        ]
    );

    // Egy héttel később új napi és heti ablak nyílik, mindkettő új job.
    let next_week = now + Duration::days(7);
    let n = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), next_week)
        .await
        .unwrap();
    assert_eq!(n, 2);
    let windows: Vec<String> =
        sqlx::query_scalar("SELECT time_window FROM ops.job ORDER BY time_window")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(
        windows,
        vec!["2026-11-12", "2026-11-19", "2026-W46", "2026-W47"]
    );
}

#[tokio::test]
async fn kickoff_window_boundaries_and_proximity_clamp() {
    let pool = test_pool().await;
    let now = base_time();
    let policy = policy_with(vec![kickoff_event("refresh", -60)], true);
    repo::upsert_policy(&pool, &policy).await.unwrap();

    // Ablakhatáron belül: [now-72h, now+24h] (BETWEEN, inkluzív).
    let at_past_edge = seed_match(&pool, now - Duration::hours(72), 5).await;
    let at_future_edge = seed_match(&pool, now + Duration::hours(24), 5).await;
    // Ablakon kívül: nem keletkezhet job.
    let outside_past = seed_match(&pool, now - Duration::hours(73), 5).await;
    let outside_future = seed_match(&pool, now + Duration::hours(25), 5).await;

    let n = scheduler::generate_jobs(&pool, &[policy], now)
        .await
        .unwrap();
    assert_eq!(n, 2);
    let rows = job_rows(&pool).await;
    // Múltbeli meccs: közelség 0-ra clampolva → 10 + 5 + 0 = 15.
    assert_eq!(
        rows[0],
        (
            "refresh".into(),
            at_past_edge.to_string(),
            "2026-11-09".into(),
            15
        )
    );
    // +24h: közelség 1440 → 10 + 5 + 1440 = 1455.
    assert_eq!(
        rows[1],
        (
            "refresh".into(),
            at_future_edge.to_string(),
            "2026-11-13".into(),
            1455
        )
    );
    let outside_jobs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.job WHERE entity_key = ANY($1)")
            .bind(vec![outside_past.to_string(), outside_future.to_string()])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(outside_jobs, 0);
}

#[tokio::test]
async fn claim_requeue_and_complete_lifecycle() {
    let pool = test_pool().await;
    let now = base_time();
    let policy = policy_with(vec![interval_event("discovery", 1440)], true);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    assert_eq!(
        scheduler::generate_jobs(&pool, &[policy], now)
            .await
            .unwrap(),
        1
    );

    let worker = Uuid::now_v7();
    let claimed = scheduler::claim(&pool, worker, now, 60, 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    let job_id = claimed[0].id;
    // Lease él: nem jár le, nem sorolódik vissza.
    assert_eq!(
        scheduler::requeue_expired(&pool, now + Duration::seconds(30))
            .await
            .unwrap(),
        0
    );
    // Lejárt lease visszasorolása, majd új claim ugyanarra a jobra.
    assert_eq!(
        scheduler::requeue_expired(&pool, now + Duration::seconds(61))
            .await
            .unwrap(),
        1
    );
    let again = scheduler::claim(&pool, worker, now + Duration::seconds(61), 60, 10)
        .await
        .unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].id, job_id);
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM ops.job WHERE id=$1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 2);

    scheduler::complete(&pool, job_id, JobStatus::Done, None)
        .await
        .unwrap();
    let (status, worker_id, leased_until): (String, Option<Uuid>, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT status, worker_id, leased_until FROM ops.job WHERE id=$1")
            .bind(job_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(status, "done");
    assert_eq!(worker_id, None);
    assert_eq!(leased_until, None);
    assert!(
        scheduler::claim(&pool, worker, now + Duration::seconds(62), 60, 10)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn disabled_policy_generates_nothing() {
    let pool = test_pool().await;
    let now = base_time();
    let policy = policy_with(vec![interval_event("discovery", 1440)], false);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    assert_eq!(
        scheduler::generate_jobs(&pool, &[policy], now)
            .await
            .unwrap(),
        0
    );
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ops.job")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
}
