//! Napi integritás-riport (S25): egyetlen, read-only pillanatkép azokról
//! a mutatókról, amelyek a napi ingest után ellenőrzendők. A CLI ezt
//! `footballctl integrity` néven teszi elérhetővé; a parancs nem ír.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IntegrityReport {
    /// Ugyanaz a verseny+szezon+csapat-pár+pontos kickoff több sorban.
    pub duplicate_kickoff_groups: i64,
    /// Ugyanaz a verseny+szezon+csapat-pár+UTC-nap több sorban.
    pub duplicate_fixture_groups: i64,
    /// Ugyanaz a csapatpár ugyanazon az UTC-napon több sorban — akkor is, ha a
    /// két forrás eltérő csapat-entitást használt (cross-source névváltozat),
    /// ezért a `duplicate_fixture_groups` id-alapú kulcsa nem látja. Egy klub
    /// egy napon legfeljebb egyszer játszhat, ezért ez review-jelzés.
    pub same_pair_same_day_groups: i64,
    /// Provenance sor, amelyhez nincs kanonikus meccs.
    pub orphan_provenance: i64,
    /// Aktív forrás-ref olyan entitásra, amelyet összevontak.
    pub active_refs_to_merged: i64,
    /// Függő identitás-review (blokkolhatja az új megfigyeléseket).
    pub pending_reviews: i64,
    /// Withheld publish gate ( valódi adathiány vagy konfliktus).
    pub withheld_gates: i64,
    /// Failed job a vizsgált időablakban.
    pub failed_jobs: i64,
    /// Nyitott breaker.
    pub open_breakers: i64,
}

impl IntegrityReport {
    /// A kemény integritási hibák: duplikátum, árva bizonyíték, összevont
    /// entitásra mutató aktív ref. Ezek >0 esetén a napi riport FAIL.
    pub fn has_hard_failures(&self) -> bool {
        self.duplicate_kickoff_groups > 0
            || self.duplicate_fixture_groups > 0
            || self.orphan_provenance > 0
            || self.active_refs_to_merged > 0
    }
}

/// Read-only integritás-pillanatkép. A dátum-szűrés csak a duplikátum-
/// csoportokra és a failed job-okra vonatkozik; a többi mutató teljes DB-re
/// szól (a review- és gate-állomány nem dátumhoz kötött).
pub async fn report(
    pool: &PgPool,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<IntegrityReport> {
    let dup_kickoff: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
            SELECT 1 FROM core.match
             WHERE ($1::date IS NULL OR kickoff_at::date >= $1)
               AND ($2::date IS NULL OR kickoff_at::date <= $2)
             GROUP BY competition_id, season_id,
                      LEAST(home_team_id, away_team_id),
                      GREATEST(home_team_id, away_team_id), kickoff_at
            HAVING count(*) > 1
         ) x",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
    .context("pontos kickoff duplikátumok számlálása")?;

    let dup_fixture: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
            SELECT 1 FROM core.match
             WHERE ($1::date IS NULL OR kickoff_at::date >= $1)
               AND ($2::date IS NULL OR kickoff_at::date <= $2)
             GROUP BY competition_id, season_id,
                      LEAST(home_team_id, away_team_id),
                      GREATEST(home_team_id, away_team_id), kickoff_at::date
            HAVING count(*) > 1
         ) x",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
    .context("fixture duplikátumok számlálása")?;

    let orphan_prov: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM fact.provenance p
          WHERE p.subject_kind='match'
            AND NOT EXISTS (SELECT 1 FROM core.match m WHERE m.id=p.subject_id)",
    )
    .fetch_one(pool)
    .await
    .context("árva provenance számlálása")?;

    let merged_refs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM identity.source_entity_ref r
          JOIN core.entity e ON e.id=r.entity_id
          WHERE r.status='active' AND e.merged_into_id IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .context("összevont entitásra mutató aktív refek számlálása")?;

    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM identity.review_queue WHERE status='pending'")
            .fetch_one(pool)
            .await?;

    let withheld: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.publish_gate WHERE state='withheld'")
            .fetch_one(pool)
            .await?;

    let failed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ops.job
          WHERE status='failed'
            AND ($1::date IS NULL OR time_window >= $1::text)
            AND ($2::date IS NULL OR time_window <= $2::text)",
    )
    .bind(from.map(|d| d.to_string()))
    .bind(to.map(|d| d.to_string()))
    .fetch_one(pool)
    .await?;

    let open_breakers: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ops.breaker_state WHERE state <> 'closed'")
            .fetch_one(pool)
            .await?;

    // S97: cross-source névváltozat duplikátum — a két sor külön csapat-entitást
    // használ (ezért az id-alapú kulcs nem látja), de a foldolt nevek és a nap
    // ugyanaz. Egy klub egy napon egyszer játszhat, ezért ez review-jelzés.
    let same_pair_same_day: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (
            SELECT 1
              FROM core.match m
              JOIN core.team ht ON ht.id = m.home_team_id
              JOIN core.entity he ON he.id = ht.entity_id
              JOIN core.team at ON at.id = m.away_team_id
              JOIN core.entity ae ON ae.id = at.entity_id
             WHERE ($1::date IS NULL OR m.kickoff_at::date >= $1)
               AND ($2::date IS NULL OR m.kickoff_at::date <= $2)
             GROUP BY identity.fold_name(he.canonical_name),
                      identity.fold_name(ae.canonical_name),
                      m.kickoff_at::date
            HAVING count(*) > 1
         ) x",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
    .context("azonos csapatpár azonos napon duplikátumok számlálása")?;

    Ok(IntegrityReport {
        duplicate_kickoff_groups: dup_kickoff,
        duplicate_fixture_groups: dup_fixture,
        same_pair_same_day_groups: same_pair_same_day,
        orphan_provenance: orphan_prov,
        active_refs_to_merged: merged_refs,
        pending_reviews: pending,
        withheld_gates: withheld,
        failed_jobs: failed,
        open_breakers,
    })
}
