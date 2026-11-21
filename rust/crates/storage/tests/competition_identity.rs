//! S81: verseny find-or-create identitás-tesztek (football_v2_test_comp).

use football_storage::repositories::competition_identity::{
    CompetitionIdentity, CompetitionMatch, find_or_create_competition,
};
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

async fn seed_source(pool: &PgPool, code: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query("INSERT INTO ops.source (id, code, display_name) VALUES ($1,$2,$2)")
        .bind(id)
        .bind(code)
        .execute(pool)
        .await
        .unwrap();
    id
}

fn input<'a>(
    name: &'a str,
    country_id: Option<Uuid>,
    source_id: Option<Uuid>,
    source_entity_id: Option<&'a str>,
) -> CompetitionIdentity<'a> {
    CompetitionIdentity {
        id_hint: None,
        name,
        country_id,
        country_code: country_id.map(|_| "ES"),
        tier: Some(1),
        priority: Some(10),
        source_id,
        source_entity_id,
    }
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

/// (a) Ugyanaz a verseny két írásmóddal és ország-kontextussal nem hoz létre
/// két sort; létrehozáskor alias és source-ref íródik.
#[tokio::test]
async fn same_competition_two_spellings_creates_one_row() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "ES", "Spain").await;
    let source = seed_source(&pool, "test-comp-ident").await;

    let mut conn = pool.acquire().await.unwrap();
    let first = find_or_create_competition(
        &mut conn,
        &input(
            "Primera División",
            Some(country),
            Some(source),
            Some("ext-comp-1"),
        ),
    )
    .await
    .unwrap();
    assert!(first.created);
    assert_eq!(first.matched_by, CompetitionMatch::Created);
    assert!(first.alias_written);
    assert!(first.ref_written);

    // A második írásmód (ékezet nélkül) az alias-fold révén ugyanaz a sor.
    let second = find_or_create_competition(
        &mut conn,
        &input(
            "Primera Division",
            Some(country),
            Some(source),
            Some("ext-comp-2"),
        ),
    )
    .await
    .unwrap();
    assert!(!second.created);
    assert_eq!(second.competition_id, first.competition_id);
    assert_eq!(second.matched_by, CompetitionMatch::Alias);

    // Ismeretlen ország = semleges wildcard: az alias továbbra is talál.
    let third = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: None,
            name: "primera división",
            country_id: None,
            country_code: None,
            tier: None,
            priority: None,
            source_id: Some(source),
            source_entity_id: Some("ext-comp-3"),
        },
    )
    .await
    .unwrap();
    assert_eq!(third.competition_id, first.competition_id);

    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
    assert_eq!(
        count(
            &pool,
            "SELECT count(*) FROM core.entity WHERE kind='competition'"
        )
        .await,
        1
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM identity.alias").await, 1);

    let (alias, lang, country_code, status, method): (
        String,
        String,
        Option<String>,
        String,
        String,
    ) = sqlx::query_as("SELECT alias, lang, country_code, status, method FROM identity.alias")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(alias, "Primera División");
    assert_eq!(lang, "und");
    assert_eq!(country_code.as_deref(), Some("ES"));
    assert_eq!(status, "reviewed");
    assert_eq!(method, "import");

    assert_eq!(
        count(&pool, "SELECT count(*) FROM identity.source_entity_ref").await,
        3
    );
}

/// A pontos source_entity_ref lookup megelőzi a név-alapú egyeztetést.
#[tokio::test]
async fn exact_source_ref_wins_over_name_lookup() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "ES", "Spain").await;
    let source = seed_source(&pool, "test-comp-ref").await;

    let entity = Uuid::now_v7();
    let competition = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','Segunda División')",
    )
    .bind(entity)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'Segunda División')",
    )
    .bind(competition)
    .bind(entity)
    .bind(country)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO identity.source_entity_ref
             (id, source_id, entity_kind, source_entity_id, entity_id, method, confidence, status)
         VALUES ($1,$2,'competition','ext-ref',$3,'exact_external_id',1.0,'active')",
    )
    .bind(Uuid::now_v7())
    .bind(source)
    .bind(entity)
    .execute(&pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let resolved = find_or_create_competition(
        &mut conn,
        &input(
            "Teljesen Más Név",
            Some(country),
            Some(source),
            Some("ext-ref"),
        ),
    )
    .await
    .unwrap();

    assert_eq!(resolved.matched_by, CompetitionMatch::SourceRef);
    assert!(!resolved.created);
    assert_eq!(resolved.competition_id, competition);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1,
        "a source-ref találat nem hoz létre második versenyt"
    );
    let name: String = sqlx::query_scalar("SELECT name FROM core.competition WHERE id=$1")
        .bind(competition)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name, "Segunda División", "a kanonikus nevet nem írjuk át");
}

/// Az id-hint csak lookup után számít: létező sorra mutatva nem hoz létre újat.
#[tokio::test]
async fn id_hint_is_looked_up_before_create() {
    let pool = test_pool().await;
    let hint = Uuid::now_v7();

    let mut conn = pool.acquire().await.unwrap();
    let created = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: Some(hint),
            name: "Tercera División",
            country_id: None,
            country_code: None,
            tier: None,
            priority: None,
            source_id: None,
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert!(created.created);
    assert_eq!(created.competition_id, hint, "a hint adja az új sor id-jét");

    let again = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: Some(hint),
            name: "Teljesen Eltérő Név",
            country_id: None,
            country_code: None,
            tier: None,
            priority: None,
            source_id: None,
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert!(!again.created);
    assert_eq!(again.matched_by, CompetitionMatch::IdHint);
    assert_eq!(again.competition_id, hint);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
}

/// Ismeretlen országú (NULL) versenyhez NULL országú alias íródik, és a
/// későbbi ismert ország-kontextus is megtalálja (wildcard).
#[tokio::test]
async fn unknown_country_is_neutral_wildcard() {
    let pool = test_pool().await;
    let country = seed_country(&pool, "ES", "Spain").await;
    let source = seed_source(&pool, "test-comp-wildcard").await;

    let mut conn = pool.acquire().await.unwrap();
    let first = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: None,
            name: "Copa Neutral",
            country_id: None,
            country_code: None,
            tier: None,
            priority: None,
            source_id: Some(source),
            source_entity_id: Some("ext-neutral"),
        },
    )
    .await
    .unwrap();
    assert!(first.created);

    let alias_country: Option<String> =
        sqlx::query_scalar("SELECT country_code FROM identity.alias WHERE entity_id=$1")
            .bind(first.competition_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(alias_country, None, "semleges ország → NULL alias-ország");

    let second = find_or_create_competition(
        &mut conn,
        &input("copa neutral", Some(country), Some(source), Some("ext-es")),
    )
    .await
    .unwrap();
    assert_eq!(second.competition_id, first.competition_id);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );
}

/// Üres/használhatatlan név tipizált hibát ad, nulla írással.
#[tokio::test]
async fn empty_name_is_rejected_without_writes() {
    let pool = test_pool().await;
    let mut conn = pool.acquire().await.unwrap();
    let result = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: None,
            name: "   ",
            country_id: None,
            country_code: None,
            tier: None,
            priority: None,
            source_id: None,
            source_entity_id: None,
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        0
    );
    assert_eq!(count(&pool, "SELECT count(*) FROM core.entity").await, 0);
}

/// S81: alias nélküli, korábban létrehozott versenynél az entitás kanonikus
/// neve is egyeztet, mielőtt új sort hoznánk létre; más országra nem hat.
#[tokio::test]
async fn canonical_name_lookup_closes_alias_gap() {
    let pool = test_pool().await;
    let cl = seed_country(&pool, "CL", "Chile").await;
    let ar = seed_country(&pool, "AR", "Argentina").await;
    let source = seed_source(&pool, "test-comp-canonical").await;

    let entity = Uuid::now_v7();
    let competition = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name) VALUES ($1,'competition','La Liga')",
    )
    .bind(entity)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name)
         VALUES ($1,$2,$3,'Liga de Primera')",
    )
    .bind(competition)
    .bind(entity)
    .bind(cl)
    .execute(&pool)
    .await
    .unwrap();

    let mut conn = pool.acquire().await.unwrap();
    let resolved = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: None,
            name: "La Liga",
            country_id: Some(cl),
            country_code: Some("CL"),
            tier: None,
            priority: None,
            source_id: Some(source),
            source_entity_id: Some("ext-canonical-cl"),
        },
    )
    .await
    .unwrap();
    assert!(!resolved.created);
    assert_eq!(resolved.competition_id, competition);
    assert_eq!(resolved.matched_by, CompetitionMatch::CanonicalName);
    assert!(resolved.alias_written);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        1
    );

    // Ugyanaz a név más országban új verseny (az ország-szűrő szigorú).
    let other = find_or_create_competition(
        &mut conn,
        &CompetitionIdentity {
            id_hint: None,
            name: "La Liga",
            country_id: Some(ar),
            country_code: Some("AR"),
            tier: None,
            priority: None,
            source_id: Some(source),
            source_entity_id: Some("ext-canonical-ar"),
        },
    )
    .await
    .unwrap();
    assert!(other.created);
    assert_eq!(
        count(&pool, "SELECT count(*) FROM core.competition").await,
        2
    );
}

/// S84: az ingest nem írhatja felül a javított verseny-országot.
///
/// Egy beolvadt alias (más ország-kontextussal) továbbra is feloldhatja a
/// versenyt, de a forrás országa nem billentheti vissza a `repair-countries`
/// által beállított kanonikus országot.
#[tokio::test]
async fn source_country_does_not_overwrite_existing_competition_country() {
    let pool = test_pool().await;
    let es = seed_country(&pool, "ES", "Spain").await;
    let fr = seed_country(&pool, "FR", "France").await;
    let source = seed_source(&pool, "unit_country").await;

    let created = find_or_create_competition(
        &mut pool.acquire().await.unwrap(),
        &CompetitionIdentity {
            id_hint: None,
            name: "Liga Egy",
            country_id: Some(es),
            country_code: Some("ES"),
            tier: Some(1),
            priority: Some(10),
            source_id: None,
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(created.matched_by, CompetitionMatch::Created);

    // A beolvadt alias más ország-kontextussal él (mint a merge utáni backfill).
    let entity_id: Uuid =
        sqlx::query_scalar("SELECT entity_id FROM core.competition WHERE id = $1")
            .bind(created.competition_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO identity.alias (id, entity_id, alias, lang, country_code, status, method)
         VALUES ($1, $2, 'Liga Uno', 'und', 'FR', 'reviewed', 'import')",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .execute(&pool)
    .await
    .unwrap();

    let resolved = find_or_create_competition(
        &mut pool.acquire().await.unwrap(),
        &CompetitionIdentity {
            id_hint: None,
            name: "Liga Uno",
            country_id: Some(fr),
            country_code: Some("FR"),
            tier: None,
            priority: None,
            source_id: Some(source),
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(resolved.competition_id, created.competition_id);
    assert_eq!(resolved.matched_by, CompetitionMatch::Alias);

    let code: String = sqlx::query_scalar(
        "SELECT co.code FROM core.competition c
         JOIN core.country co ON co.id = c.country_id WHERE c.id = $1",
    )
    .bind(created.competition_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(code, "ES", "a javított országot a forrás nem írhatja felül");
}

/// S84: az ország nélküli alias nem tapadhat más ország versenyére.
///
/// A generikus nevek („Liga 1", „National League", „Super Liga") régi aliasa
/// ország-kontextus nélkül jött létre; ismert forrás-ország mellett csak akkor
/// fogadható el, ha a verseny ugyanabban az országban van (vagy ország nélküli).
#[tokio::test]
async fn countryless_alias_does_not_capture_other_countrys_competition() {
    let pool = test_pool().await;
    let pe = seed_country(&pool, "PE", "Peru").await;
    let ro = seed_country(&pool, "RO", "Romania").await;

    let peru = find_or_create_competition(
        &mut pool.acquire().await.unwrap(),
        &CompetitionIdentity {
            id_hint: None,
            name: "Liga 1",
            country_id: Some(pe),
            country_code: Some("PE"),
            tier: Some(1),
            priority: Some(10),
            source_id: None,
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(peru.matched_by, CompetitionMatch::Created);

    // Régi, ország nélküli alias a perui versenyen.
    let entity_id: Uuid =
        sqlx::query_scalar("SELECT entity_id FROM core.competition WHERE id = $1")
            .bind(peru.competition_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        "INSERT INTO identity.alias (id, entity_id, alias, lang, country_code, status, method)
         VALUES ($1, $2, 'Liga 1', 'und', NULL, 'reviewed', 'import')",
    )
    .bind(Uuid::now_v7())
    .bind(entity_id)
    .execute(&pool)
    .await
    .unwrap();

    // Román forrás ugyanazzal a generikus névvel: NEM a perui versenyre tapad.
    let romanian = find_or_create_competition(
        &mut pool.acquire().await.unwrap(),
        &CompetitionIdentity {
            id_hint: None,
            name: "Liga 1",
            country_id: Some(ro),
            country_code: Some("RO"),
            tier: Some(1),
            priority: Some(10),
            source_id: None,
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(romanian.matched_by, CompetitionMatch::Created);
    assert_ne!(romanian.competition_id, peru.competition_id);

    // Peru forrás továbbra is megtalálja a saját versenyét az ország nélküli aliason át.
    let same = find_or_create_competition(
        &mut pool.acquire().await.unwrap(),
        &CompetitionIdentity {
            id_hint: None,
            name: "Liga 1",
            country_id: Some(pe),
            country_code: Some("PE"),
            tier: None,
            priority: None,
            source_id: None,
            source_entity_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(same.competition_id, peru.competition_id);
}
