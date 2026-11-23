//! Globális keresés: ékezet-inszenzitív találatok csapatra, versenyre és meccsre.

use football_storage::{repositories::search::search, test_support::test_pool};
use uuid::Uuid;

async fn seed_entity(pool: &sqlx::PgPool, kind: &str, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(kind)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn search_is_accent_insensitive_across_teams_competitions_and_matches() {
    let pool = test_pool().await;

    let country_id: Uuid = sqlx::query_scalar(
        "INSERT INTO core.country (id, code, name) VALUES ($1,'HU','Magyarország')
         ON CONFLICT (code) DO UPDATE SET name=EXCLUDED.name RETURNING id",
    )
    .bind(Uuid::now_v7())
    .fetch_one(&pool)
    .await
    .unwrap();

    let home_entity = seed_entity(&pool, "team", "Újpest").await;
    let away_entity = seed_entity(&pool, "team", "Győr").await;
    let home_team_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,'Újpest','male','senior','senior')",
    )
    .bind(home_team_id)
    .bind(home_entity)
    .bind(country_id)
    .execute(&pool)
    .await
    .unwrap();
    let away_team_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,'Győr','male','senior','senior')",
    )
    .bind(away_team_id)
    .bind(away_entity)
    .bind(country_id)
    .execute(&pool)
    .await
    .unwrap();

    let competition_id = Uuid::now_v7();
    let competition_entity = seed_entity(&pool, "competition", "Magyar Kupa").await;
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1,$2,$3,'Magyar Kupa',1,6)",
    )
    .bind(competition_id)
    .bind(competition_entity)
    .bind(country_id)
    .execute(&pool)
    .await
    .unwrap();

    let season_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.season (id, competition_id, label) VALUES ($1,$2,'2026/27')")
        .bind(season_id)
        .bind(competition_id)
        .execute(&pool)
        .await
        .unwrap();

    let kickoff = chrono::Utc::now() - chrono::Duration::hours(20);
    sqlx::query(
        "INSERT INTO core.match
           (id, competition_id, season_id, home_team_id, away_team_id, kickoff_at,
            competition_timezone, status)
         VALUES ($1,$2,$3,$4,$5,$6,'Europe/Budapest','finished')",
    )
    .bind(Uuid::now_v7())
    .bind(competition_id)
    .bind(season_id)
    .bind(home_team_id)
    .bind(away_team_id)
    .bind(kickoff)
    .execute(&pool)
    .await
    .unwrap();

    let player_id = seed_entity(&pool, "player", "Játékos Kereső").await;
    sqlx::query("INSERT INTO core.player (entity_id, country_id) VALUES ($1,$2)")
        .bind(player_id)
        .bind(country_id)
        .execute(&pool)
        .await
        .unwrap();

    // Ékezet nélküli keresőszó megtalálja az ékezetes nevet.
    let hits = search(&pool, "ujpest", 8).await.unwrap();
    assert!(hits.iter().any(|h| h.kind == "team" && h.title == "Újpest"));
    assert!(
        hits.iter()
            .any(|h| h.kind == "match" && h.title.contains("Újpest")),
        "a meccs találat a csapatnévből jön: {hits:?}"
    );

    let hits = search(&pool, "jatekos", 8).await.unwrap();
    assert!(
        hits.iter()
            .any(|h| h.kind == "player" && h.title == "Játékos Kereső"),
        "a játékos is találat: {hits:?}"
    );

    let hits = search(&pool, "magyar", 8).await.unwrap();
    assert!(
        hits.iter()
            .any(|h| h.kind == "competition" && h.title == "Magyar Kupa")
    );

    let hits = search(&pool, "nem-this", 8).await.unwrap();
    assert!(hits.is_empty(), "nincs találat: {hits:?}");
}
