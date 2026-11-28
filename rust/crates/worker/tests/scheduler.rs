//! Scheduler integrációs tesztek (Task 9).
//! Determinisztikus idő: fix bázis-időpont, nincs élő hálózat.
//! A core.match seed minimális, de FK-érvényes (country/entity/competition/season/team/match).

use chrono::{DateTime, Duration, SubsecRound, TimeZone, Utc};
use football_domain::policy::{EventTemplate, JobStatus, SourceMode, SourcePolicy, Transport};
use football_storage::repositories::worker as repo;
use football_storage::test_support::test_pool;
use football_worker::scheduler;
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::test]
async fn markets_jobs_come_only_from_linked_tipplap_events() {
    let pool = test_pool().await;
    let now = base_time();
    let match_id = seed_match(&pool, now, 50).await;

    let source_id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,'tipplap','Tipplap')")
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();

    let artifact_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.artifact
         (id, content_sha256, compression, byte_size, storage_key, media_type)
         VALUES ($1,$2,'none',0,$3,'application/json')",
    )
    .bind(artifact_id)
    .bind(format!("sha-{artifact_id}"))
    .bind(format!("key-{artifact_id}"))
    .execute(&pool)
    .await
    .unwrap();
    let capture_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO raw.capture
         (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
          received_at, http_status, adapter_version, parser_version, request_key)
         VALUES ($1,$2,$3,'day','https://example.test/day',$4,$4,200,'t','p',$5)",
    )
    .bind(capture_id)
    .bind(source_id)
    .bind(artifact_id)
    .bind(now)
    .bind(format!("req-{capture_id}"))
    .execute(&pool)
    .await
    .unwrap();

    // Két napi observation: csak az egyikhez tartozik aktív kanonikus ref.
    for key in ["tipplap-5621252", "tipplap-5599999"] {
        sqlx::query(
            "INSERT INTO stage.observation
             (id, source_id, capture_id, entity_kind, aspect, natural_key,
              source_record_key, observed_at, normalized_payload,
              schema_version, quality_flags, content_hash)
             VALUES ($1,$2,$3,'match','match_core',$4,$4,$5,'{}'::jsonb,'1.0','{}',$6)",
        )
        .bind(Uuid::now_v7())
        .bind(source_id)
        .bind(capture_id)
        .bind(key)
        .bind(now)
        .bind(format!("hash-{key}"))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
         (id, source_id, entity_kind, source_entity_id, entity_id, method, confidence)
         VALUES ($1,$2,'match','tipplap-5621252',$3,'exact_external_id',1.0)",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(match_id)
    .execute(&pool)
    .await
    .unwrap();

    let ids = repo::linked_tipplap_event_ids(&pool, "tipplap", now.date_naive())
        .await
        .unwrap();
    assert_eq!(ids, vec!["5621252".to_owned()]);

    // Az ops.job.source_slug az ops.source_policy-ra hivatkozik (FK), és a
    // tesztadatbázisban ez a sor nincs benne alapból.
    sqlx::query(
        "INSERT INTO ops.source_policy
         (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
          breaker_threshold, breaker_cooldown_secs, adapter_version,
          parser_version, domains, events, enabled, max_bytes)
         VALUES ('tipplap','rust_http','shadow',150,0,0,3,60,'1.1','1.2',
                 '[\"day\"]'::jsonb,
                 '[{\"event_type\":\"discovery\",\"offset_minutes\":null,\"interval_minutes\":1440}]'::jsonb,
                 true, 5242880)
         ON CONFLICT (source_slug) DO NOTHING",
    )
    .execute(&pool)
    .await
    .unwrap();

    let policy = SourcePolicy {
        source_slug: "tipplap".into(),
        transport: Transport::RustHttp,
        mode: SourceMode::Shadow,
        daily_budget: 150,
        min_delay_ms: 0,
        jitter_ms: 0,
        breaker_threshold: 3,
        breaker_cooldown_secs: 60,
        adapter_version: "1.1".into(),
        parser_version: "1.2".into(),
        domains: vec!["day".into()],
        events: vec![interval_event("discovery", 1440)],
        enabled: true,
        max_bytes: 5_242_880,
    };
    scheduler::generate_jobs(&pool, &[policy], now)
        .await
        .unwrap();

    let jobs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT domain, entity_key, event_type FROM ops.job
          WHERE source_slug = 'tipplap' ORDER BY entity_key",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(jobs.contains(&(
        "markets".to_owned(),
        "markets-5621252".to_owned(),
        "refresh".to_owned()
    )));
    assert!(
        !jobs.iter().any(|job| job.1 == "markets-5599999"),
        "a feloldatlan sor kimarad"
    );
    assert!(
        !jobs
            .iter()
            .any(|job| job.0 == "markets" && (job.1 == "all" || job.1.starts_with("slot-")))
    );
}

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
    let claimed = scheduler::claim(&pool, Uuid::now_v7(), 60, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 2);
    assert_eq!(claimed[0].job.event_type, "discovery");
    assert_eq!(claimed[0].job.priority, 0); // interval: közelség 0, verseny-prioritás 0
    assert_eq!(claimed[0].job.time_window, "2026-11-12");
    assert_eq!(claimed[0].job.due_at, now);
    assert_eq!(claimed[1].job.event_type, "refresh");
    // 10 base + 10 verseny + 1200 közelség (generáláskor 20 óra van a kickoffig).
    assert_eq!(claimed[1].job.priority, 1220);
    assert_eq!(claimed[1].job.entity_key, match_id.to_string());
    assert_eq!(claimed[1].job.time_window, "2026-11-13");
    assert_eq!(claimed[1].job.due_at, kickoff - Duration::minutes(60));
}

#[tokio::test]
async fn subdaily_interval_generates_slot_jobs_with_due_times() {
    let pool = test_pool().await;
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT clock_timestamp()")
        .fetch_one(&pool)
        .await
        .unwrap();
    let policy = policy_with(vec![interval_event("refresh", 240)], true);
    repo::upsert_policy(&pool, &policy).await.unwrap();

    let n = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    assert_eq!(n, 6, "240 → napi 6 frissítési sáv");
    let n = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    assert_eq!(n, 0, "egyedi kulcs: újragenerálva nincs duplikátum");

    let rows = job_rows(&pool).await;
    let keys: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "slot-00", "slot-01", "slot-02", "slot-03", "slot-04", "slot-05"
        ]
    );
    assert!(
        rows.iter()
            .all(|r| r.0 == "refresh" && r.2 == now.date_naive().to_string())
    );
    // priority = refresh base 10 + sáv index.
    assert_eq!(rows[0].3, 10);
    assert_eq!(rows[5].3, 15);

    let midnight = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
    let due_times: Vec<DateTime<Utc>> =
        sqlx::query_scalar("SELECT due_at FROM ops.job ORDER BY due_at")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(due_times.len(), 6);
    assert_eq!(due_times[0], midnight);
    assert!(
        due_times
            .windows(2)
            .all(|pair| pair[1] - pair[0] == Duration::hours(4))
    );
    sqlx::query(
        "UPDATE ops.job SET due_at = CASE
             WHEN entity_key IN ('slot-00','slot-01','slot-02')
                 THEN clock_timestamp() - interval '1 second'
             ELSE clock_timestamp() + interval '1 hour'
         END",
    )
    .execute(&pool)
    .await
    .unwrap();
    let claimed = scheduler::claim(&pool, Uuid::now_v7(), 60, 10)
        .await
        .unwrap();
    assert_eq!(claimed.len(), 3, "csak a már esedékes sávok claimelhetők");
    sqlx::query(
        "UPDATE ops.job SET due_at = clock_timestamp() - interval '1 second'
         WHERE status='pending'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let later = scheduler::claim(&pool, Uuid::now_v7(), 60, 10)
        .await
        .unwrap();
    assert_eq!(later.len(), 3, "a későbbi sávok most esedékesek");
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
    let now = Utc::now().trunc_subsecs(6);
    let policy = policy_with(vec![interval_event("discovery", 1440)], true);
    repo::upsert_policy(&pool, &policy).await.unwrap();
    assert_eq!(
        scheduler::generate_jobs(&pool, &[policy], now)
            .await
            .unwrap(),
        1
    );

    let worker = Uuid::now_v7();
    let claimed = scheduler::claim(&pool, worker, 60, 10).await.unwrap();
    assert_eq!(claimed.len(), 1);
    let job_id = claimed[0].job.id;
    // Lease él: nem jár le, nem sorolódik vissza.
    assert_eq!(scheduler::requeue_expired(&pool).await.unwrap(), 0);
    // Lejárt lease visszasorolása, majd új claim ugyanarra a jobra.
    sqlx::query(
        "UPDATE ops.job SET leased_until = clock_timestamp() - interval '1 second' WHERE id = $1",
    )
    .bind(job_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(scheduler::requeue_expired(&pool).await.unwrap(), 1);
    let again = scheduler::claim(&pool, worker, 60, 10).await.unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].job.id, job_id);
    let attempts: i32 = sqlx::query_scalar("SELECT attempts FROM ops.job WHERE id=$1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(attempts, 2);

    assert!(
        scheduler::complete(&pool, &again[0].lease, JobStatus::Done, None)
            .await
            .unwrap()
    );
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
        scheduler::claim(&pool, worker, 60, 10)
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

/// S45: a research forrás naponta egyszer visszamenőleges refresh jobot kap az
/// elmúlt hét napra (a research projekt 08:00-s köre után frissülő eredmények).
#[tokio::test]
async fn research_policy_enqueues_daily_catchup_refreshes() {
    let pool = test_pool().await;
    let now = base_time();
    let mut policy = policy_with(vec![interval_event("discovery", 1440)], true);
    policy.source_slug = "research".into();
    policy.domains = vec!["day".into()];
    repo::upsert_policy(&pool, &policy).await.unwrap();

    let n1 = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    // 1 napi discovery + 7 catch-up refresh
    assert_eq!(n1, 8);

    let catchups: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.job
         WHERE source_slug='research' AND event_type='refresh'
           AND entity_key LIKE 'catchup-%' AND time_window=$1",
    )
    .bind(now.date_naive().to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(catchups, 7);

    // S136: a catch-up csak a research frissítés után futhat — a due_at legyen
    // a generálás időpontjánál későbbi, különben elavult napi fájlt olvas.
    let earliest_due: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
        "SELECT min(due_at) FROM ops.job
         WHERE source_slug='research' AND entity_key LIKE 'catchup-%' AND time_window=$1",
    )
    .bind(now.date_naive().to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        earliest_due > now,
        "a catch-up túl korán futna: {earliest_due}"
    );

    // Ugyanazon a napon belül nem duplikálódik.
    let n2 = scheduler::generate_jobs(&pool, std::slice::from_ref(&policy), now)
        .await
        .unwrap();
    assert_eq!(n2, 0);
}
