//! Verseny-entitás find-or-create (S81).
//!
//! A verseny-identitás egyetlen domain-szabálya él itt: előbb a
//! forrás-specifikus hivatkozás (`identity.source_entity_ref`), majd a fold-olt
//! alias, majd a természetes kulcs (név + ország-kontextus), és csak ezután
//! hozunk létre új entitást. Ismeretlen ország esetén az ország-szűrő kimarad
//! (semleges wildcard), ugyanaz a minta, ami a csapatoknál már bevált.
//!
//! Mindkét létrehozási út (kanonikalizálás és a footballctl javító parancsa)
//! ezt a függvényt használja, hogy ne legyen lookup nélküli id-hint create.

use anyhow::{Context, Result, bail};
use football_domain::identity::{alias_country_code, fold_name};
use sqlx::PgConnection;
use uuid::Uuid;

/// Egy verseny-megfigyelés identitás-bemenete.
#[derive(Debug, Clone)]
pub struct CompetitionIdentity<'a> {
    /// A forrás által adott kanonikus id (jelenleg csak a fixture/demo út adja).
    pub id_hint: Option<Uuid>,
    pub name: &'a str,
    pub country_id: Option<Uuid>,
    /// Kanonikus ország-kód; `None` vagy semleges kód = nincs ország-kontextus.
    pub country_code: Option<&'a str>,
    /// `None` = a hívó nem kívánja felülírni a meglévő értéket.
    pub tier: Option<i16>,
    pub priority: Option<i32>,
    pub source_id: Option<Uuid>,
    pub source_entity_id: Option<&'a str>,
}

/// Melyik identitás-lépés találta meg a versenyt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompetitionMatch {
    IdHint,
    SourceRef,
    Alias,
    /// A verseny entitás kanonikus neve fold-olva egyezik (alias-hiány pótlása).
    CanonicalName,
    NaturalKey,
    Created,
}

impl CompetitionMatch {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::IdHint => "id_hint",
            Self::SourceRef => "source_ref",
            Self::Alias => "alias",
            Self::CanonicalName => "canonical_name",
            Self::NaturalKey => "natural_key",
            Self::Created => "created",
        }
    }
}

/// A find-or-create eredménye.
#[derive(Debug, Clone)]
pub struct CompetitionResolution {
    pub competition_id: Uuid,
    pub matched_by: CompetitionMatch,
    pub created: bool,
    pub alias_written: bool,
    pub ref_written: bool,
}

/// Megkeresi vagy létrehozza a versenyt a megfigyelés alapján.
///
/// A meglévő verseny metaadatait (ország, tier, prioritás) frissíti, de a
/// kanonikus nevet nem írja át: a névváltozatok az alias-táblában élnek.
pub async fn find_or_create_competition(
    conn: &mut PgConnection,
    input: &CompetitionIdentity<'_>,
) -> Result<CompetitionResolution> {
    let name = input.name.trim();
    if name.is_empty() {
        bail!("üres verseny-név nem hozható létre");
    }
    let folded = fold_name(name);
    if folded.is_empty() {
        // A fold-olt alak nélkül az alias- és természetes-kulcs egyeztetés
        // minden írásjel-only nevet összemosna; ilyen forrásadat hibás.
        bail!("a verseny-név nem tartalmaz betűt vagy számot: {name:?}");
    }

    let alias_country = resolve_alias_country(conn, input.country_id, input.country_code).await?;
    // S89: ha a forrás csak ország-kódot ad (pl. research: "AUT"), a lookup
    // ország-kontextusa eddig elveszett, és a kanonikus-név egyeztetés
    // ország nélküli wildcarddal a legnagyobb meccsszámú névrokonra esett
    // (Bundesliga DE vs AT). A kódból feloldjuk a country_id-t.
    let resolved_country_id =
        resolve_country_id(conn, input.country_id, input.country_code).await?;
    let effective = CompetitionIdentity {
        country_id: resolved_country_id,
        ..input.clone()
    };

    if let Some(hint) = input.id_hint
        && let Some(competition_id) = live_competition_by_id(conn, hint).await?
    {
        update_metadata(conn, competition_id, &effective).await?;
        let ref_written = ensure_source_ref(conn, competition_id, input).await?;
        return Ok(CompetitionResolution {
            competition_id,
            matched_by: CompetitionMatch::IdHint,
            created: false,
            alias_written: false,
            ref_written,
        });
    }

    if let Some(competition_id) = competition_by_source_ref(conn, input).await? {
        update_metadata(conn, competition_id, &effective).await?;
        return Ok(CompetitionResolution {
            competition_id,
            matched_by: CompetitionMatch::SourceRef,
            created: false,
            alias_written: false,
            ref_written: false,
        });
    }

    if let Some(competition_id) = competition_by_alias(
        conn,
        &folded,
        alias_country.as_deref(),
        effective.country_id,
    )
    .await?
    {
        update_metadata(conn, competition_id, &effective).await?;
        let ref_written = ensure_source_ref(conn, competition_id, input).await?;
        return Ok(CompetitionResolution {
            competition_id,
            matched_by: CompetitionMatch::Alias,
            created: false,
            alias_written: false,
            ref_written,
        });
    }

    // S81: az alias-réteg hiányos lehet a korábban létrehozott versenyeknél
    // (a create-on-miss csak mostantól ír aliast); ezért az entitás kanonikus
    // nevét is fold-olva egyeztetjük, mielőtt új versenyt hoznánk létre.
    if let Some(competition_id) =
        competition_by_canonical_name(conn, &folded, effective.country_id).await?
    {
        update_metadata(conn, competition_id, &effective).await?;
        let ref_written = ensure_source_ref(conn, competition_id, input).await?;
        // A régi soroknál a competition.id és az entity.id eltérhet, ezért az
        // aliast az entitásra írjuk, nem a verseny sorára.
        let entity_id: Uuid =
            sqlx::query_scalar("SELECT entity_id FROM core.competition WHERE id = $1")
                .bind(competition_id)
                .fetch_one(&mut *conn)
                .await
                .context("verseny entitás-azonosítójának olvasása")?;
        let alias_written = insert_alias(conn, entity_id, name, alias_country.as_deref()).await?;
        return Ok(CompetitionResolution {
            competition_id,
            matched_by: CompetitionMatch::CanonicalName,
            created: false,
            alias_written,
            ref_written,
        });
    }

    if let Some(competition_id) =
        competition_by_natural_key(conn, name, &folded, effective.country_id).await?
    {
        update_metadata(conn, competition_id, &effective).await?;
        let ref_written = ensure_source_ref(conn, competition_id, input).await?;
        return Ok(CompetitionResolution {
            competition_id,
            matched_by: CompetitionMatch::NaturalKey,
            created: false,
            alias_written: false,
            ref_written,
        });
    }

    let competition_id = input.id_hint.unwrap_or_else(Uuid::now_v7);
    insert_competition(conn, competition_id, name, &effective).await?;
    let alias_written = insert_alias(conn, competition_id, name, alias_country.as_deref()).await?;
    let ref_written = ensure_source_ref(conn, competition_id, input).await?;

    Ok(CompetitionResolution {
        competition_id,
        matched_by: CompetitionMatch::Created,
        created: true,
        alias_written,
        ref_written,
    })
}

/// Az alias ország-kódja: semleges kód (`INT`, `XX`, ...) esetén `None`, hogy
/// az alias-lookup ne szűkítsen feleslegesen országra.
async fn resolve_alias_country(
    conn: &mut PgConnection,
    country_id: Option<Uuid>,
    country_code: Option<&str>,
) -> Result<Option<String>> {
    let from_code = alias_country_code(country_code);
    if from_code.is_some() || country_id.is_none() {
        return Ok(from_code);
    }
    let code: Option<String> = sqlx::query_scalar("SELECT code FROM core.country WHERE id = $1")
        .bind(country_id)
        .fetch_optional(&mut *conn)
        .await
        .context("verseny ország-kódjának olvasása")?;
    Ok(alias_country_code(code.as_deref()))
}

/// A forrás ország-kódjából feloldott `core.country.id`, ha a hívó nem adott
/// közvetlenül ország-azonosítót. Ismeretlen/semleges kód → `None`.
async fn resolve_country_id(
    conn: &mut PgConnection,
    country_id: Option<Uuid>,
    country_code: Option<&str>,
) -> Result<Option<Uuid>> {
    if country_id.is_some() {
        return Ok(country_id);
    }
    let Some(code) = alias_country_code(country_code) else {
        return Ok(None);
    };
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT id FROM core.country WHERE code = $1 LIMIT 1")
            .bind(code)
            .fetch_optional(&mut *conn)
            .await
            .context("verseny ország-azonosítójának feloldása")?;
    Ok(id)
}

/// Id-hint feloldása a merge-láncon át a mai (nem merged) versenyre.
async fn live_competition_by_id(conn: &mut PgConnection, id: Uuid) -> Result<Option<Uuid>> {
    let mut current = id;
    // A merge-lánc ciklusát nem követjük végtelenül; a gyakorlatban 1-2 lépés.
    for _ in 0..8 {
        let row: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT c.id, e.merged_into_id
             FROM core.competition c
             JOIN core.entity e ON e.id = c.entity_id
             WHERE c.id = $1",
        )
        .bind(current)
        .fetch_optional(&mut *conn)
        .await
        .context("id-hint verseny olvasása")?;

        let Some((competition_id, merged_into)) = row else {
            return Ok(None);
        };
        match merged_into {
            None => return Ok(Some(competition_id)),
            Some(target) => {
                let next: Option<Uuid> =
                    sqlx::query_scalar("SELECT id FROM core.competition WHERE entity_id = $1")
                        .bind(target)
                        .fetch_optional(&mut *conn)
                        .await
                        .context("merge-cél verseny olvasása")?;
                match next {
                    Some(competition_id) => current = competition_id,
                    None => return Ok(None),
                }
            }
        }
    }
    Ok(None)
}

async fn competition_by_source_ref(
    conn: &mut PgConnection,
    input: &CompetitionIdentity<'_>,
) -> Result<Option<Uuid>> {
    let (Some(source_id), Some(source_entity_id)) = (input.source_id, input.source_entity_id)
    else {
        return Ok(None);
    };
    sqlx::query_scalar(
        "SELECT c.id
         FROM identity.source_entity_ref r
         JOIN core.entity e ON e.id = r.entity_id
         JOIN core.competition c ON c.entity_id = e.id
         WHERE r.source_id = $1
           AND r.entity_kind = 'competition'
           AND r.source_entity_id = $2
           AND r.status = 'active'
           AND e.merged_into_id IS NULL
           AND ($3::uuid IS NULL OR c.country_id IS NULL OR c.country_id = $3)
         ORDER BY r.created_at DESC
         LIMIT 1",
    )
    .bind(source_id)
    .bind(source_entity_id)
    .bind(input.country_id)
    .fetch_optional(&mut *conn)
    .await
    .context("verseny source_entity_ref lookup")
}

async fn competition_by_alias(
    conn: &mut PgConnection,
    folded_name: &str,
    alias_country: Option<&str>,
    country_id: Option<Uuid>,
) -> Result<Option<Uuid>> {
    // S84: az ország nélküli alias csak akkor fogadható el ismert forrás-ország
    // mellett, ha maga a verseny is ország nélküli vagy ugyanabban az országban
    // van. Enélkül a „Liga 1"/„National League"/„Super Liga" típusú generikus
    // nevek bármely ország névrokon versenyére rátapadtak (pl. román meccsek a
    // perui Liga 1-re), mert az alias ország-kontextusa NULL volt.
    sqlx::query_scalar(
        "SELECT c.id
         FROM identity.alias a
         JOIN core.entity e ON e.id = a.entity_id
         JOIN core.competition c ON c.entity_id = e.id
         WHERE e.kind = 'competition'
           AND e.merged_into_id IS NULL
           AND a.status = 'reviewed'
           AND identity.fold_name(a.alias) = $1
           AND (
                 $2::text IS NULL
                 OR a.country_code = $2
                 OR (a.country_code IS NULL
                     AND ($3::uuid IS NULL OR c.country_id IS NULL OR c.country_id = $3))
               )
         ORDER BY (a.country_code = $2) DESC NULLS LAST,
                  (SELECT count(*) FROM core.match m WHERE m.competition_id = c.id) DESC,
                  c.id
         LIMIT 1",
    )
    .bind(folded_name)
    .bind(alias_country)
    .bind(country_id)
    .fetch_optional(&mut *conn)
    .await
    .context("verseny alias lookup")
}

/// Kanonikus entitás-név fold-olt egyezése ország-kontextussal.
///
/// A `country_id` szerinti szűrés megengedi a NULL országú sort (semleges
/// wildcard), de más ország versenyét nem hozza be.
async fn competition_by_canonical_name(
    conn: &mut PgConnection,
    folded: &str,
    country_id: Option<Uuid>,
) -> Result<Option<Uuid>> {
    let id: Option<Uuid> = sqlx::query_scalar(
        "SELECT c.id
         FROM core.competition c
         JOIN core.entity e ON e.id = c.entity_id AND e.merged_into_id IS NULL
         WHERE identity.fold_name(e.canonical_name) = $1
           AND ($2::uuid IS NULL OR c.country_id IS NULL OR c.country_id = $2)
         ORDER BY (c.country_id = $2) DESC NULLS LAST, c.id
         LIMIT 1",
    )
    .bind(folded)
    .bind(country_id)
    .fetch_optional(&mut *conn)
    .await
    .context("verseny kanonikus-név lookup")?;
    Ok(id)
}

async fn competition_by_natural_key(
    conn: &mut PgConnection,
    name: &str,
    folded_name: &str,
    country_id: Option<Uuid>,
) -> Result<Option<Uuid>> {
    sqlx::query_scalar(
        "SELECT c.id
         FROM core.competition c
         JOIN core.entity e ON e.id = c.entity_id
         WHERE e.kind = 'competition'
           AND e.merged_into_id IS NULL
           AND ($2::uuid IS NULL OR c.country_id = $2 OR c.country_id IS NULL)
           AND (c.name = $1 OR identity.fold_name(c.name) = $3)
         ORDER BY (c.country_id = $2) DESC NULLS LAST,
                  (c.name = $1) DESC,
                  (SELECT count(*) FROM core.match m WHERE m.competition_id = c.id) DESC,
                  c.id
         LIMIT 1",
    )
    .bind(name)
    .bind(country_id)
    .bind(folded_name)
    .fetch_optional(&mut *conn)
    .await
    .context("verseny természetes-kulcs lookup")
}

async fn update_metadata(
    conn: &mut PgConnection,
    competition_id: Uuid,
    input: &CompetitionIdentity<'_>,
) -> Result<()> {
    // S84: a verseny országát az ingest NEM írhatja felül, ha már van értéke.
    // Az ország a forrás-szavazatokból dolgozó `competitions repair-countries`
    // hatásköre; egyetlen forrás megfigyelése (pl. egy beolvadt alias országa)
    // különben visszabillentené a javított országot. Csak a hiányzó (NULL)
    // országot töltjük ki.
    sqlx::query(
        "UPDATE core.competition
         SET country_id = COALESCE(country_id, $2),
             tier = COALESCE($3, tier),
             priority = COALESCE($4, priority)
         WHERE id = $1",
    )
    .bind(competition_id)
    .bind(input.country_id)
    .bind(input.tier)
    .bind(input.priority)
    .execute(&mut *conn)
    .await
    .context("verseny metaadatainak frissítése")?;
    Ok(())
}

async fn insert_competition(
    conn: &mut PgConnection,
    competition_id: Uuid,
    name: &str,
    input: &CompetitionIdentity<'_>,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO core.entity (id, kind, canonical_name)
         VALUES ($1, 'competition', $2)",
    )
    .bind(competition_id)
    .bind(name)
    .execute(&mut *conn)
    .await
    .context("verseny entitás létrehozása")?;

    sqlx::query(
        "INSERT INTO core.competition (id, entity_id, country_id, name, tier, priority)
         VALUES ($1, $1, $2, $3, $4, $5)",
    )
    .bind(competition_id)
    .bind(input.country_id)
    .bind(name)
    .bind(input.tier.unwrap_or(1))
    .bind(input.priority.unwrap_or(100))
    .execute(&mut *conn)
    .await
    .context("verseny létrehozása")?;

    Ok(())
}

async fn insert_alias(
    conn: &mut PgConnection,
    competition_id: Uuid,
    name: &str,
    alias_country: Option<&str>,
) -> Result<bool> {
    let result = sqlx::query(
        "INSERT INTO identity.alias
             (id, entity_id, alias, lang, country_code, status, method)
         VALUES ($1, $2, $3, 'und', $4, 'reviewed', 'import')
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(competition_id)
    .bind(name)
    .bind(alias_country)
    .execute(&mut *conn)
    .await
    .context("verseny alias írása")?;
    Ok(result.rows_affected() == 1)
}

async fn ensure_source_ref(
    conn: &mut PgConnection,
    competition_id: Uuid,
    input: &CompetitionIdentity<'_>,
) -> Result<bool> {
    let (Some(source_id), Some(source_entity_id)) = (input.source_id, input.source_entity_id)
    else {
        return Ok(false);
    };
    let result = sqlx::query(
        "INSERT INTO identity.source_entity_ref
             (id, source_id, entity_kind, source_entity_id, entity_id, method, confidence, status)
         VALUES ($1, $2, 'competition', $3, $4, 'exact_external_id', 1.0, 'active')
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(source_id)
    .bind(source_entity_id)
    .bind(competition_id)
    .execute(&mut *conn)
    .await
    .context("verseny source_entity_ref írása")?;
    Ok(result.rows_affected() == 1)
}
