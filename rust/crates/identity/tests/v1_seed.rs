//! v1 crosswalk-seed integráció: a seeded aliasok a linkert pontos
//! (reviewed_alias) feloldásra vezetik, ékezet nélküli forrás-névvel is.

use football_domain::identity::{IdentityContext, Resolution, ResolveMethod};
use football_identity::resolve;
use football_storage::repositories::identity as repo;
use football_storage::test_support::test_pool;
use sqlx::PgPool;
use uuid::Uuid;

async fn seed_country(pool: &PgPool, code: &str, name: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.country (id, code, name) VALUES ($1,$2,$3)")
        .bind(id)
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    id
}

async fn seed_team(pool: &PgPool, name: &str, country_id: Uuid) -> Uuid {
    let entity_id = Uuid::now_v7();
    sqlx::query("INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'team',$2)")
        .bind(entity_id)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO core.team (id, entity_id, country_id, name, gender, age_group, branch)
         VALUES ($1,$2,$3,$4,'male','senior','senior')",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .bind(country_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
    entity_id
}

async fn seed_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,'Demó')")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn seeded_alias_resolves_accentless_source_name() {
    let pool = test_pool().await;
    let hu = seed_country(&pool, "HU", "Hungary").await;
    let entity_id = seed_team(&pool, "Ferencváros", hu).await;
    let source_id = seed_source(&pool, "tipplap").await;

    let v2 = repo::all_team_entities(&pool).await.unwrap();
    let v1 = vec![football_identity::v1_seed::V1Team {
        names: vec![
            ("Ferencváros".to_string(), "en".to_string()),
            ("Ferencvárosi TC".to_string(), "hu".to_string()),
        ],
        country_code: Some("HU".to_string()),
        gender: Some("male".to_string()),
    }];
    let (seeds, stats) = football_identity::v1_seed::plan_seeds(&v2, &v1);
    assert_eq!(stats.matched, 1);
    assert!(seeds.iter().any(|s| s.alias == "ferencvaros"));

    let inserted = repo::insert_alias_seeds(&pool, &seeds).await.unwrap();
    assert_eq!(inserted, seeds.len());
    // Második futás: nincs új sor.
    let again = repo::insert_alias_seeds(&pool, &seeds).await.unwrap();
    assert_eq!(again, 0);

    let ctx = IdentityContext {
        country_code: Some("HU".into()),
        gender: Some("male".into()),
        age_group: Some("senior".into()),
        branch: Some("senior".into()),
    };
    let resolution = resolve(
        &pool,
        source_id,
        "tipplap",
        "team",
        "Ferencvaros",
        "Ferencvaros",
        &ctx,
    )
    .await
    .unwrap();
    match resolution {
        Resolution::Exact {
            entity_id: hit,
            method,
        } => {
            assert_eq!(hit, entity_id);
            assert_eq!(method, ResolveMethod::ReviewedAlias);
        }
        other => panic!("várt reviewed_alias feloldás, kapott: {other:?}"),
    }
}
