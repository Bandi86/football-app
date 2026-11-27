//! S216: feloldatlan cross-source duplikátumok rendezése.
//!
//! Csak a `date_only` placeholder sorokat állítja `canceled`re, a pontos
//! kezdésű sort meghagyja. Nem töröl, nem fuzzy-merge-el: minden változás
//! `ops.audit_event`ben rögzített és visszaállítható. A CLI alapból dry-run.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateResolution {
    pub loser_id: Uuid,
    pub winner_id: Uuid,
}

type RawRow = (Uuid, Uuid, Uuid, NaiveDate, String, String, Uuid);

/// A rendezendő párok: a vesztes mindig `date_only` placeholder, a győztes
/// a pontos kezdésű sor (több ilyen esetén a legalacsonyabb UUID).
/// Ugyanaz a determinisztikus szabály, mint az integritási kapunál S215-ben.
pub async fn candidates(
    pool: &PgPool,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<DuplicateResolution>> {
    let rows: Vec<RawRow> = sqlx::query_as(
        "SELECT m.id, m.home_team_id, m.away_team_id, m.kickoff_at::date,
                m.kickoff_precision, m.status, m.competition_id
           FROM core.match m
          WHERE ($1::date IS NULL OR m.kickoff_at::date >= $1)
            AND ($2::date IS NULL OR m.kickoff_at::date <= $2)",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .context("duplikátum-jelöltek lekérése")?;

    use std::collections::HashMap;
    let mut groups: HashMap<(Uuid, Uuid, NaiveDate), Vec<&RawRow>> = HashMap::new();
    for row in &rows {
        let key = if row.1 <= row.2 {
            (row.1, row.2, row.3)
        } else {
            (row.2, row.1, row.3)
        };
        groups.entry(key).or_default().push(row);
    }

    let mut result = Vec::new();
    for members in groups.values() {
        if members.len() < 2 || !members.iter().any(|r| r.4 == "date_only") {
            continue;
        }
        let competitions: std::collections::HashSet<Uuid> = members.iter().map(|r| r.6).collect();
        if competitions.len() < 2 {
            continue;
        }
        let winner = members
            .iter()
            .filter(|r| r.4 == "exact")
            .map(|r| r.0)
            .min()
            .or_else(|| members.iter().map(|r| r.0).min())
            .expect("nem üres csoport");
        for row in members
            .iter()
            // Csak a placeholder sorokat állítjuk le; a két `exact` sor
            // közül egyik sem érintett (az API úgyis egyet jelenít meg).
            .filter(|r| r.4 == "date_only" && r.5 != "canceled")
        {
            result.push(DuplicateResolution {
                loser_id: row.0,
                winner_id: winner,
            });
        }
    }
    result.sort_by_key(|r| (r.winner_id, r.loser_id));
    Ok(result)
}

/// A rendezés végrehajtása egy tranzakcióban. Idempotens: második futás 0 sort ír.
pub async fn apply(
    pool: &PgPool,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<(usize, usize)> {
    let candidates = candidates(pool, from, to).await?;
    let affected = candidates.len();
    let mut tx = pool.begin().await?;
    for resolution in &candidates {
        let updated = sqlx::query(
            "UPDATE core.match SET status='canceled', updated_at=now()
              WHERE id=$1 AND status<>'canceled'",
        )
        .bind(resolution.loser_id)
        .execute(&mut *tx)
        .await
        .context("duplikátum sor canceledre állítása")?;
        if updated.rows_affected() == 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO ops.audit_event
             (id, actor, action, subject_kind, subject_id, detail, created_at)
             VALUES ($1,'cli','duplicate_fixture_cancel','match',$2,$3,now())",
        )
        .bind(Uuid::now_v7())
        .bind(resolution.loser_id)
        .bind(json!({
            "reason": "S216: date_only placeholder duplikátum; a pontos kezdésű sor a kanonikus",
            "winner_id": resolution.winner_id,
            "policy": "deterministic-id-rule",
        }))
        .execute(&mut *tx)
        .await
        .context("duplikátum-rendezés audit eventje")?;
    }
    tx.commit().await?;
    let groups = candidates
        .iter()
        .map(|r| r.winner_id)
        .collect::<std::collections::HashSet<_>>()
        .len();
    Ok((groups, affected))
}
