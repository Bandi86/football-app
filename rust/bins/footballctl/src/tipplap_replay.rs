//! Tipplap replay: a már eltárolt nyers HTML újrafeldolgozása az aktuális parserrel.
//!
//! Nincs hálózati kérés, ezért a robots/policy, a napi kvóta és a breaker
//! érintetlen (AGENTS.md §7). A dátum-guardot nem kerüljük meg: a tárolt oldal
//! saját `select#dates option[selected]` értékét ellenőrzi a parser.

use std::path::Path;

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use football_ingest::adapter::{ParseCtx, SourceAdapter};
use football_ingest::fetch::RawPayload;
use football_ingest::normalized::NormalizedDay;
use football_ingest::observe::insert_observation;
use football_ingest::tipplap::TipplapAdapter;
use football_storage::object_store::{ObjectStore, StoredArtifact};
use football_worker::tick::verify_and_publish;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow)]
struct CaptureRow {
    id: Uuid,
    request_key: String,
    requested_at: DateTime<Utc>,
    content_sha256: String,
    compression: String,
    byte_size: i64,
    storage_key: String,
}

#[derive(Debug, Default)]
pub struct ReplayReport {
    pub days: usize,
    pub parsed: usize,
    pub inserted_new: usize,
    pub materialized_competitions: usize,
    pub skipped: Vec<String>,
    pub applied: bool,
}

pub fn request_key(date: NaiveDate) -> String {
    format!("tipplap:day:{date}")
}

/// Egy nap legfrissebb sikeres tipplap-capture-je.
async fn latest_capture(
    pool: &PgPool,
    source_id: Uuid,
    date: NaiveDate,
) -> Result<Option<CaptureRow>> {
    sqlx::query_as::<_, CaptureRow>(
        "SELECT c.id, c.request_key, c.requested_at, a.content_sha256,
                a.compression, a.byte_size, a.storage_key
           FROM raw.capture c
           JOIN raw.artifact a ON a.id = c.artifact_id
          WHERE c.source_id = $1 AND c.request_key = $2
            AND c.http_status = 200 AND c.error_code IS NULL
          ORDER BY c.requested_at DESC, c.id DESC
          LIMIT 1",
    )
    .bind(source_id)
    .bind(request_key(date))
    .fetch_optional(pool)
    .await
    .context("tipplap capture lekérése")
}

pub async fn run(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
    apply: bool,
    data_dir: &Path,
) -> Result<ReplayReport> {
    let source_id: Uuid = sqlx::query_scalar("SELECT id FROM ops.source WHERE code='tipplap'")
        .fetch_one(pool)
        .await
        .context("tipplap forrás hiányzik")?;
    let store = ObjectStore::new(data_dir).context("objektumtár")?;
    let adapter = TipplapAdapter;
    let mut report = ReplayReport {
        applied: apply,
        ..Default::default()
    };
    let mut date = from;
    loop {
        report.days += 1;
        match latest_capture(pool, source_id, date).await? {
            None => report
                .skipped
                .push(format!("{date}: nincs sikeres capture")),
            Some(capture) => {
                let artifact = StoredArtifact {
                    sha256: capture.content_sha256.clone(),
                    storage_key: capture.storage_key.clone(),
                    byte_size: capture.byte_size as u64,
                    compression: if capture.compression == "zstd" {
                        "zstd"
                    } else {
                        "none"
                    },
                };
                let bytes = store
                    .get(&artifact)
                    .with_context(|| format!("nyers HTML betöltése: {}", capture.storage_key))?;
                let raw = RawPayload {
                    bytes,
                    content_type: "text/html".into(),
                    http_status: 200,
                    fetched_at: capture.requested_at,
                    sha256: capture.content_sha256.clone(),
                };
                let ctx = ParseCtx {
                    requested_date: date,
                    request_key: capture.request_key.clone(),
                };
                match adapter.parse(&raw, &ctx) {
                    Ok(observations) => {
                        report.parsed += observations.len();
                        if apply && !observations.is_empty() {
                            let before: i64 = sqlx::query_scalar(
                                "SELECT count(*) FROM stage.observation WHERE source_id=$1",
                            )
                            .bind(source_id)
                            .fetch_one(pool)
                            .await?;
                            for obs in &observations {
                                insert_observation(
                                    pool,
                                    source_id,
                                    capture.id,
                                    &obs.entity_kind,
                                    &obs.aspect,
                                    &obs.natural_key,
                                    &obs.source_record_key,
                                    obs.observed_at,
                                    obs.payload.clone(),
                                )
                                .await
                                .with_context(|| {
                                    format!("observation írása: {date} / {}", obs.aspect)
                                })?;
                            }
                            let after: i64 = sqlx::query_scalar(
                                "SELECT count(*) FROM stage.observation WHERE source_id=$1",
                            )
                            .bind(source_id)
                            .fetch_one(pool)
                            .await?;
                            report.inserted_new += (after - before).max(0) as usize;

                            let days = NormalizedDay::grouped_from_observations(&observations)
                                .context("observationök normalizálása")?;
                            for day in &days {
                                verify_and_publish(pool, day)
                                    .await
                                    .with_context(|| format!("kanonikus materializáció: {date}"))?;
                            }
                            report.materialized_competitions += days.len();
                        }
                    }
                    Err(err) => report.skipped.push(format!("{date}: {err}")),
                }
            }
        }
        if date == to {
            break;
        }
        let Some(next) = date.succ_opt() else { break };
        date = next;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_storage::test_support::test_pool;
    use sqlx::postgres::PgPoolOptions;

    #[test]
    fn request_key_is_stable() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 21).unwrap();
        assert_eq!(request_key(date), "tipplap:day:2026-11-21");
    }

    #[tokio::test]
    async fn latest_capture_breaks_requested_at_ties_by_capture_id() {
        let setup_pool = test_pool().await;
        setup_pool.close().await;
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();

        // Az index jelenleg elfedheti a hiányzó tie-breaket; a szerződés
        // ellenőrzéséhez a teszt soros táblavizsgálatot kényszerít ki.
        sqlx::query("SET enable_indexscan = off")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("SET enable_bitmapscan = off")
            .execute(&pool)
            .await
            .unwrap();

        let source_id = Uuid::from_u128(100);
        let artifact_id = Uuid::from_u128(101);
        let older_capture_id = Uuid::from_u128(102);
        let newer_capture_id = Uuid::from_u128(103);
        let captured_at = DateTime::parse_from_rfc3339("2026-11-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        sqlx::query(
            "INSERT INTO ops.source (id, code, display_name) VALUES ($1,'tipplap','Tipplap')",
        )
        .bind(source_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO raw.artifact
             (id, content_sha256, compression, byte_size, storage_key, media_type)
             VALUES ($1,$2,'none',4,'replay/tie.html','text/html')",
        )
        .bind(artifact_id)
        .bind("a".repeat(64))
        .execute(&pool)
        .await
        .unwrap();
        for capture_id in [newer_capture_id, older_capture_id] {
            sqlx::query(
                "INSERT INTO raw.capture
                 (id, source_id, artifact_id, endpoint_key, request_url, requested_at,
                  received_at, http_status, adapter_version, parser_version, request_key)
                 VALUES ($1,$2,$3,'day','https://example.invalid/tipplap',$4,$4,200,'test','test',
                         'tipplap:day:2026-11-21')",
            )
            .bind(capture_id)
            .bind(source_id)
            .bind(artifact_id)
            .bind(captured_at)
            .execute(&pool)
            .await
            .unwrap();
        }

        let selected = latest_capture(
            &pool,
            source_id,
            NaiveDate::from_ymd_opt(2026, 11, 21).unwrap(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(selected.id, newer_capture_id);
    }
}
