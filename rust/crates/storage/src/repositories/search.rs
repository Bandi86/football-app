//! Globális keresés a kanonikus entitásokra (fázis-2 kilépési feltétel).
//!
//! Ékezet-inszenzitív: az SQL `translate()` a magyar ékezetes magánhangzókat
//! az alapbetűkre képezi le mind a mezőn, mind a keresőszón — ezért nincs
//! szükség `unaccent` kiterjesztésre, és a viselkedés determinisztikus.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use utoipa::ToSchema;
use uuid::Uuid;

const FOLD_FROM: &str = "áéíóöőúüűÁÉÍÓÖŐÚÜŰ";
const FOLD_TO: &str = "aeiooouuuaeiooouuu";

pub fn fold_expression(column: &str) -> String {
    format!("translate(lower({column}), '{FOLD_FROM}', '{FOLD_TO}')")
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SearchHit {
    /// `team` | `competition` | `match`.
    pub kind: String,
    pub id: Uuid,
    pub title: String,
    pub subtitle: Option<String>,
}

/// Találatok csoportonként korlátozva; a sorrend team → verseny → meccs.
/// A meccsek csak az utóbbi 7 napból jönnek (költségkorlát, dokumentált).
pub async fn search(pool: &PgPool, query: &str, per_kind: i64) -> Result<Vec<SearchHit>> {
    let team_fold = fold_expression("t.name");
    let competition_fold = fold_expression("comp.name");
    let home_fold = fold_expression("th.name");
    let away_fold = fold_expression("ta.name");
    let round_fold = fold_expression("coalesce(m.round_label, '')");
    let query_fold = format!("translate(lower($1), '{FOLD_FROM}', '{FOLD_TO}')");

    let teams = sqlx::query_as::<_, SearchHit>(&format!(
        "SELECT 'team'::text AS kind, t.id, t.name AS title, c.name AS subtitle
               FROM core.team t
               LEFT JOIN core.country c ON c.id = t.country_id
              WHERE {team_fold} LIKE '%' || {query_fold} || '%'
              ORDER BY t.name LIMIT $2"
    ))
    .bind(query)
    .bind(per_kind)
    .fetch_all(pool)
    .await?;

    let competitions = sqlx::query_as::<_, SearchHit>(&format!(
        "SELECT 'competition'::text AS kind, comp.id, comp.name AS title,
                    c.code AS subtitle
               FROM core.competition comp
               LEFT JOIN core.country c ON c.id = comp.country_id
              WHERE {competition_fold} LIKE '%' || {query_fold} || '%'
              ORDER BY comp.priority DESC, comp.name LIMIT $2"
    ))
    .bind(query)
    .bind(per_kind)
    .fetch_all(pool)
    .await?;

    let players = sqlx::query_as::<_, SearchHit>(&format!(
        "SELECT 'player'::text AS kind, e.id, e.canonical_name AS title,
                c.code AS subtitle
           FROM core.entity e
           JOIN core.player p ON p.entity_id = e.id
           LEFT JOIN core.country c ON c.id = p.country_id
          WHERE e.kind = 'player' AND e.merged_into_id IS NULL
            AND {player_fold} LIKE '%' || {query_fold} || '%'
          ORDER BY e.canonical_name LIMIT $2",
        player_fold = fold_expression("e.canonical_name"),
    ))
    .bind(query)
    .bind(per_kind)
    .fetch_all(pool)
    .await?;

    let matches = sqlx::query_as::<_, SearchHit>(&format!(
        "SELECT 'match'::text AS kind, m.id,
                    (th.name || ' – ' || ta.name) AS title,
                    to_char(m.kickoff_at AT TIME ZONE 'Europe/Budapest',
                            'YYYY-MM-DD HH24:MI') AS subtitle
               FROM core.match m
               JOIN core.team th ON th.id = m.home_team_id
               JOIN core.team ta ON ta.id = m.away_team_id
               JOIN core.competition comp ON comp.id = m.competition_id
              WHERE m.kickoff_at >= now() - interval '7 days'
                AND ({home_fold} LIKE '%' || {query_fold} || '%'
                     OR {away_fold} LIKE '%' || {query_fold} || '%'
                     OR {round_fold} LIKE '%' || {query_fold} || '%'
                     -- S122: verseny nev alapjan is (pl. indonesia ->
                     -- League Cup Indonesia meccsei).
                     OR {competition_fold} LIKE '%' || {query_fold} || '%')
              ORDER BY m.kickoff_at DESC LIMIT $2"
    ))
    .bind(query)
    .bind(per_kind)
    .fetch_all(pool)
    .await?;

    let mut all = teams;
    all.extend(competitions);
    all.extend(matches);
    all.extend(players);
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_expression_is_deterministic() {
        let expr = fold_expression("t.name");
        assert!(expr.contains("translate(lower(t.name)"));
        assert_eq!(FOLD_FROM.chars().count(), FOLD_TO.chars().count());
    }
}
