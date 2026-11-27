use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{NaiveDate, TimeZone, Utc};
use clap::{Parser, Subcommand};
#[cfg(feature = "obscura")]
use football_domain::policy::Transport;
use football_domain::trust::{ODDS_TOLERANCE_DEFAULT, OddsTrustState};
use football_ingest::adapter::SourceAdapter;
use football_ingest::canonicalize::Canonicalizer;
use football_ingest::demo::DemoAdapter;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::fixture::{FixtureAdapter, FixtureSource};
use football_ingest::footballdata::FootballdataAdapter;
use football_ingest::fotmob::FotMobAdapter;
use football_ingest::http::RustHttpFetcher;
use football_ingest::openfoot::OpenFootAdapter;
use football_ingest::soccerway::SoccerwayAdapter;
use football_storage::object_store::ObjectStore;
use football_storage::repositories::audit as audit_repo;
use football_storage::repositories::competition_merge;
use football_storage::repositories::day::{DayFilter, fetch_day};
use football_storage::repositories::identity as identity_repo;
use football_storage::repositories::identity_merge;
use football_storage::repositories::trust;
use football_storage::repositories::worker as worker_repo;
use football_verification::conflict::load_resolved_fields;
use football_verification::engine::{VerificationContext, VerificationEngine};
use football_verification::gate::evaluate_and_persist_gate;
use football_verification::odds::evaluate_odds_trust;
use football_verification::persist::persist_report;
use football_verification::rules::default_rules;
use football_worker::{scheduler, tick};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

mod competitions;
mod issues;
mod match_insight_import;
mod player_import;
mod prediction_fit;
mod prediction_measure;
mod rounds;
mod tipplap_replay;

/// S2b-2: a TRUST_LEVEL + odds trust szabálykészlet verziója (a tickével azonos).
const RULE_SET_VERSION: &str = "verification-1.3";

#[derive(Parser)]
#[command(name = "footballctl")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum IssuesAction {
    /// Egy nap hiányjeleinek rögzítése (dry-run alapból, auditált).
    Detect {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
}

#[derive(Subcommand)]
enum DedupeAction {
    /// Duplikált kanonikus meccsek merge-e egy napon (dry-run alapból).
    Matches {
        #[arg(long)]
        date: NaiveDate,
        /// Opcionális tartomány-vég (backfill-takarítás).
        #[arg(long)]
        to: Option<NaiveDate>,
        /// Tényleges végrehajtás (alapból csak terv).
        #[arg(long)]
        apply: bool,
        /// Elavult névváltozatok `|`-vel elválasztva: a túlélő az a sor,
        /// amelyik NEM ezek közül való (egyébként a legkorábbi sor).
        #[arg(long)]
        deny: Option<String>,
    },
    /// S96: cross-source csapatnév-változatból keletkezett duplikátumok
    /// (azonos kickoff + foldolt versenynév + státusz + eredmény, az egyik
    /// oldalon azonos csapat-entitás, a másikon hasonló nevű, azonos
    /// kontextusú csapat). `--apply` esetén a csapat-entitások és a
    /// fixture-ök is merge-elődnek. Dry-run alapból.
    Aliases {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        /// Tényleges végrehajtás (alapból csak terv).
        #[arg(long)]
        apply: bool,
        /// Az eltérő csapatnevek foldolt hasonlóságának alsó korlátja.
        #[arg(long, default_value_t = 0.55)]
        min_similarity: f64,
    },
    /// S37c: név-variáns duplikátumok egy napon (legfeljebb 12 órás
    /// kickoff-különbség, de forrásonként eltérő csapatnév). Alapból csak azonos
    /// versenyen belül; `--cross-competition` kapcsolóval a különböző
    /// verseny-entitások alatti sorok is (azonos normalizált ország +
    /// csapat-kontextus mellett). A szigorú `matches` parancs ezeket nem látja;
    /// dry-run alapból.
    Variants {
        #[arg(long)]
        date: NaiveDate,
        /// Opcionális záró nap: a `--date` és `--to` közötti teljes tartományt
        /// vizsgálja (a naponkénti ±1 napos ablak helyett). Hasznos a heti
        /// átvizsgáláshoz.
        #[arg(long)]
        to: Option<NaiveDate>,
        /// Tényleges végrehajtás (alapból csak terv).
        #[arg(long)]
        apply: bool,
        /// Kézi döntés: lazább név-ellenőrzés (12 órás kickoff-ablak,
        /// fordítható pár). Csak tudatos használatra.
        #[arg(long)]
        allow_loose: bool,
        /// Különböző verseny-entitások alatti sorok párosítása is (pl. Cymru
        /// Premier vs tévesen "Premier League" alá sorolt walesi sor). Csak
        /// egyező normalizált verseny-ország és egyező csapat-kontextus mellett.
        /// Alapból kikapcsolva.
        #[arg(long)]
        cross_competition: bool,
    },
    /// Kézi, ellenőrzött pár merge: a megadott két meccs közül a `survivor`
    /// marad, a `duplicate` archiválva törlődik. Csak akkor használd, ha a
    /// pár valóban ugyanaz a fixture (pl. eltérő verseny-entitás miatt maradt ki).
    Pair {
        #[arg(long)]
        survivor: Uuid,
        #[arg(long)]
        duplicate: Uuid,
        #[arg(long)]
        note: String,
    },
    /// S26: fixture-szintű rekonciliáció — azonos verseny+szezon+csapat-pár+
    /// UTC-nap, eltérő kickoff (v1 import vs footballdata). Auto-merge csak
    /// azonos orientációnál és egyező (vagy egyik oldalon hiányzó) eredménynél;
    /// a kockázatos párok listázódnak, nem íródnak. Dry-run alapból.
    Fixtures {
        #[arg(long)]
        from: Option<NaiveDate>,
        #[arg(long)]
        to: Option<NaiveDate>,
        /// Verseny kanonikus neve (pontos egyezés).
        #[arg(long)]
        competition: Option<String>,
        #[arg(long)]
        apply: bool,
        /// Merge-ek felső korlátja (kötegelt futtatáshoz).
        #[arg(long)]
        limit: Option<usize>,
        /// Felülvizsgált kockázatos párok merge-e: a megadott indoklás
        /// az archívumba kerül. Csak akkor használd, ha a források
        /// közül a túlélő eredményét külső bizonyítékkal ellenőrizted.
        #[arg(long)]
        review_note: Option<String>,
    },
}

#[derive(Subcommand)]
enum CompetitionsAction {
    /// S42: a forrás-szavazatokból javítja a versenyek országát, a versenynevet
    /// és a rossz országú meccseket a helyes cél-versenybe/szezonba sorolja át.
    /// Alapból dry-run (csak riport); `--apply` egy tranzakcióban ír, minden
    /// változás `ops.audit_event` sort kap.
    RepairCountries {
        /// A helyi (Europe/Budapest) intervallum első napja, zárt.
        #[arg(long)]
        from: NaiveDate,
        /// Az intervallum utolsó napja, zárt.
        #[arg(long)]
        to: NaiveDate,
        /// Tényleges végrehajtás (alapból csak terv).
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
enum Command {
    Migrate,
    IngestFixture {
        #[arg(long)]
        path: PathBuf,
    },
    Day {
        #[arg(long)]
        date: NaiveDate,
        #[arg(long, default_value_t = false)]
        include_all: bool,
    },
    Doctor,
    Worker {
        #[command(subcommand)]
        action: WorkerAction,
    },
    Identity {
        #[command(subcommand)]
        action: IdentityAction,
    },
    /// S198: shadow predikciók as-of EB-illesztésből (`--apply` nélkül nem ír).
    PredictionFit {
        #[arg(long)]
        date: NaiveDate,
        /// Backtest: a becslés cutoffja; ekkor a modellverzió `-bt-{dátum}` utótagot kap.
        #[arg(long)]
        as_of: Option<NaiveDate>,
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// S211: hiányzó fordulószámok pótlása research obszervációkból.
    RoundsBackfill {
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// S196: as-of mérési snapshot a piaci oddsokra (shadow, `--apply` nélkül nem ír).
    PredictionMeasure {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        /// Mérendő modellverzió (alap: market-shin-v1).
        #[arg(long)]
        model_version: Option<String>,
        #[arg(long, default_value_t = false)]
        apply: bool,
    },
    /// S170: hiányjelek rögzítése az ops.issue táblába.
    Issues {
        #[command(subcommand)]
        action: IssuesAction,
    },
    Asset {
        #[command(subcommand)]
        action: AssetAction,
    },
    Dedupe {
        #[command(subcommand)]
        action: DedupeAction,
    },
    /// S47: heti duplikátum- és konzisztencia-audit (read-only); találat esetén exit 1.
    Audit {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        /// Gép-olvasható JSON a magyar összefoglaló helyett.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// S42: verseny-ország javítás a kiválasztott kickoff-megfigyelésekből.
    Competitions {
        #[command(subcommand)]
        action: CompetitionsAction,
    },
    /// Napi lefedettségi riport (S64): meccs, eredmény, odds, logó, hiányok.
    CoverageDay {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
    },
    /// Liga-szezon lefedettségi riport (S23).
    Coverage {
        /// Csak ettől a szezon-kezdőévtől (pl. 2015).
        #[arg(long)]
        from: Option<i32>,
        /// Csak eddig a szezon-kezdőévig.
        #[arg(long)]
        to: Option<i32>,
        /// Listázott szezonok felső korlátja (alap: 300, a legnagyobb szezonok).
        #[arg(long, default_value_t = 300)]
        limit: i64,
        /// Csak ez a verseny (pontos név).
        #[arg(long)]
        competition: Option<String>,
    },
    /// Napi integritás-riport (read-only): duplikátumok, árva hivatkozások,
    /// review/gate/job/breaker állomány. Kemény hiba esetén exit 1.
    Integrity {
        #[arg(long)]
        from: Option<NaiveDate>,
        #[arg(long)]
        to: Option<NaiveDate>,
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Saját Elo-számítás a lezárt meccsekből (fact.club_elo, own-elo).
    Elo {
        /// A pillanatkép dátuma (alap: ma).
        #[arg(long)]
        as_of: Option<NaiveDate>,
    },
    /// Adatbázis-állapot riport: liga/ország/szezon bontás, minőségi mutatók.
    DataStatus {
        #[arg(long)]
        from: Option<NaiveDate>,
        #[arg(long)]
        to: Option<NaiveDate>,
        #[arg(long, default_value_t = 80)]
        top: i64,
    },
    /// Meccsek újraellenőrzése a tárolt megfigyelésekből (fetch nélkül).
    Reverify {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        #[arg(long)]
        limit: Option<i64>,
    },
    /// Research meccs-insight szekciók importja a már tárolt JSON-ekből,
    /// hálózat nélkül. Alapból dry-run; auditált upsert.
    MatchInsightImport {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        #[arg(long)]
        apply: bool,
    },
    /// Játékosadatok importja a már tárolt helyi research JSON-ekből,
    /// hálózat nélkül. Alapból dry-run; csak stabil forrás-ID alapján hoz létre
    /// játékosentitást (exact_external_id, nincs fuzzy merge).
    PlayerImport {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        #[arg(long)]
        apply: bool,
    },
    /// Tipplap replay: a már tárolt nyers HTML újrafeldolgozása a jelenlegi
    /// parserrel, hálózati kérés nélkül. Alapból dry-run.
    TipplapReplay {
        #[arg(long)]
        from: NaiveDate,
        #[arg(long)]
        to: NaiveDate,
        #[arg(long)]
        apply: bool,
    },
    /// Konfliktusok listája (alap: feloldatlan).
    Conflicts {
        #[arg(long, default_value = "unresolved")]
        status: String,
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
    /// Konfliktus tételes feloldása: a győztes megfigyelés a meccs
    /// provenance-láncából származzon; minden döntés auditált.
    ResolveConflict {
        #[arg(long)]
        match_id: Uuid,
        #[arg(long, default_value = "match")]
        field: String,
        #[arg(long)]
        winner: Uuid,
        #[arg(long)]
        reason: String,
    },
    /// Történelmi backfill jobok tervezése (S23).
    Backfill {
        #[command(subcommand)]
        action: BackfillAction,
    },
    /// Helyi AI (Magnitude + SearXNG) hívások; nem igényel adatbázist.
    Ai {
        #[command(subcommand)]
        action: AiAction,
    },
}

#[derive(Subcommand)]
enum AiAction {
    /// Helyi SearXNG keresés nyers találatokkal.
    Search {
        query: String,
        #[arg(long, default_value_t = 6)]
        limit: usize,
    },
    /// Kérdés megválaszolása a helyi modell + SearXNG tool-hurokkal.
    Ask {
        question: String,
        #[arg(long)]
        max_steps: Option<usize>,
    },
}

#[derive(Subcommand)]
enum BackfillAction {
    /// Liga-szezon backfill jobok sorba állítása (alapból dry-run).
    Plan {
        #[arg(long, default_value = "footballdata")]
        source: String,
        /// Kezdő szezon kezdőéve (pl. 2015).
        #[arg(long)]
        from: Option<u16>,
        /// Utolsó szezon kezdőéve (pl. 2026).
        #[arg(long)]
        to: Option<u16>,
        /// Liga-kódok vesszővel (pl. e0,e1), vagy "all".
        #[arg(long, default_value = "all")]
        leagues: String,
        /// A `new/` ligák (egy fájl = sok szezon) sorba állítása.
        #[arg(long)]
        new_leagues: bool,
        #[arg(long)]
        apply: bool,
    },
    /// v1 import (golfolyam-írás) egy versenyre: mérkőzések + 1X2 záró/nyitó
    /// odds a football_app adatbázisból, a normál ellenőrzési láncon át.
    V1Import {
        #[arg(long)]
        tournament: i64,
        #[arg(long, default_value_t = 2015)]
        from_year: i32,
        #[arg(long, default_value_t = 2026)]
        to_year: i32,
        /// Meglévő v2 verseny UUID: a v1 versenynév helyett ide tölti
        /// (pl. v1 "La Liga 2" → v2 "LaLiga 2"), így nem jön létre új entitás.
        #[arg(long)]
        target_competition: Option<uuid::Uuid>,
        #[arg(long)]
        apply: bool,
    },
    /// v1 pilot-előkészítés: csapat-illesztés és odds-mezők (read-only, írás nélkül).
    V1Pilot {
        /// A v1 tournament azonosítója (pl. 27 = Eliteserien).
        #[arg(long)]
        tournament: i64,
        #[arg(long, default_value_t = 2015)]
        from_year: i32,
        #[arg(long, default_value_t = 2026)]
        to_year: i32,
    },
    /// v1 (football_app) lefedettség a v2-höz képest — read-only riport.
    V1Inventory {
        #[arg(long, default_value_t = 2000)]
        from_year: i32,
        #[arg(long, default_value_t = 2026)]
        to_year: i32,
        #[arg(long, default_value_t = 60)]
        top: i64,
    },
}

#[derive(Subcommand)]
enum WorkerAction {
    Status,
    Tick,
}

#[derive(Subcommand)]
enum IdentityAction {
    Review {
        #[arg(long)]
        status: Option<String>,
    },
    Approve {
        id: Uuid,
        #[arg(long)]
        entity: Uuid,
        #[arg(long)]
        note: Option<String>,
    },
    /// Új kanonikus entitás létrehozása a review-sorból (élő forrás bootstrap).
    ApproveCreate {
        id: Uuid,
        #[arg(long)]
        note: Option<String>,
    },
    /// Minden pending review-sor batch jóváhagyása új entitással (bootstrap).
    ApproveCreateAll {
        /// Ország nélküli kontextusú csapatot NULL országgal hoz létre
        /// (őszinte hiány; alapból az ilyen sorok pending maradnak).
        #[arg(long)]
        allow_unknown_country: bool,
    },
    Reject {
        id: Uuid,
        #[arg(long)]
        note: String,
    },
    /// Pending review-ök összesítése (nem/korosztály/ország/verseny).
    ReviewSummary,
    /// Minden pending review-sor elutasítása kötelező indoklással
    /// (pl. lokalizált, elavult kör tisztítása).
    RejectAllPending {
        #[arg(long)]
        note: String,
    },
    ListAliases {
        #[arg(long)]
        entity: Option<Uuid>,
    },
    /// v1 crosswalk-seed: csapat- és alias-nevek importja a v1 adatbázisból
    /// (csak olvasás). Alapból dry-run; `--apply` reviewed aliasokat ír.
    SeedV1 {
        #[arg(long, default_value = "football_app")]
        v1_db: String,
        #[arg(long)]
        apply: bool,
        /// Csak az első N v1 csapatot dolgozza fel (minta-ellenőrzéshez).
        #[arg(long)]
        limit: Option<i64>,
    },
    /// Duplikált csapat-entitások (azonos név + kontextus) listája.
    Duplicates,
    /// Duplikált csapat-entitás összevonása a túlélőbe (alapból dry-run).
    MergeTeam {
        /// Az összevonandó (áldozat) entitás.
        #[arg(long)]
        from: Uuid,
        /// A túlélő entitás.
        #[arg(long)]
        to: Uuid,
        #[arg(long, default_value = "S19 entitás-merge")]
        note: String,
        #[arg(long)]
        apply: bool,
        /// S26e: alias-variáns merge (eltérő nevek). A bizonyíték-szöveg
        /// kötelező (pl. „v1 team_aliases: 'Man City' -> 'Manchester City'”);
        /// az áldozat neve reviewed aliasként rögzítődik a túlélőn.
        #[arg(long)]
        via_alias: Option<String>,
    },
    /// S81: duplikált verseny-entitás összevonása a túlélőbe (alapból dry-run).
    MergeCompetition {
        /// A beolvadó (áldozat) verseny-entitás.
        #[arg(long)]
        from: Uuid,
        /// A túlélő verseny-entitás.
        #[arg(long, alias = "to")]
        into: Uuid,
        #[arg(long, default_value = "S81 verseny-entity merge")]
        note: String,
        #[arg(long)]
        apply: bool,
        /// A meccsszám-őr megkerülése: a survivor kevesebb meccsel is nyerhet.
        #[arg(long)]
        force: bool,
    },
    CrosswalkCheck,
    /// S41: a forrás csapat-crosswalkjának újraépítése a legfrissebb
    /// `team_identity` megfigyelésekből. Alapból dry-run (nulla írás).
    RebuildRefs {
        /// Forrás-kód (pl. `soccerway`).
        #[arg(long, default_value = "soccerway")]
        source: String,
        /// Tényleges írás (supersede + új aktív ref, auditálva).
        #[arg(long)]
        apply: bool,
    },
    /// S41: a megadott kickoff-tartomány soccerway meccseinek
    /// csapat-hozzárendelése a saját capture-jük team-nevei alapján.
    /// Alapból dry-run (nulla írás).
    RepairMatches {
        /// Kezdő nap (helyi, UTC-re vetítve 00:00).
        #[arg(long)]
        from: NaiveDate,
        /// Záró nap (kizárólagos felső határ).
        #[arg(long)]
        to: NaiveDate,
        /// Tényleges írás (átállítás/merge/visszatartás, auditálva).
        #[arg(long)]
        apply: bool,
        /// Feloldhatatlan meccs visszatartása a napi listáról (apply-ban).
        #[arg(long)]
        withhold_unresolved: bool,
    },
}

#[derive(Subcommand)]
enum AssetAction {
    /// Asset-JSON importálása (tömb: entity_kind, entity_id?, kind, url,
    /// source, licence?, licence_status, attribution?, captured_at?).
    Import {
        #[arg(long)]
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    // Az AI-parancsok csak a helyi Magnitude/SearXNG szolgáltatásokat használják,
    // adatbázis nélkül is működniük kell, ezért a pool előtt futnak.
    if let Command::Ai { action } = cli.command {
        return run_ai(action).await;
    }
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL hiányzik")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await?;
    let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".into());
    let store = ObjectStore::new(&data_dir)?;

    match cli.command {
        Command::Migrate => {
            sqlx::migrate!("../../../db/migrations").run(&pool).await?;
            println!("migráció kész");
        }
        Command::IngestFixture { path } => {
            let report = FixtureAdapter::ingest(&pool, &store, &path).await?;
            let fixture: FixtureSource = serde_json::from_slice(&std::fs::read(&path)?)?;
            let normalized = football_ingest::normalized::NormalizedDay::from(&fixture);
            let day = Canonicalizer::materialize(&pool, &normalized).await?;
            let rules = default_rules();

            // S2b-2: trust sorok egyszer, az ÖSSZES hozzájáruló forrásra
            // (a meccsek source_values forráskódjai + a fixture forrása).
            let mut material_values = Vec::with_capacity(day.matches.len());
            let mut source_codes = vec![fixture.source_code.clone()];
            for materialized in &day.matches {
                let values = football_verification::conflict::load_material_values(
                    &pool,
                    materialized.match_id,
                )
                .await?;
                for value in &values {
                    if !source_codes.contains(&value.source_code) {
                        source_codes.push(value.source_code.clone());
                    }
                }
                material_values.push(values);
            }
            let source_trust = trust::trust_for_sources(&pool, &source_codes).await?;

            for (materialized, source_values) in day.matches.iter().zip(&material_values) {
                let home_identity_exact = materialized.identity_exact;
                let away_identity_exact = materialized.identity_exact;
                let odds_values = trust::load_match_odds(&pool, materialized.match_id).await?;
                let resolved_fields = load_resolved_fields(&pool, materialized.match_id).await?;
                let team =
                    load_team_context(&pool, materialized.home_team_id, materialized.away_team_id)
                        .await?;
                let competition_country =
                    load_competition_country(&pool, materialized.match_id).await?;
                let context = VerificationContext {
                    snapshot: &materialized.snapshot,
                    home_identity_exact,
                    away_identity_exact,
                    home_team_name: team.home_name,
                    away_team_name: team.away_name,
                    home_team_country: team.home_country_code,
                    away_team_country: team.away_country_code,
                    competition_country,
                    duplicate_match_count: duplicate_count(&pool, materialized).await?,
                    source_values,
                    source_trust: &source_trust,
                    odds_values: &odds_values,
                    odds_tolerance: ODDS_TOLERANCE_DEFAULT,
                    resolved_fields: &resolved_fields,
                };
                let report = VerificationEngine::run(&rules, &context);
                let run_id =
                    persist_report(&pool, materialized.match_id, &report, RULE_SET_VERSION).await?;
                let date = football_verification::gate::local_publish_date(
                    materialized.snapshot.kickoff_at,
                    "Europe/Budapest",
                )?;
                let state = evaluate_and_persist_gate(
                    &pool,
                    materialized.match_id,
                    date,
                    &report,
                    RULE_SET_VERSION,
                )
                .await?;
                // Odds trust_state a gate után; üres odds listán nincs frissítés.
                let odds_state = evaluate_odds_trust(&odds_values, ODDS_TOLERANCE_DEFAULT);
                if odds_state != OddsTrustState::NoOdds {
                    trust::update_odds_trust_state(&pool, materialized.match_id, odds_state)
                        .await?;
                }
                println!(
                    "match={} run={} state={}",
                    materialized.match_id,
                    run_id,
                    state.as_db_str()
                );
            }
            println!(
                "capture={} observations={}",
                report.capture_id,
                report.observation_ids.len()
            );
        }
        Command::Audit { from, to, json } => {
            let report = audit_repo::audit_range(&pool, from, to).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                audit_report_print(&report);
            }
            if !report.is_clean() {
                std::process::exit(1);
            }
        }
        Command::Day { date, include_all } => {
            let rows = fetch_day(
                &pool,
                &DayFilter {
                    date,
                    timezone: "Europe/Budapest".into(),
                    include_all,
                    competition_id: None,
                    window_days: 0,
                    team_id: None,
                },
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        Command::Doctor => {
            let counts = worker_repo::doctor_counts(&pool).await?;
            println!(
                "db={} gates={} jobs_pending={} breakers_open={} quarantined={}",
                counts.reachable,
                counts.publish_gates,
                counts.jobs_pending,
                counts.breakers_open,
                counts.quarantined
            );
        }
        Command::Worker { action } => match action {
            WorkerAction::Status => worker_status(&pool).await?,
            WorkerAction::Tick => worker_tick(&pool, &store, &data_dir).await?,
        },
        Command::Identity { action } => identity_command(&pool, action).await?,
        Command::PredictionFit { date, as_of, apply } => {
            prediction_fit::run(&pool, date, as_of, apply).await?;
        }
        Command::RoundsBackfill { apply } => {
            rounds::run(&pool, apply).await?;
        }
        Command::PredictionMeasure {
            from,
            to,
            model_version,
            apply,
        } => {
            prediction_measure::run(&pool, from, to, model_version.as_deref(), apply).await?;
        }
        Command::Issues { action } => match action {
            IssuesAction::Detect { date, apply } => {
                issues::run_detect(&pool, date, apply).await?;
            }
        },
        Command::Dedupe { action } => match action {
            DedupeAction::Matches {
                date,
                to,
                apply,
                deny,
            } => {
                let deny_list: Vec<String> = deny
                    .as_deref()
                    .map(|d| d.split('|').map(|x| x.trim().to_string()).collect())
                    .unwrap_or_default();
                let groups = match to {
                    Some(end) => {
                        football_storage::repositories::dedupe::duplicate_groups_between(
                            &pool, date, end,
                        )
                        .await?
                    }
                    None => {
                        football_storage::repositories::dedupe::duplicate_groups(&pool, date)
                            .await?
                    }
                };
                if groups.is_empty() {
                    println!("nincs duplikált meccs ezen a napon: {date}");
                } else {
                    let mut merges = 0usize;
                    let mut skipped = 0usize;
                    for group in &groups {
                        // Túlélő: nem deny-olt név; ha mindkettő/egyik sem,
                        // a legkorábbi sor.
                        let survivor = group
                            .matches
                            .iter()
                            .filter(|m| !deny_list.iter().any(|d| d == &m.competition_name))
                            .min_by_key(|m| m.created_at)
                            .or_else(|| group.matches.iter().min_by_key(|m| m.created_at))
                            .expect("csoport nem üres");
                        println!(
                            "csoport {} — túlélő: {} [{}]",
                            group.key, survivor.match_id, survivor.competition_name
                        );
                        for m in group
                            .matches
                            .iter()
                            .filter(|m| m.match_id != survivor.match_id)
                        {
                            println!(
                                "  áldozat: {} [{}] (prov={}, result={}, odds={}, gate={:?})",
                                m.match_id,
                                m.competition_name,
                                m.provenance_count,
                                m.has_result,
                                m.has_odds,
                                m.gate_state
                            );
                            if apply {
                                // Egy hibás/kockázatos pár ne állítsa meg a
                                // teljes napi takarítást: kihagyjuk, jelezzük.
                                let outcome: Result<()> = async {
                                    let same =
                                        football_storage::repositories::dedupe::is_same_match(
                                            &pool,
                                            survivor.match_id,
                                            m.match_id,
                                        )
                                        .await?;
                                    anyhow::ensure!(
                                        same,
                                        "a pár nem ugyanaz a meccs: {} / {}",
                                        survivor.match_id,
                                        m.match_id
                                    );
                                    football_storage::repositories::dedupe::merge_duplicate(
                                        &pool,
                                        survivor.match_id,
                                        m.match_id,
                                        "S16 dedupe (duplikált kanonikus meccs)",
                                    )
                                    .await?;
                                    Ok(())
                                }
                                .await;
                                match outcome {
                                    Ok(()) => merges += 1,
                                    Err(err) => {
                                        skipped += 1;
                                        println!("  kihagyva: {err:#}");
                                    }
                                }
                            }
                        }
                    }
                    println!(
                        "duplikált csoportok: {}, merge-ek: {}, kihagyva: {}{}",
                        groups.len(),
                        merges,
                        skipped,
                        if apply {
                            ""
                        } else {
                            " (dry-run — --apply a végrehajtáshoz)"
                        }
                    );
                }
            }
            DedupeAction::Aliases {
                from,
                to,
                apply,
                min_similarity,
            } => {
                let groups =
                    football_storage::repositories::dedupe::find_cross_source_alias_duplicates(
                        &pool,
                        from,
                        to,
                        min_similarity,
                    )
                    .await?;
                if groups.is_empty() {
                    println!(
                        "nincs cross-source névváltozat-duplikátum a {from}..{to} tartományban"
                    );
                    println!(
                        "csoportok: 0, csapat-merge-ek: 0, fixture-merge-ek: 0, fixture-twin merge-ek: 0, kihagyva: 0{}",
                        if apply {
                            ""
                        } else {
                            " (dry-run — --apply a végrehajtáshoz)"
                        }
                    );
                } else {
                    let mut team_merges = 0usize;
                    let mut fixture_merges = 0usize;
                    let mut fixture_twin_merges = 0usize;
                    let mut skipped = 0usize;
                    for group in &groups {
                        println!(
                            "csoport — {} ({} {}) | eltérő oldal: {}, hasonlóság {:.3}, bizonyíték: {}",
                            group.competition_name,
                            group.kickoff_at.date_naive(),
                            group.kickoff_at.time(),
                            group.side.as_str(),
                            group.similarity,
                            group.evidence.as_str()
                        );
                        println!(
                            "  túlélő: {} [{} - {}]",
                            group.survivor.match_id,
                            group.survivor.home_name,
                            group.survivor.away_name
                        );
                        println!(
                            "  áldozat: {} [{} - {}]",
                            group.duplicate.match_id,
                            group.duplicate.home_name,
                            group.duplicate.away_name
                        );
                        println!(
                            "  csapat-névváltozat: {} / {}",
                            group.survivor_team_name(),
                            group.duplicate_team_name()
                        );
                        println!("  bizonyíték: {}", group.evidence_text());
                        if !group.evidence.merges_team_entity() {
                            println!(
                                "  csapat-entitás NEM merge-elve (twin: csak a fixture olvad össze)"
                            );
                        }
                        if apply {
                            let outcome =
                                football_storage::repositories::dedupe::apply_cross_source_alias_group(
                                    &pool, group,
                                )
                                .await;
                            match outcome {
                                Ok(report) => {
                                    if group.evidence.merges_team_entity() {
                                        fixture_merges += 1;
                                        if report.merged_entity_id.is_some() {
                                            team_merges += 1;
                                        }
                                    } else {
                                        fixture_twin_merges += 1;
                                    }
                                    println!(
                                        "  merge kész: audit={} | csapat-merge={:?}",
                                        report.match_audit_id, report.merged_entity_id
                                    );
                                }
                                Err(err) => {
                                    skipped += 1;
                                    println!("  kihagyva: {err:#}");
                                }
                            }
                        } else if group.evidence.merges_team_entity() {
                            // Dry-run: a tervezett merge-ek osztály szerint.
                            fixture_merges += 1;
                            team_merges += 1;
                        } else {
                            fixture_twin_merges += 1;
                        }
                    }
                    println!(
                        "csoportok: {}, csapat-merge-ek: {}, fixture-merge-ek: {}, fixture-twin merge-ek: {}, kihagyva: {}{}",
                        groups.len(),
                        team_merges,
                        fixture_merges,
                        fixture_twin_merges,
                        skipped,
                        if apply {
                            ""
                        } else {
                            " (dry-run — --apply a végrehajtáshoz)"
                        }
                    );
                }
            }
            DedupeAction::Pair {
                survivor,
                duplicate,
                note,
            } => {
                let audit = football_storage::repositories::dedupe::merge_duplicate(
                    &pool, survivor, duplicate, &note,
                )
                .await?;
                println!("merge kész: {audit} | túlélő {survivor} | áldozat {duplicate}");
            }
            DedupeAction::Variants {
                date,
                to,
                apply,
                allow_loose,
                cross_competition,
            } => {
                // A forrás- és a megjelenítési nap eltérhet (a meccsek a helyi
                // időzóna szerint csoportosulnak, a lekérdezés UTC-napra bont),
                // ezért ±1 napos ablakban keresünk; a párosítás saját 12 órás
                // kickoff-korlátja továbbra is kizárja a következő napi meccseket.
                // Tartomány-módban (`--to`) a teljes intervallumot vizsgáljuk.
                let end_date = to;
                let (from, to) = match to {
                    Some(end) => (date, end),
                    None => (
                        date.pred_opt().unwrap_or(date),
                        date.succ_opt().unwrap_or(date),
                    ),
                };
                let groups =
                    football_storage::repositories::dedupe::variant_duplicate_groups_between(
                        &pool,
                        from,
                        to,
                        cross_competition,
                    )
                    .await?;
                if groups.is_empty() {
                    match (date, end_date) {
                        (start, Some(end)) => {
                            println!("nincs név-variáns duplikátum a tartományban: {start} – {end}")
                        }
                        (start, None) => {
                            println!("nincs név-variáns duplikátum ezen a napon: {start}")
                        }
                    }
                } else {
                    let mut merges = 0usize;
                    let mut skipped = 0usize;
                    for group in &groups {
                        // Túlélő: amelyiknél van eredmény, majd több
                        // provenance, végül a legkorábbi sor (determinisztikus).
                        // Cross-competition módban a verseny-ország/gát is
                        // számít: a verifikáltabb, jobb (kisebb) prioritású
                        // verseny-entitás marad.
                        let survivor = if cross_competition {
                            group
                                .matches
                                .iter()
                                .max_by_key(|m| {
                                    (
                                        m.has_result,
                                        m.has_odds,
                                        m.provenance_count,
                                        m.gate_state.as_deref() == Some("verified_publish"),
                                        std::cmp::Reverse(m.competition_priority),
                                        std::cmp::Reverse(m.created_at),
                                    )
                                })
                                .expect("csoport nem üres")
                        } else {
                            group
                                .matches
                                .iter()
                                .max_by_key(|m| {
                                    (
                                        m.has_result,
                                        m.has_odds,
                                        m.provenance_count,
                                        std::cmp::Reverse(m.created_at),
                                    )
                                })
                                .expect("csoport nem üres")
                        };
                        println!(
                            "csoport {} — túlélő: {} [{} {} - {}]",
                            group.key,
                            survivor.match_id,
                            survivor.competition_name,
                            survivor.home_name,
                            survivor.away_name
                        );
                        for m in group
                            .matches
                            .iter()
                            .filter(|m| m.match_id != survivor.match_id)
                        {
                            println!(
                                "  áldozat: {} [{} {} - {}] (prov={}, result={}, odds={})",
                                m.match_id,
                                m.competition_name,
                                m.home_name,
                                m.away_name,
                                m.provenance_count,
                                m.has_result,
                                m.has_odds
                            );
                            if apply {
                                let outcome: Result<()> = async {
                                    let same = if allow_loose {
                                        football_storage::repositories::dedupe::is_variant_same_match(
                                            &pool,
                                            survivor.match_id,
                                            m.match_id,
                                            cross_competition,
                                        )
                                        .await?
                                    } else {
                                        football_storage::repositories::dedupe::is_strict_variant_same_match(
                                            &pool,
                                            survivor.match_id,
                                            m.match_id,
                                            cross_competition,
                                        )
                                        .await?
                                    };
                                    anyhow::ensure!(
                                        same,
                                        "nem név-variáns pár (azonos nap, egyező vagy 2 órával eltérő kickoff és korosztály-jelölés nélküli név kell; cross-competition módban azonos normalizált verseny-ország és csapat-kontextus is; lazább kézi döntéshez: --allow-loose): {} / {}",
                                        survivor.match_id,
                                        m.match_id
                                    );
                                    let note = if allow_loose {
                                        "S37c dedupe (név-variáns duplikátum, kézi döntés)"
                                    } else {
                                        "S37c dedupe (név-variáns duplikátum)"
                                    };
                                    football_storage::repositories::dedupe::merge_duplicate(
                                        &pool,
                                        survivor.match_id,
                                        m.match_id,
                                        note,
                                    )
                                    .await?;
                                    Ok(())
                                }
                                .await;
                                match outcome {
                                    Ok(()) => merges += 1,
                                    Err(err) => {
                                        skipped += 1;
                                        println!("  kihagyva: {err:#}");
                                    }
                                }
                            }
                        }
                    }
                    println!(
                        "névvariáns-csoportok: {}, merge-ek: {}, kihagyva: {}{}",
                        groups.len(),
                        merges,
                        skipped,
                        if apply {
                            ""
                        } else {
                            " (dry-run — --apply a végrehajtáshoz)"
                        }
                    );
                }
            }
            DedupeAction::Fixtures {
                from,
                to,
                competition,
                apply,
                limit,
                review_note,
            } => {
                let groups = football_storage::repositories::dedupe::duplicate_fixture_groups(
                    &pool,
                    from,
                    to,
                    competition.as_deref(),
                )
                .await?;
                let mut merges = 0usize;
                let mut safe = 0usize;
                let mut risky = 0usize;
                'groups: for group in &groups {
                    // Túlélő: footballdata-provenance előbb, tie-break a
                    // legkorábbi created_at (S26 spec 3. pont).
                    let survivor = group
                        .matches
                        .iter()
                        .min_by_key(|m| {
                            (
                                !m.source_codes.split(',').any(|c| c == "footballdata"),
                                m.created_at,
                            )
                        })
                        .expect("csoport nem üres");
                    for m in group
                        .matches
                        .iter()
                        .filter(|m| m.match_id != survivor.match_id)
                    {
                        let mirrored = m.home_team_id == survivor.away_team_id
                            && m.away_team_id == survivor.home_team_id;
                        let results_ok = !m.has_result
                            || !survivor.has_result
                            || (m.home_ft == survivor.home_ft && m.away_ft == survivor.away_ft);
                        if mirrored || (!results_ok && review_note.is_none()) {
                            risky += 1;
                            println!(
                                "KOCKÁZAT [{}] {} — {} vs {} | {}: kickoff {} / {} | eredmény {:?}-{:?} / {:?}-{:?} | forrás {} / {} | ok: {}",
                                group.key,
                                m.competition_name,
                                m.home_name,
                                m.away_name,
                                m.kickoff_at.date_naive(),
                                survivor.kickoff_at,
                                m.kickoff_at,
                                survivor.home_ft,
                                survivor.away_ft,
                                m.home_ft,
                                m.away_ft,
                                survivor.source_codes,
                                m.source_codes,
                                if mirrored {
                                    "tükrözött orientáció"
                                } else {
                                    "eltérő eredmény"
                                }
                            );
                            continue;
                        }
                        safe += 1;
                        if apply
                            && let Some(lim) = limit
                            && merges >= lim
                        {
                            println!("merge-limit elérve: {lim}");
                            break 'groups;
                        }
                        if apply {
                            let outcome: Result<()> = async {
                                let same = if review_note.is_some() {
                                    football_storage::repositories::dedupe::same_fixture_identity(
                                        &pool,
                                        survivor.match_id,
                                        m.match_id,
                                    )
                                    .await?
                                } else {
                                    let same_fixture =
                                        football_storage::repositories::dedupe::is_same_fixture(
                                            &pool,
                                            survivor.match_id,
                                            m.match_id,
                                        )
                                        .await?;
                                    if same_fixture {
                                        true
                                    } else {
                                        // S97b: eltérő szezon, de PONTOSAN azonos
                                        // kickoff-pillanat és csapatpár — forrás-
                                        // feloldási hiba, nem valódi külön meccs.
                                        football_storage::repositories::dedupe::is_same_instant_fixture(
                                            &pool,
                                            survivor.match_id,
                                            m.match_id,
                                        )
                                        .await?
                                    }
                                };
                                anyhow::ensure!(
                                    same,
                                    "a pár nem ugyanaz a fixture: {} / {}",
                                    survivor.match_id,
                                    m.match_id
                                );
                                let note = match review_note.as_ref() {
                                    Some(review) => format!(
                                        "S26 felülvizsgált fixture-rekonciliáció: {} | kickoff {} → {} | eltérő eredmény a forrásokban: {:?}-{:?} vs {:?}-{:?} | a túlélő eredményét külső forrás igazolta",
                                        review,
                                        m.kickoff_at,
                                        survivor.kickoff_at,
                                        survivor.home_ft,
                                        survivor.away_ft,
                                        m.home_ft,
                                        m.away_ft
                                    ),
                                    None => format!(
                                        "S26 fixture-rekonciliáció: kickoff {} → {} (forrás-prioritás), eredmény egyezik",
                                        m.kickoff_at, survivor.kickoff_at
                                    ),
                                };
                                football_storage::repositories::dedupe::merge_duplicate(
                                    &pool,
                                    survivor.match_id,
                                    m.match_id,
                                    &note,
                                )
                                .await?;
                                Ok(())
                            }
                            .await;
                            match outcome {
                                Ok(()) => merges += 1,
                                Err(err) => {
                                    risky += 1;
                                    println!("  kihagyva: {err:#}");
                                }
                            }
                        }
                    }
                }
                // Dátumon átnyúló párok (kickoff <24h, de más UTC-nap) — a
                // nap-kulcsú csoportosítás ezeket nem látja; külön lista.
                let straddling: Vec<(String, String, String, String, String, Uuid, Uuid)> =
                    sqlx::query_as(
                        "WITH g AS (
                        SELECT competition_id, season_id,
                               LEAST(home_team_id, away_team_id) h,
                               GREATEST(home_team_id, away_team_id) a,
                               min(kickoff_at) k1, max(kickoff_at) k2
                          FROM core.match
                         GROUP BY 1, 2, 3, 4
                        HAVING count(*) = 2
                           AND min(kickoff_at)::date <> max(kickoff_at)::date
                           AND max(kickoff_at) - min(kickoff_at) < interval '24 hours'
                    )
                    SELECT c.name, th.name, ta.name, g.k1::text, g.k2::text,
                           m1.id, m2.id
                      FROM g
                      JOIN core.competition c ON c.id = g.competition_id
                      JOIN core.match m1 ON m1.competition_id = g.competition_id
                       AND m1.season_id = g.season_id AND m1.kickoff_at = g.k1
                       AND LEAST(m1.home_team_id,m1.away_team_id)=g.h
                       AND GREATEST(m1.home_team_id,m1.away_team_id)=g.a
                      JOIN core.match m2 ON m2.competition_id = g.competition_id
                       AND m2.season_id = g.season_id AND m2.kickoff_at = g.k2
                       AND LEAST(m2.home_team_id,m2.away_team_id)=g.h
                       AND GREATEST(m2.home_team_id,m2.away_team_id)=g.a
                      JOIN core.team th ON th.id = m1.home_team_id
                      JOIN core.team ta ON ta.id = m1.away_team_id",
                    )
                    .fetch_all(&pool)
                    .await?;
                for (comp, home, away, k1, k2, id1, id2) in &straddling {
                    risky += 1;
                    println!("KOCKÁZAT [dátumon átnyúló] {comp}: {home} vs {away} | {k1} / {k2}");
                    if apply && let Some(review) = review_note.as_ref() {
                        // A későbbi kickoff a halasztott/átütemezett fixture.
                        let same =
                            football_storage::repositories::dedupe::same_fixture_identity_any_day(
                                &pool, *id2, *id1,
                            )
                            .await?;
                        if !same {
                            println!("  kihagyva: nem azonos fixture ({id1} / {id2})");
                            continue;
                        }
                        football_storage::repositories::dedupe::merge_duplicate(
                                &pool,
                                *id2,
                                *id1,
                                &format!(
                                    "S26 felülvizsgált dátumon-átnyúló fixture: {review} | korábbi {k1} → halasztott {k2}"
                                ),
                            )
                            .await?;
                        merges += 1;
                        risky -= 1;
                    }
                }
                println!(
                    "fixture-csoportok: {}, biztonságos pár: {}, kockázatos: {}, merge-ek: {}{}",
                    groups.len(),
                    safe,
                    risky,
                    merges,
                    if apply {
                        ""
                    } else {
                        " (dry-run — --apply a végrehajtáshoz)"
                    }
                );
            }
        },
        Command::Competitions { action } => match action {
            CompetitionsAction::RepairCountries { from, to, apply } => {
                competitions::run_repair_countries(&pool, from, to, apply).await?;
            }
        },
        Command::Integrity { from, to, json } => {
            let r = football_storage::repositories::integrity::report(&pool, from, to).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&r)?);
            } else {
                println!("duplikált kickoff-csoport: {}", r.duplicate_kickoff_groups);
                println!("duplikált fixture-csoport: {}", r.duplicate_fixture_groups);
                println!(
                    "FELOLDATLAN cross-source duplikátum: {}",
                    r.unresolved_duplicate_fixture_groups
                );
                println!(
                    "lejárt, scheduled, eredmény nélkül: {}",
                    r.stale_scheduled_without_result
                );
                println!("lezárt, eredmény nélkül: {}", r.missing_finished_result);
                println!(
                    "azonos csapatpár azonos pillanatban: {}",
                    r.same_pair_same_instant_groups
                );
                println!(
                    "azonos csapatpár azonos napon: {}",
                    r.same_pair_same_day_groups
                );
                println!("árva provenance: {}", r.orphan_provenance);
                println!("aktív ref összevont entitásra: {}", r.active_refs_to_merged);
                println!("függő review: {}", r.pending_reviews);
                println!("withheld gate: {}", r.withheld_gates);
                println!("failed job: {}", r.failed_jobs);
                println!("nyitott breaker: {}", r.open_breakers);
                println!(
                    "verdict: {}",
                    if r.has_hard_failures() {
                        "FAIL (kemény integritási hiba)"
                    } else if r.has_data_gaps() {
                        "OK — de ADATHIÁNY (lásd a fenti számokat)"
                    } else {
                        "OK"
                    }
                );
            }
            if r.has_hard_failures() {
                anyhow::bail!("integritási riport: kemény hiba");
            }
        }
        Command::Elo { as_of } => {
            let as_of = as_of.unwrap_or_else(|| Utc::now().date_naive());
            let results: Vec<(Uuid, Uuid, i16, i16)> = sqlx::query_as(
                "SELECT m.home_team_id, m.away_team_id, r.home_ft, r.away_ft
                 FROM core.match m
                 JOIN core.match_result r ON r.match_id = m.id
                 WHERE m.status = 'finished'
                   AND r.home_ft IS NOT NULL AND r.away_ft IS NOT NULL
                 ORDER BY m.kickoff_at",
            )
            .fetch_all(&pool)
            .await?;
            let mut ratings: HashMap<Uuid, f64> = HashMap::new();
            for (home, away, hs, as_) in &results {
                let h = *ratings.entry(*home).or_insert(football_ingest::elo::START);
                let a = *ratings.entry(*away).or_insert(football_ingest::elo::START);
                let (nh, na) = football_ingest::elo::update(h, a, *hs, *as_);
                ratings.insert(*home, nh);
                ratings.insert(*away, na);
            }
            let teams: Vec<(Uuid, String, String)> = sqlx::query_as(
                "SELECT t.id, e.canonical_name, coalesce(c.code, '')
                 FROM core.team t
                 JOIN core.entity e ON e.id = t.entity_id
                 LEFT JOIN core.country c ON c.id = t.country_id",
            )
            .fetch_all(&pool)
            .await?;
            let now = Utc::now();
            let mut written = 0usize;
            for (id, name, code) in &teams {
                let Some(rating) = ratings.get(id) else {
                    continue;
                };
                sqlx::query(
                    "INSERT INTO fact.club_elo
                     (country_code, club_name, club_entity_id, as_of, elo, provenance, source, captured_at)
                     VALUES ($1,$2,$3,$4,$5,'observed','own-elo',$6)
                     ON CONFLICT (country_code, club_name, as_of) DO UPDATE SET
                       elo = EXCLUDED.elo,
                       club_entity_id = EXCLUDED.club_entity_id,
                       captured_at = EXCLUDED.captured_at",
                )
                .bind(code)
                .bind(name)
                .bind(id)
                .bind(as_of)
                .bind(rating)
                .bind(now)
                .execute(&pool)
                .await?;
                written += 1;
            }
            println!(
                "Elo számítva: {} meccsből, {} csapat ratingje mentve (as_of {as_of})",
                results.len(),
                written
            );
        }
        Command::DataStatus { from, to, top } => {
            #[allow(clippy::type_complexity)]
            let rows: Vec<(String, String, i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
                "SELECT c.name, coalesce(co.code,''),
                        count(DISTINCT s.id)::bigint, count(*)::bigint,
                        count(r.match_id)::bigint,
                        count(*) FILTER (WHERE EXISTS (
                            SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id=m.id))::bigint,
                        count(*) FILTER (WHERE g.state='verified_publish')::bigint,
                        count(*) FILTER (WHERE g.state='provisional_publish')::bigint,
                        count(*) FILTER (WHERE g.state='withheld')::bigint
                 FROM core.match m
                 JOIN core.competition c ON c.id=m.competition_id
                 JOIN core.season s ON s.id=m.season_id
                 LEFT JOIN core.country co ON co.id=c.country_id
                 LEFT JOIN core.match_result r ON r.match_id=m.id
                 LEFT JOIN LATERAL (
                     SELECT state FROM ops.publish_gate pg
                     WHERE pg.match_id=m.id ORDER BY publish_date DESC LIMIT 1
                 ) g ON true
                 WHERE ($1::date IS NULL OR m.kickoff_at::date >= $1)
                   AND ($2::date IS NULL OR m.kickoff_at::date <= $2)
                 GROUP BY 1,2
                 ORDER BY 4 DESC
                 LIMIT $3",
            )
            .bind(from)
            .bind(to)
            .bind(top)
            .fetch_all(&pool)
            .await?;
            println!(
                "verseny | ország | szezon | meccs | eredmény % | odds % | verified | provisional | withheld"
            );
            for (
                name,
                code,
                seasons,
                matches,
                with_result,
                with_odds,
                verified,
                provisional,
                withheld,
            ) in &rows
            {
                println!(
                    "{} | {} | {} | {} | {:.1}% | {:.1}% | {} | {} | {}",
                    name,
                    if code.is_empty() { "-" } else { code },
                    seasons,
                    matches,
                    *with_result as f64 / *matches as f64 * 100.0,
                    *with_odds as f64 / *matches as f64 * 100.0,
                    verified,
                    provisional,
                    withheld
                );
            }

            let (total, verified, provisional, withheld): (i64, i64, i64, i64) = sqlx::query_as(
                "SELECT count(*)::bigint,
                        count(*) FILTER (WHERE g.state='verified_publish')::bigint,
                        count(*) FILTER (WHERE g.state='provisional_publish')::bigint,
                        count(*) FILTER (WHERE g.state='withheld')::bigint
                 FROM core.match m
                 LEFT JOIN LATERAL (
                     SELECT state FROM ops.publish_gate pg
                     WHERE pg.match_id=m.id ORDER BY publish_date DESC LIMIT 1
                 ) g ON true
                 WHERE ($1::date IS NULL OR m.kickoff_at::date >= $1)
                   AND ($2::date IS NULL OR m.kickoff_at::date <= $2)",
            )
            .bind(from)
            .bind(to)
            .fetch_one(&pool)
            .await?;
            let dup_groups: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM (
                    SELECT 1 FROM core.match
                    WHERE ($1::date IS NULL OR kickoff_at::date >= $1)
                      AND ($2::date IS NULL OR kickoff_at::date <= $2)
                    GROUP BY LEAST(home_team_id, away_team_id),
                             GREATEST(home_team_id, away_team_id), kickoff_at
                    HAVING count(*) > 1
                 ) d",
            )
            .bind(from)
            .bind(to)
            .fetch_one(&pool)
            .await?;
            let reviews: Vec<(String, i64)> = sqlx::query_as(
                "SELECT entity_kind, count(*)::bigint FROM identity.review_queue
                 WHERE status='pending' GROUP BY 1 ORDER BY 2 DESC",
            )
            .fetch_all(&pool)
            .await?;
            println!(
                "összesen: {total} meccs | verified {verified} | provisional {provisional} | withheld {withheld} | duplikált csoport {dup_groups}"
            );
            if reviews.is_empty() {
                println!("pending review: 0");
            } else {
                let summary: Vec<String> = reviews
                    .iter()
                    .map(|(kind, count)| format!("{kind} {count}"))
                    .collect();
                println!("pending review: {}", summary.join(", "));
            }
        }
        Command::MatchInsightImport { from, to, apply } => {
            let api_dir = PathBuf::from(std::env::var("FOOTBALL_RESEARCH_API_DIR").unwrap_or_else(
                |_| {
                    format!(
                        "{}/Library/Application Support/FootballResearch/api/v1",
                        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
                    )
                },
            ));
            let report = match_insight_import::run(&pool, &api_dir, from, to, apply).await?;
            println!(
                "match-insight {from}..{to}: {} fájl / {} meccs / {} írt sor{}",
                report.files,
                report.matches,
                report.written,
                if report.applied { "" } else { " (dry-run)" }
            );
            for skip in report.skipped.iter().take(10) {
                println!("  kihagyva: {skip}");
            }
        }
        Command::PlayerImport { from, to, apply } => {
            let api_dir = PathBuf::from(std::env::var("FOOTBALL_RESEARCH_API_DIR").unwrap_or_else(
                |_| {
                    format!(
                        "{}/Library/Application Support/FootballResearch/api/v1",
                        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
                    )
                },
            ));
            let report = player_import::run(&pool, &api_dir, from, to, apply).await?;
            println!(
                "player import {from}..{to}: {} fájl / {} meccs / {} játékos létrehozva, \
                 {} lineup / {} stat / {} lövés / {} tagság sor{}",
                report.files,
                report.matches,
                report.players_created,
                report.lineup_rows,
                report.stat_rows,
                report.shot_rows,
                report.spell_rows,
                if report.applied {
                    ""
                } else {
                    " (dry-run — nem történt írás)"
                }
            );
            for skip in report.skipped.iter().take(10) {
                println!("  kihagyva: {skip}");
            }
        }
        Command::TipplapReplay { from, to, apply } => {
            let data_dir =
                PathBuf::from(std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string()));
            let report = tipplap_replay::run(&pool, from, to, apply, &data_dir).await?;
            println!(
                "tipplap replay {from}..{to}: {} nap, {} observation, {} új sor, {} verseny materializálva{}",
                report.days,
                report.parsed,
                report.inserted_new,
                report.materialized_competitions,
                if report.applied {
                    ""
                } else {
                    " (dry-run — nem történt írás)"
                }
            );
            for skip in &report.skipped {
                println!("  kihagyva: {skip}");
            }
        }
        Command::Reverify { from, to, limit } => {
            let ids: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM core.match
                 WHERE kickoff_at::date BETWEEN $1 AND $2
                 ORDER BY kickoff_at
                 LIMIT $3",
            )
            .bind(from)
            .bind(to)
            .bind(limit.unwrap_or(i64::MAX))
            .fetch_all(&pool)
            .await?;
            let mut done = 0usize;
            let mut failed = 0usize;
            for id in &ids {
                match football_worker::tick::reverify_match(&pool, *id).await {
                    Ok(()) => done += 1,
                    Err(err) => {
                        failed += 1;
                        eprintln!("reverify hiba ({id}): {err:#}");
                    }
                }
            }
            println!("újraellenőrzés: {done} meccs kész, {failed} hiba");
            let states: Vec<(String, i64)> = sqlx::query_as(
                "SELECT g.state, count(*)::bigint
                 FROM ops.publish_gate g JOIN core.match m ON m.id=g.match_id
                 WHERE m.kickoff_at::date BETWEEN $1 AND $2
                 GROUP BY 1 ORDER BY 2 DESC",
            )
            .bind(from)
            .bind(to)
            .fetch_all(&pool)
            .await?;
            println!("gate-állapotok:");
            for (state, count) in states {
                println!("  {state}: {count}");
            }
        }
        Command::CoverageDay { from, to } => {
            let rows = football_storage::repositories::coverage::fetch_daily_coverage(
                &pool,
                from,
                to,
                "Europe/Budapest",
            )
            .await?;
            println!(
                "nap         meccs  lezárt  eredm.  odds  logó  idő nélkül  withheld  verified"
            );
            let mut totals = [0i64; 8];
            for row in &rows {
                println!(
                    "{}  {:>5}  {:>6}  {:>6}  {:>4}  {:>4}  {:>10}  {:>8}  {:>8}",
                    row.day,
                    row.matches,
                    row.finished,
                    row.with_result,
                    row.with_odds,
                    row.with_both_logos,
                    row.missing_kickoff,
                    row.withheld,
                    row.verified
                );
                for (slot, value) in totals.iter_mut().zip([
                    row.matches,
                    row.finished,
                    row.with_result,
                    row.with_odds,
                    row.with_both_logos,
                    row.missing_kickoff,
                    row.withheld,
                    row.verified,
                ]) {
                    *slot += value;
                }
            }
            println!(
                "ÖSSZESEN    {:>5}  {:>6}  {:>6}  {:>4}  {:>4}  {:>10}  {:>8}  {:>8}",
                totals[0],
                totals[1],
                totals[2],
                totals[3],
                totals[4],
                totals[5],
                totals[6],
                totals[7]
            );
        }
        Command::Coverage {
            from,
            to,
            limit,
            competition,
        } => {
            let rows: Vec<(String, String, i64, i64, i64)> = sqlx::query_as(
                "SELECT c.name, s.label, count(*)::bigint,
                        count(r.match_id)::bigint,
                        count(*) FILTER (WHERE EXISTS (
                            SELECT 1 FROM fact.odds_snapshot o WHERE o.match_id = m.id))::bigint
                 FROM core.match m
                 JOIN core.competition c ON c.id = m.competition_id
                 JOIN core.season s ON s.id = m.season_id
                 LEFT JOIN core.match_result r ON r.match_id = m.id
                 WHERE ($1::int IS NULL OR substring(s.label from 1 for 4)::int >= $1)
                   AND ($2::int IS NULL OR substring(s.label from 1 for 4)::int <= $2)
                   AND ($3::text IS NULL OR c.name = $3)
                 GROUP BY 1, 2
                 ORDER BY 3 DESC
                 LIMIT $4",
            )
            .bind(from)
            .bind(to)
            .bind(competition)
            .bind(limit)
            .fetch_all(&pool)
            .await?;
            println!("verseny | szezon | meccs | eredmény % | odds %");
            let (mut total, mut res, mut odds) = (0i64, 0i64, 0i64);
            for (name, season, matches, with_result, with_odds) in &rows {
                total += matches;
                res += with_result;
                odds += with_odds;
                println!(
                    "{} | {} | {} | {:.1}% | {:.1}%",
                    name,
                    season,
                    matches,
                    *with_result as f64 / *matches as f64 * 100.0,
                    *with_odds as f64 / *matches as f64 * 100.0
                );
            }
            if total > 0 {
                println!(
                    "összesen: {} meccs | eredmény {:.1}% | odds {:.1}%",
                    total,
                    res as f64 / total as f64 * 100.0,
                    odds as f64 / total as f64 * 100.0
                );
            } else {
                println!("(nincs adat a szűrésre)");
            }
        }
        Command::Conflicts { status, limit } => {
            let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
                "SELECT c.subject_id, c.field_key, c.status,
                        concat_ws(' — ', ht.name, at.name)
                 FROM fact.conflict c
                 JOIN core.match m ON m.id=c.subject_id
                 JOIN core.team ht ON ht.id=m.home_team_id
                 JOIN core.team at ON at.id=m.away_team_id
                 WHERE c.subject_kind='match' AND c.status=$1
                 ORDER BY c.opened_at DESC LIMIT $2",
            )
            .bind(&status)
            .bind(limit)
            .fetch_all(&pool)
            .await?;
            println!("meccskonfliktusok ({status}): {}", rows.len());
            for (id, field, state, pair) in rows {
                println!("  {id} | {field} | {state} | {pair}");
            }
        }
        Command::ResolveConflict {
            match_id,
            field,
            winner,
            reason,
        } => {
            let id = football_verification::conflict::resolve_conflict(
                &pool, match_id, &field, winner, &reason,
            )
            .await?;
            println!("feloldva: {id} | meccs {match_id} | mező {field} | győztes {winner}");
        }
        Command::Backfill { action } => match action {
            BackfillAction::V1Import {
                tournament,
                from_year,
                to_year,
                target_competition,
                apply,
            } => {
                run_v1_import(
                    &pool,
                    &store,
                    tournament,
                    from_year,
                    to_year,
                    target_competition,
                    apply,
                )
                .await?;
            }
            BackfillAction::V1Pilot {
                tournament,
                from_year,
                to_year,
            } => {
                let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
                let v1_url = format!("postgres://{user}@127.0.0.1/football_app");
                let v1 = PgPoolOptions::new()
                    .max_connections(2)
                    .connect(&v1_url)
                    .await
                    .context("v1 adatbázis kapcsolat (football_app)")?;
                let (name, matches, with_odds): (String, i64, i64) = sqlx::query_as(
                    "SELECT t.name, count(*)::bigint,
                            count(*) FILTER (WHERE EXISTS (
                                SELECT 1 FROM match_odds o WHERE o.match_id=m.id))::bigint
                     FROM matches m
                     JOIN tournaments t ON t.id=m.tournament_id
                     JOIN seasons s ON s.id=m.season_id
                     WHERE t.id=$1 AND s.year_start BETWEEN $2 AND $3
                     GROUP BY 1",
                )
                .bind(tournament)
                .bind(from_year)
                .bind(to_year)
                .fetch_optional(&v1)
                .await
                .context("v1 tournament összesítés")?
                .unwrap_or(("(nincs ilyen tournament)".into(), 0, 0));
                println!(
                    "v1 {}: {} meccs, {} oddsszal ({:.1}%)",
                    name,
                    matches,
                    with_odds,
                    if matches > 0 {
                        with_odds as f64 / matches as f64 * 100.0
                    } else {
                        0.0
                    }
                );

                let v1_teams: Vec<(String, String, String)> = sqlx::query_as(
                    "SELECT DISTINCT t.name, coalesce(c.iso2,''), coalesce(t.gender,'M')
                     FROM teams t
                     JOIN matches m ON m.home_team_id=t.id OR m.away_team_id=t.id
                     JOIN seasons s ON s.id=m.season_id
                     LEFT JOIN countries c ON c.id=t.country_id
                     WHERE m.tournament_id=$1 AND s.year_start BETWEEN $2 AND $3",
                )
                .bind(tournament)
                .bind(from_year)
                .bind(to_year)
                .fetch_all(&v1)
                .await
                .context("v1 csapatok")?;

                let v2 = identity_repo::all_team_entities(&pool).await?;
                let mut by_key: HashMap<(String, String), usize> = HashMap::new();
                for row in &v2 {
                    let key = (
                        football_domain::identity::fold_name(&row.canonical_name),
                        row.country_code.clone().unwrap_or_default(),
                    );
                    *by_key.entry(key).or_insert(0) += 1;
                }
                let mut resolved = 0usize;
                let mut unresolved: Vec<String> = Vec::new();
                for (name, iso2, gender) in &v1_teams {
                    let key = (football_domain::identity::fold_name(name), iso2.clone());
                    if by_key.get(&key) == Some(&1) {
                        resolved += 1;
                    } else {
                        let _ = gender;
                        unresolved.push(format!("{name} ({iso2})"));
                    }
                }
                println!(
                    "csapatok: {} egyedi, v2-ben egyértelműen illeszkedő: {} ({:.1}%)",
                    v1_teams.len(),
                    resolved,
                    if v1_teams.is_empty() {
                        0.0
                    } else {
                        resolved as f64 / v1_teams.len() as f64 * 100.0
                    }
                );
                if !unresolved.is_empty() {
                    println!("illesztetlen minta (max 20):");
                    for name in unresolved.iter().take(20) {
                        println!("  {name}");
                    }
                }

                let markets: Vec<(String, String, i64)> = sqlx::query_as(
                    "SELECT o.market, o.outcome, count(*)::bigint
                     FROM match_odds o
                     JOIN matches m ON m.id=o.match_id
                     JOIN seasons s ON s.id=m.season_id
                     WHERE m.tournament_id=$1 AND s.year_start BETWEEN $2 AND $3
                     GROUP BY 1,2 ORDER BY 3 DESC LIMIT 8",
                )
                .bind(tournament)
                .bind(from_year)
                .bind(to_year)
                .fetch_all(&v1)
                .await
                .context("v1 odds-piacok")?;
                println!("odds-piacok (v1):");
                for (market, outcome, count) in markets {
                    println!("  {market} {outcome}: {count}");
                }
            }
            BackfillAction::V1Inventory {
                from_year,
                to_year,
                top,
            } => {
                let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
                let v1_url = format!("postgres://{user}@127.0.0.1/football_app");
                let v1 = PgPoolOptions::new()
                    .max_connections(2)
                    .connect(&v1_url)
                    .await
                    .context("v1 adatbázis kapcsolat (football_app)")?;
                let v1_rows: Vec<(String, String, i32, i64, i64, i64)> = sqlx::query_as(
                    "SELECT t.name, coalesce(c.iso2, ''), s.year_start::int, count(*)::bigint,
                            count(*) FILTER (WHERE m.home_score IS NOT NULL)::bigint,
                            count(*) FILTER (WHERE EXISTS (
                                SELECT 1 FROM match_odds o WHERE o.match_id = m.id))::bigint
                     FROM matches m
                     JOIN tournaments t ON t.id = m.tournament_id
                     JOIN seasons s ON s.id = m.season_id
                     LEFT JOIN countries c ON c.id = t.country_id
                     WHERE s.year_start BETWEEN $1 AND $2
                     GROUP BY 1, 2, 3
                     ORDER BY 4 DESC
                     LIMIT $3",
                )
                .bind(from_year)
                .bind(to_year)
                .bind(top)
                .fetch_all(&v1)
                .await
                .context("v1 lefedettség lekérés")?;

                // v2: foldolt verseny-név + szezon-kezdőév → meccsszám.
                let v2_rows: Vec<(String, String, String, i64)> = sqlx::query_as(
                    "SELECT c.name, coalesce(co.code, ''), s.label, count(*)::bigint
                     FROM core.match m
                     JOIN core.competition c ON c.id = m.competition_id
                     JOIN core.season s ON s.id = m.season_id
                     LEFT JOIN core.country co ON co.id = c.country_id
                     GROUP BY 1, 2, 3",
                )
                .fetch_all(&pool)
                .await?;
                let mut v2_map: HashMap<(String, String, i32), i64> = HashMap::new();
                for (name, code, label, matches) in v2_rows {
                    let year = label
                        .get(..4)
                        .and_then(|y| y.parse::<i32>().ok())
                        .unwrap_or(0);
                    *v2_map
                        .entry((football_domain::identity::fold_name(&name), code, year))
                        .or_insert(0) += matches;
                }

                println!(
                    "v1 verseny | ország | szezon | v1 meccs | v1 odds % | v2 meccs | megjegyzés"
                );
                let (mut total, mut matched) = (0i64, 0i64);
                for (name, country, year, matches, with_result, with_odds) in &v1_rows {
                    let key = (
                        football_domain::identity::fold_name(name),
                        country.clone(),
                        *year,
                    );
                    let in_v2 = v2_map.get(&key).copied().unwrap_or(0);
                    total += matches;
                    matched += (*matches).min(in_v2);
                    let note = if in_v2 == 0 {
                        "v2-ben nincs".to_string()
                    } else if in_v2 >= *matches {
                        "lefedve".to_string()
                    } else {
                        format!("résleges ({} hiány)", matches - in_v2)
                    };
                    println!(
                        "{} | {} | {}/{} | {} | {:.1}% | {} | {}",
                        name,
                        if country.is_empty() { "-" } else { country },
                        year,
                        year + 1,
                        matches,
                        *with_odds as f64 / *matches as f64 * 100.0,
                        in_v2,
                        note
                    );
                    let _ = with_result;
                }
                if total > 0 {
                    println!(
                        "összesen: v1 {} meccs (top {}), v2-ben visszaigazolt ~{} ({:.1}%)",
                        total,
                        v1_rows.len(),
                        matched,
                        matched as f64 / total as f64 * 100.0
                    );
                }
            }

            BackfillAction::Plan {
                source,
                from,
                to,
                leagues,
                new_leagues,
                apply,
            } => {
                if source != "footballdata" {
                    anyhow::bail!(
                        "a backfill-tervezés egyelőre csak a footballdata forrást támogatja"
                    );
                }
                if new_leagues {
                    let mut new_jobs = 0usize;
                    for code in football_ingest::footballdata::new_league_codes() {
                        if !apply {
                            println!("  terv: new-{code} (minden szezon)");
                            new_jobs += 1;
                            continue;
                        }
                        new_jobs += worker_repo::insert_job_counted(
                            &pool,
                            &worker_repo::NewJob {
                                source_slug: "footballdata".into(),
                                domain: "backfill".into(),
                                entity_key: format!("new-{code}"),
                                event_type: "backfill".into(),
                                time_window: "all".into(),
                                due_at: Utc::now(),
                                priority: 200,
                            },
                        )
                        .await?;
                    }
                    println!(
                        "new-liga backfill terv: {} job{}{}",
                        new_jobs,
                        if apply {
                            " sorba állítva"
                        } else {
                            " (dry-run)"
                        },
                        if apply {
                            ""
                        } else {
                            " — --apply a sorba állításhoz"
                        }
                    );
                }
                let (Some(from), Some(to)) = (from, to) else {
                    if new_leagues {
                        return Ok(());
                    }
                    anyhow::bail!("a szezonos backfillhez --from és --to kell");
                };
                let codes: Vec<String> = if leagues == "all" {
                    football_ingest::footballdata::league_codes()
                        .into_iter()
                        .map(str::to_string)
                        .collect()
                } else {
                    leagues
                        .split(',')
                        .map(|c| c.trim().to_ascii_lowercase())
                        .filter(|c| !c.is_empty())
                        .collect()
                };
                let mut planned = 0usize;
                let mut skipped = 0usize;
                for code in &codes {
                    let first = football_ingest::footballdata::first_season(code)
                        .ok_or_else(|| anyhow::anyhow!("ismeretlen liga-kód: {code}"))?;
                    for start in from..=to {
                        if start < first {
                            skipped += 1;
                            continue;
                        }
                        let season = format!("{:02}{:02}", start % 100, (start + 1) % 100);
                        let label = football_ingest::footballdata::season_label_from_code(&season)
                            .expect("érvényes szezonkód");
                        if !apply {
                            if planned < 6 {
                                println!("  terv: {code} {label} ({season})");
                            }
                            planned += 1;
                            continue;
                        }
                        let inserted = worker_repo::insert_job_counted(
                            &pool,
                            &worker_repo::NewJob {
                                source_slug: "footballdata".into(),
                                domain: "backfill".into(),
                                entity_key: code.clone(),
                                event_type: "backfill".into(),
                                time_window: season,
                                due_at: Utc::now(),
                                priority: 200,
                            },
                        )
                        .await?;
                        planned += inserted;
                    }
                }
                println!(
                    "backfill terv: {} job{}, {} kihagyott (a liga még nem létezett){}",
                    planned,
                    if apply {
                        " sorba állítva"
                    } else {
                        " (dry-run)"
                    },
                    skipped,
                    if apply {
                        ""
                    } else {
                        " — --apply a sorba állításhoz"
                    }
                );
            }
        },
        Command::Asset { action } => match action {
            AssetAction::Import { file } => {
                let rows: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(&file)?)?;
                let now = Utc::now();
                let mut written = 0usize;
                for row in &rows {
                    let field = |k: &str| row.get(k).and_then(|v| v.as_str());
                    // Helyi fájl (v1-audit logó) → tartalom-címzett blob a
                    // nyers objektumtárba; a storage_key kerül az asset-sorba.
                    let storage_key = match field("file") {
                        Some(file) => {
                            let bytes = std::fs::read(file).map_err(|e| {
                                anyhow::anyhow!("asset fájl olvasása ({file}): {e}")
                            })?;
                            let ext = std::path::Path::new(file)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("bin");
                            let stored = store.put_blob(&bytes, ext)?;
                            if let Some(expected) = field("sha256") {
                                anyhow::ensure!(
                                    expected.eq_ignore_ascii_case(&stored.sha256),
                                    "sha256 eltérés: {file} (várt {expected}, kapott {})",
                                    stored.sha256
                                );
                            }
                            Some(stored.storage_key)
                        }
                        None => field("storage_key").map(str::to_string),
                    };
                    let input = worker_repo::AssetRowInput {
                        entity_kind: field("entity_kind")
                            .ok_or_else(|| anyhow::anyhow!("hiányzó entity_kind"))?,
                        entity_id: field("entity_id").and_then(|v| v.parse::<Uuid>().ok()),
                        kind: field("kind").ok_or_else(|| anyhow::anyhow!("hiányzó kind"))?,
                        url: field("url").ok_or_else(|| anyhow::anyhow!("hiányzó url"))?,
                        storage_key: storage_key.as_deref(),
                        source: field("source").ok_or_else(|| anyhow::anyhow!("hiányzó source"))?,
                        licence: field("licence"),
                        licence_status: field("licence_status").unwrap_or("unknown"),
                        attribution: field("attribution"),
                        captured_at: field("captured_at")
                            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
                            .map(|d| d.with_timezone(&Utc))
                            .unwrap_or(now),
                    };
                    worker_repo::upsert_asset(&pool, &input).await?;
                    written += 1;
                }
                println!("asset import: {written} sor feldolgozva");
            }
        },
        // Az AI-parancsok a pool előtt, külön ágban futottak.
        Command::Ai { .. } => unreachable!("az AI-parancs a pool előtt fut"),
    }

    Ok(())
}

/// Worker állapot: policy-k (slug/mód/engedélyezve), mai ledger-szám a napi
/// kerethez, breaker-állapotok, függő jobok és karanténos coverage-slotok.
async fn worker_status(pool: &sqlx::PgPool) -> Result<()> {
    let policies: Vec<(String, String, bool, i32)> = sqlx::query_as(
        "SELECT source_slug, mode, enabled, daily_budget
         FROM ops.source_policy ORDER BY source_slug",
    )
    .fetch_all(pool)
    .await?;
    let today = Utc::now().date_naive();
    println!("forráspolicy-k:");
    if policies.is_empty() {
        println!("  (nincs policy)");
    }
    for (slug, mode, enabled, budget) in &policies {
        let attempts = worker_repo::count_fetch_attempts_today(pool, slug, today).await?;
        let breaker = worker_repo::get_breaker(pool, slug)
            .await?
            .map(|row| row.state.as_db_str())
            .unwrap_or("closed");
        let enabled_text = if *enabled { "igen" } else { "nem" };
        println!(
            "  {slug} mód={mode} engedélyezve={enabled_text} ma={attempts}/{budget} breaker={breaker}"
        );
    }
    let counts = worker_repo::worker_status_counts(pool).await?;
    println!(
        "függő jobok: {} (leased: {})",
        counts.jobs_pending, counts.jobs_leased
    );
    println!("karanténban: {}", counts.quarantined);
    println!("függő identitás-review: {}", counts.reviews_pending);
    Ok(())
}

/// Egyszeri worker kör kézi indítása: job-generálás + tick a DATA_DIR/worker-fixtures
/// fixture-eivel és a DemoAdapter registry-vel. Shadow policy esetén csak
/// observationök íródnak, kanonikus adat nem.
async fn worker_tick(pool: &sqlx::PgPool, store: &ObjectStore, data_dir: &str) -> Result<()> {
    let fixture_dir = std::path::Path::new(data_dir).join("worker-fixtures");
    let fixture = FixtureFetcher::from_dir(&fixture_dir)?;
    let fetchers = Fetchers::new(RustHttpFetcher::new("football-app-v2")?, fixture)
        .with_local_file(&football_ingest::research::default_api_root());
    // Canary/élő Soccerway-körhöz a CLI is indíthat Obscura-szervert
    // (csak engedélyezett obscura policy esetén; a feature mögött).
    #[cfg(feature = "obscura")]
    let fetchers = {
        let mut fetchers = fetchers;
        if worker_repo::has_enabled_transport(pool, Transport::Obscura.as_db_str()).await? {
            let cfg = football_ingest::obscura::ObscuraConfig::from_env();
            let fetcher = football_ingest::obscura::ObscuraFetcher::connect_managed(&cfg).await?;
            fetchers = fetchers.with_obscura(fetcher);
            println!("obscura fetcher aktív (CLI mód, önjavító)");
        }
        fetchers
    };
    let mut adapters: HashMap<String, Box<dyn SourceAdapter>> = HashMap::new();
    adapters.insert("demo-synthetic".into(), Box::new(DemoAdapter));
    adapters.insert("footballdata".into(), Box::new(FootballdataAdapter));
    adapters.insert(
        football_ingest::research::SOURCE_SLUG.into(),
        Box::new(football_ingest::research::ResearchAdapter::default()),
    );
    adapters.insert("fotmob".into(), Box::new(FotMobAdapter::default()));
    adapters.insert("soccerway".into(), Box::new(SoccerwayAdapter::default()));
    adapters.insert("openfoot".into(), Box::new(OpenFootAdapter::from_env()));
    adapters.insert(
        "tipplap".into(),
        Box::new(football_ingest::tipplap::TipplapAdapter),
    );
    let now = Utc::now();
    let policies = worker_repo::list_enabled_policies(pool).await?;
    let generated = scheduler::generate_jobs(pool, &policies, now).await?;
    let outcome = tick::tick(pool, store, &fetchers, &adapters, uuid::Uuid::now_v7(), now).await?;
    println!(
        "generált={generated} feldolgozott={} blokkolt={} sikertelen={}",
        outcome.processed, outcome.blocked, outcome.failed
    );
    Ok(())
}

/// Identitás review-életciklus: lista, jóváhagyás, elutasítás, alias-lista.
/// A döntéseket a football-identity review modulja hajtja végre tranzakcióban,
/// auditálva; a CLI döntője "cli". A hibaüzenetek magyar nyelvűek.
async fn identity_command(pool: &sqlx::PgPool, action: IdentityAction) -> Result<()> {
    match action {
        IdentityAction::Review { status } => {
            if let Some(s) = &status
                && !matches!(s.as_str(), "pending" | "approved" | "rejected")
            {
                anyhow::bail!("Érvénytelen státusz: {s} (lehetséges: pending, approved, rejected)");
            }
            let rows = identity_repo::list_reviews(pool, status.as_deref()).await?;
            let sources = identity_repo::list_sources(pool).await?;
            let source_codes: HashMap<Uuid, String> = sources.into_iter().collect();
            println!("id | forrás | external ID | név | ok | jelöltek száma | létrehozva");
            if rows.is_empty() {
                println!("(nincs felülvizsgálati sor)");
            }
            for r in rows {
                let source = r
                    .source_id
                    .and_then(|id| source_codes.get(&id).cloned())
                    .unwrap_or_else(|| "-".into());
                let candidates = r.candidate_entity_ids.as_array().map_or(0, |a| a.len());
                println!(
                    "{} | {} | {} | {} | {} | {} | {}",
                    r.id,
                    source,
                    r.source_entity_id,
                    r.proposed_name,
                    r.reason,
                    candidates,
                    r.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                );
            }
        }
        IdentityAction::Duplicates => {
            let groups = identity_merge::duplicate_team_groups(pool).await?;
            if groups.is_empty() {
                println!("(nincs duplikált csapat-entitás)");
            }
            for group in groups {
                println!(
                    "{} | ország: {} | nem: {} | korosztály: {} | ág: {}",
                    group.name,
                    group.country_code.as_deref().unwrap_or("-"),
                    group.gender.as_deref().unwrap_or("-"),
                    group.age_group.as_deref().unwrap_or("-"),
                    group.branch.as_deref().unwrap_or("-")
                );
                for team in &group.teams {
                    println!(
                        "  entitás: {} | team: {} | meccsek: {} | létre: {}",
                        team.entity_id,
                        team.team_id,
                        team.matches,
                        team.created_at.format("%Y-%m-%d %H:%M")
                    );
                }
            }
        }
        IdentityAction::MergeTeam {
            from,
            to,
            note,
            apply,
            via_alias,
        } => {
            let preview =
                identity_merge::merge_team_preview(pool, to, from, via_alias.is_none()).await?;
            println!(
                "merge: {} ← {} | meccsek: {} | lehetséges duplikált meccs: {} | alias: {} | ref: {} | tabella: {} | szezon-stat: {} | asset: {}",
                preview.survivor_name,
                preview.duplicate_name,
                preview.matches_moved,
                preview.potential_duplicate_matches,
                preview.aliases,
                preview.refs,
                preview.standings,
                preview.season_stats,
                preview.assets
            );
            if !apply {
                println!("dry-run: nincs írás (--apply a merge-hez)");
                return Ok(());
            }
            let report = match via_alias {
                Some(evidence) => {
                    identity_merge::merge_team_via_alias(pool, to, from, &evidence).await?
                }
                None => identity_merge::merge_team(pool, to, from, &note).await?,
            };
            println!(
                "merge kész: meccsek {} | alias {} (eldobva {}) | ref {} | tabella {} (eldobva {}) | szezon-stat {} (eldobva {}) | asset {} (eldobva {}) | merge utáni duplikált meccsek: {}",
                report.matches_moved,
                report.aliases_moved,
                report.aliases_dropped,
                report.refs_moved,
                report.standings_moved,
                report.standings_dropped,
                report.season_stats_moved,
                report.season_stats_dropped,
                report.assets_moved,
                report.assets_dropped,
                report.duplicate_matches
            );
        }
        IdentityAction::MergeCompetition {
            from,
            into,
            note,
            apply,
            force,
        } => {
            let preview =
                competition_merge::competition_merge_preview(pool, into, from, force).await?;
            if preview.already_merged {
                println!(
                    "a beolvadó verseny már a survivorba van merge-elve: {from} → {into} (nincs írás)"
                );
                return Ok(());
            }
            println!(
                "merge: {} ← {} | meccsek: {} (survivor: {}) | season: {} (+{} ütköző) | alias: {} | ref: {}",
                preview.survivor_name,
                preview.duplicate_name,
                preview.matches_to_move,
                preview.survivor_matches,
                preview.seasons_moved,
                preview.seasons_merged,
                preview.aliases,
                preview.refs
            );
            if !apply {
                println!("dry-run: nincs írás (--apply a merge-hez)");
                return Ok(());
            }
            match competition_merge::merge_competition(pool, into, from, &note, force).await? {
                competition_merge::CompetitionMergeOutcome::Merged(report) => {
                    println!(
                        "verseny-merge kész: meccsek {} | season {} (ütköző: {}) | alias {} (eldobva {}) | ref {} | merge utáni duplikált meccscsoportok: {}",
                        report.matches_moved,
                        report.seasons_moved,
                        report.seasons_merged,
                        report.aliases_moved,
                        report.aliases_dropped,
                        report.refs_moved,
                        report.duplicate_match_groups
                    );
                }
                competition_merge::CompetitionMergeOutcome::AlreadyMerged { .. } => {
                    println!("a beolvadó verseny már a survivorba van merge-elve (nincs írás)");
                }
            }
        }
        IdentityAction::RebuildRefs { source, apply } => {
            let report =
                football_identity::repair::rebuild_source_refs(pool, &source, !apply).await?;
            println!(
                "crosswalk-újraépítés [{}]{}: {} kód, {} változatlan, {} átkötés, {} új csapat, {} bizonytalan, {} obs nélkül",
                report.source,
                if apply { "" } else { " (dry-run)" },
                report.codes_seen,
                report.unchanged,
                report.relinked,
                report.teams_created,
                report.ambiguous,
                report.no_obs
            );
            for (code, old, new) in &report.changes {
                println!(
                    "  {}: {} → {}",
                    code,
                    old.as_deref().unwrap_or("(nincs)"),
                    new
                );
            }
            if !apply {
                println!("(nem történt írás; futtasd --apply kapcsolóval)");
            }
        }
        IdentityAction::RepairMatches {
            from,
            to,
            apply,
            withhold_unresolved,
        } => {
            let report = football_identity::repair::repair_soccerway_matches(
                pool,
                from,
                to,
                !apply,
                withhold_unresolved,
            )
            .await?;
            println!(
                "meccs-javítás [{}..{}){}: {} vizsgált, {} már helyes, {} átállítva, {} merge, {} visszatartva, {} feloldhatatlan, {} hiba",
                from,
                to,
                if apply { "" } else { " (dry-run)" },
                report.matches_scanned,
                report.already_correct,
                report.repaired,
                report.merged,
                report.withheld,
                report.unresolved,
                report.errors.len()
            );
            for (match_id, err) in &report.errors {
                println!("  hiba: {match_id} — {err}");
            }
            if !apply {
                println!("(nem történt írás; futtasd --apply kapcsolóval)");
            }
        }
        IdentityAction::CrosswalkCheck => {
            let conflicts = football_identity::crosswalk_conflicts(pool).await?;
            if conflicts.is_empty() {
                println!("crosswalk: nincs konfliktus");
            } else {
                println!("crosswalk konfliktusok:");
                for c in &conflicts {
                    println!(
                        "  {} | {} -> {}",
                        c.left_source,
                        c.left_external_id,
                        c.conflicting_right_external_ids.join(", ")
                    );
                }
                anyhow::bail!(
                    "{} ambiguous crosswalk-kötés (import előtt javítandó)",
                    conflicts.len()
                );
            }
        }
        IdentityAction::Approve { id, entity, note } => {
            football_identity::review::approve(pool, id, entity, "cli", note.as_deref()).await?;
            println!("jóváhagyva: {id} → {entity}");
        }
        IdentityAction::ApproveCreateAll {
            allow_unknown_country,
        } => {
            let (approved, failed) = football_identity::review::approve_create_pending_with(
                pool,
                "cli",
                allow_unknown_country,
            )
            .await?;
            println!(
                "batch approve-create: {} jóváhagyva, {} hiba",
                approved.len(),
                failed.len()
            );
            for (queue_id, err) in &failed {
                println!("  hiba: {queue_id} — {err}");
            }
            if !failed.is_empty() {
                anyhow::bail!("{} sor nem lett jóváhagyva", failed.len());
            }
        }
        IdentityAction::ApproveCreate { id, note } => {
            let entity =
                football_identity::review::approve_create(pool, id, "cli", note.as_deref()).await?;
            println!("létrehozva és jóváhagyva: {id} → {entity}");
        }
        IdentityAction::Reject { id, note } => {
            football_identity::review::reject(pool, id, "cli", &note).await?;
            println!("elutasítva: {id}");
        }
        IdentityAction::RejectAllPending { note } => {
            if note.trim().is_empty() {
                anyhow::bail!("a jegyzet nem lehet üres");
            }
            let rows = identity_repo::list_reviews(pool, Some("pending")).await?;
            let mut rejected = 0usize;
            let mut failed = 0usize;
            for row in rows {
                match football_identity::review::reject(pool, row.id, "cli", &note).await {
                    Ok(()) => rejected += 1,
                    Err(err) => {
                        failed += 1;
                        println!("  hiba: {} — {err}", row.id);
                    }
                }
            }
            println!("batch reject: {rejected} elutasítva, {failed} hiba");
            if failed > 0 {
                anyhow::bail!("{failed} sor nem lett elutasítva");
            }
        }
        IdentityAction::ReviewSummary => {
            let rows = identity_repo::review_summary(pool).await?;
            if rows.is_empty() {
                println!("(nincs függő review)");
            } else {
                println!("nem | ág | verseny | db | országok | minta");
                for row in &rows {
                    println!(
                        "{} | {} | {} | {} | {} | {}",
                        row.gender.as_deref().unwrap_or("-"),
                        row.branch.as_deref().unwrap_or("-"),
                        row.competition.as_deref().unwrap_or("-"),
                        row.count,
                        row.countries.as_deref().unwrap_or("-"),
                        row.sample_names.as_deref().unwrap_or("")
                    );
                }
            }
        }
        IdentityAction::SeedV1 {
            v1_db,
            apply,
            limit,
        } => {
            seed_v1(pool, &v1_db, apply, limit).await?;
        }
        IdentityAction::ListAliases { entity } => {
            let rows = identity_repo::list_aliases(pool, entity).await?;
            println!("entitás | alias | nyelv | ország | státusz");
            if rows.is_empty() {
                println!("(nincs alias)");
            }
            for a in rows {
                println!(
                    "{} | {} | {} | {} | {}",
                    a.entity_id,
                    a.alias,
                    a.lang,
                    a.country_code.as_deref().unwrap_or("-"),
                    a.status
                );
            }
        }
    }
    Ok(())
}

/// v1 crosswalk-seed: a v1 csapatnevek/aliasok → reviewed v2 aliasok.
/// A v1 adatbázis csak olvasott; ismeretlen/többértelmű csapatot nem seedel.
async fn seed_v1(pool: &sqlx::PgPool, v1_db: &str, apply: bool, limit: Option<i64>) -> Result<()> {
    let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
    let v1_url = format!("postgres://{user}@127.0.0.1/{v1_db}");
    let v1 = PgPoolOptions::new()
        .max_connections(2)
        .connect(&v1_url)
        .await
        .with_context(|| format!("v1 adatbázis kapcsolat: {v1_db}"))?;

    let iso3_map: HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT iso3, iso2 FROM countries WHERE iso3 IS NOT NULL AND iso2 IS NOT NULL",
    )
    .fetch_all(&v1)
    .await
    .context("v1 ország-iso3 térkép")?
    .into_iter()
    .collect();

    let team_rows: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.name, c.iso2, t.gender
         FROM teams t
         LEFT JOIN countries c ON c.id = t.country_id
         ORDER BY t.id
         LIMIT $1",
    )
    .bind(limit.unwrap_or(i64::MAX))
    .fetch_all(&v1)
    .await
    .context("v1 csapatok")?;

    let alias_rows: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT team_id, alias, language FROM team_aliases ORDER BY id")
            .fetch_all(&v1)
            .await
            .context("v1 csapat-aliasok")?;

    let mut aliases_by_team: HashMap<i64, Vec<(String, String)>> = HashMap::new();
    for (team_id, alias, lang) in alias_rows {
        aliases_by_team
            .entry(team_id)
            .or_default()
            .push((alias, lang.unwrap_or_else(|| "en".into())));
    }

    let mut v1_teams = Vec::with_capacity(team_rows.len());
    for (id, name, iso2, gender) in team_rows {
        let mut names: Vec<(String, String)> = vec![(name, "en".into())];
        names.extend(aliases_by_team.remove(&id).unwrap_or_default());
        let mut seen = std::collections::HashSet::new();
        names.retain(|(n, _)| seen.insert(football_domain::identity::fold_name(n)));
        v1_teams.push(football_identity::v1_seed::V1Team {
            names,
            country_code: iso2,
            gender: gender.map(|g| match g.as_str() {
                "M" => "male".to_string(),
                "F" => "female".to_string(),
                other => other.to_lowercase(),
            }),
        });
    }

    // A v2 országkód néha ISO3 (HUN/GER); az egyezéshez ISO2-re vetítjük.
    let mut v2 = identity_repo::all_team_entities(pool).await?;
    for row in &mut v2 {
        if let Some(code) = &row.country_code
            && code.len() == 3
            && let Some(iso2) = iso3_map.get(code)
        {
            row.country_code = Some(iso2.clone());
        }
    }

    let (seeds, stats) = football_identity::v1_seed::plan_seeds(&v2, &v1_teams);
    println!(
        "v1 csapatok: {} | egyező: {} | többértelmű: {} | nincs találat: {} | alias-nevek: {}",
        stats.v1_teams, stats.matched, stats.ambiguous, stats.unmatched, stats.seeded_names
    );
    for seed in seeds.iter().take(8) {
        println!("  seed: {} → {}", seed.alias, seed.entity_id);
    }
    if !apply {
        println!("dry-run: nincs írás (--apply a beszúráshoz)");
        return Ok(());
    }
    let inserted = identity_repo::insert_alias_seeds(pool, &seeds).await?;
    println!("beszúrt reviewed alias: {inserted}");
    Ok(())
}

/// A meccs két csapatának neve (SAME_TEAM_BOTH_SIDES szabály); hiányzó sor → None.
/// Csapatnév + ország-kód a verification-szabályokhoz.
#[derive(Default)]
struct TeamContextInfo {
    home_name: Option<String>,
    away_name: Option<String>,
    home_country_code: Option<String>,
    away_country_code: Option<String>,
}

async fn load_team_context(
    pool: &sqlx::PgPool,
    home_team_id: Uuid,
    away_team_id: Uuid,
) -> Result<TeamContextInfo> {
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT t.id, t.name, c.code
           FROM core.team t
           LEFT JOIN core.country c ON c.id = t.country_id
          WHERE t.id = ANY($1)",
    )
    .bind(vec![home_team_id, away_team_id])
    .fetch_all(pool)
    .await?;
    let mut info = TeamContextInfo::default();
    for (id, name, country_code) in rows {
        if id == home_team_id {
            info.home_name = Some(name);
            info.home_country_code = country_code;
        } else if id == away_team_id {
            info.away_name = Some(name);
            info.away_country_code = country_code;
        }
    }
    Ok(info)
}

/// A meccs versenyének ország-kódja (FOREIGN_TEAMS_DOMESTIC_COMPETITION).
async fn load_competition_country(pool: &sqlx::PgPool, match_id: Uuid) -> Result<Option<String>> {
    let row: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT ctry.code, c.name
           FROM core.match m
           JOIN core.competition c ON c.id = m.competition_id
           LEFT JOIN core.country ctry ON ctry.id = c.country_id
          WHERE m.id = $1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await?;
    let Some((code, name)) = row else {
        return Ok(None);
    };
    // Kupákban külföldi résztvevők is lehetnek: itt ne tüzeljen a
    // FOREIGN_TEAMS_DOMESTIC_COMPETITION szabály.
    if football_domain::identity::looks_like_cup_competition(&name) {
        return Ok(None);
    }
    Ok(code)
}

async fn duplicate_count(
    pool: &sqlx::PgPool,
    materialized: &football_ingest::canonicalize::MaterializedMatch,
) -> Result<i64> {
    sqlx::query_scalar(
        "SELECT count(*) FROM core.match
         WHERE competition_id = (SELECT competition_id FROM core.match WHERE id=$1)
           AND season_id = (SELECT season_id FROM core.match WHERE id=$1)
           AND round_label IS NOT DISTINCT FROM (SELECT round_label FROM core.match WHERE id=$1)
           AND home_team_id = $2 AND away_team_id = $3
           AND kickoff_at = $4
           AND id <> $1",
    )
    .bind(materialized.match_id)
    .bind(materialized.home_team_id)
    .bind(materialized.away_team_id)
    .bind(materialized.snapshot.kickoff_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

/// v1 (football_app) import: mérkőzések + 1X2 odds a normál observation →
/// materializer → trust/gate láncon. Csak olvassa a v1 adatbázist.
type V1MatchRow = (i64, String, i64, i64, chrono::DateTime<Utc>, String, String);
type V1ResultRow = (
    i64,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
);
type V1ResultEntry = (
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
    Option<i16>,
);
type V1OddsRow = (i64, String, String, Option<f64>, Option<f64>);

async fn run_v1_import(
    pool: &sqlx::PgPool,
    store: &ObjectStore,
    tournament: i64,
    from_year: i32,
    to_year: i32,
    target_competition: Option<uuid::Uuid>,
    apply: bool,
) -> Result<()> {
    let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
    let v1_url = format!("postgres://{user}@127.0.0.1/football_app");
    let v1 = PgPoolOptions::new()
        .max_connections(4)
        .connect(&v1_url)
        .await
        .context("v1 adatbázis kapcsolat (football_app)")?;

    let (name, iso2, country_name): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT t.name, c.iso2, c.name FROM tournaments t
         LEFT JOIN countries c ON c.id=t.country_id WHERE t.id=$1",
    )
    .bind(tournament)
    .fetch_one(&v1)
    .await
    .context("v1 tournament")?;

    let teams: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT t.id, t.name, c.iso2, t.gender
         FROM teams t
         LEFT JOIN countries c ON c.id=t.country_id
         JOIN matches m ON m.home_team_id=t.id OR m.away_team_id=t.id
         JOIN seasons s ON s.id=m.season_id
         WHERE m.tournament_id=$1 AND s.year_start BETWEEN $2 AND $3",
    )
    .bind(tournament)
    .bind(from_year)
    .bind(to_year)
    .fetch_all(&v1)
    .await
    .context("v1 csapatok")?;

    let matches: Vec<V1MatchRow> = sqlx::query_as(
        "SELECT m.id, s.name, m.home_team_id, m.away_team_id, m.kickoff_ts,
                    coalesce(m.kickoff_precision,'exact'), m.status
             FROM matches m JOIN seasons s ON s.id=m.season_id
             WHERE m.tournament_id=$1 AND s.year_start BETWEEN $2 AND $3
             ORDER BY m.kickoff_ts",
    )
    .bind(tournament)
    .bind(from_year)
    .bind(to_year)
    .fetch_all(&v1)
    .await
    .context("v1 meccsek")?;

    let results: Vec<V1ResultRow> = sqlx::query_as(
        "SELECT m.id, m.home_score, m.away_score, m.home_score_ht, m.away_score_ht,
                m.home_score_et, m.away_score_et, m.home_score_pen, m.away_score_pen
         FROM matches m JOIN seasons s ON s.id=m.season_id
         WHERE m.tournament_id=$1 AND s.year_start BETWEEN $2 AND $3",
    )
    .bind(tournament)
    .bind(from_year)
    .bind(to_year)
    .fetch_all(&v1)
    .await
    .context("v1 eredmények")?;
    let results: HashMap<i64, V1ResultEntry> = results
        .into_iter()
        .map(|(id, h, a, hh, ah, he, ae, hp, ap)| (id, (h, a, hh, ah, he, ae, hp, ap)))
        .collect();

    let odds: Vec<V1OddsRow> = sqlx::query_as(
        "SELECT o.match_id, o.bookmaker, o.outcome,
                o.opening_odds::float8, o.closing_odds::float8
         FROM match_odds o
         JOIN matches m ON m.id=o.match_id
         JOIN seasons s ON s.id=m.season_id
         WHERE o.market='1X2' AND m.tournament_id=$1 AND s.year_start BETWEEN $2 AND $3",
    )
    .bind(tournament)
    .bind(from_year)
    .bind(to_year)
    .fetch_all(&v1)
    .await
    .context("v1 odds")?;
    let mut odds_by_match: HashMap<i64, Vec<serde_json::Value>> = HashMap::new();
    for (match_id, bookmaker, outcome, opening, closing) in odds {
        let (value, phase) = match (closing, opening) {
            (Some(v), _) if v > 1.0 => (v, "closing"),
            (_, Some(v)) if v > 1.0 => (v, "opening"),
            _ => continue,
        };
        let outcome = match outcome.as_str() {
            "H" => "home",
            "D" => "draw",
            "A" => "away",
            _ => continue,
        };
        odds_by_match
            .entry(match_id)
            .or_default()
            .push(serde_json::json!({
                "bookmaker": bookmaker,
                "market": "1x2",
                "outcome": outcome,
                "line": 0.0,
                "phase": phase,
                "price": value,
            }));
    }

    let captured_at = Utc::now();
    let mut comp_country = iso2.clone().unwrap_or_default();
    let mut comp_country_name = country_name.clone().unwrap_or_default();
    let mut comp_name = name.clone();
    let mut comp_id: Option<uuid::Uuid> = None;
    if let Some(target) = target_competition {
        let (tname, tcode, tcountry): (String, String, String) = sqlx::query_as(
            "SELECT c.name, coalesce(k.code,''), coalesce(k.name,'')
             FROM core.competition c LEFT JOIN core.country k ON k.id=c.country_id
             WHERE c.id=$1",
        )
        .bind(target)
        .fetch_one(pool)
        .await
        .context("target competition")?;
        comp_id = Some(target);
        comp_name = tname;
        comp_country = tcode;
        comp_country_name = tcountry;
    }
    let mut observations: Vec<football_ingest::adapter::Observation> = Vec::new();
    let mut skipped_status = 0usize;
    let mut skipped_team = 0usize;
    let team_ids: std::collections::HashSet<i64> = teams.iter().map(|(id, ..)| *id).collect();

    for (team_id, team_name, team_iso2, gender) in &teams {
        observations.push(football_ingest::adapter::Observation {
            entity_kind: "team".into(),
            aspect: "team_identity".into(),
            natural_key: format!("v1t-{team_id}"),
            source_record_key: format!("v1t-{team_id}"),
            observed_at: captured_at,
            payload: serde_json::json!({
                "id": null,
                "source_entity_id": format!("v1t-{team_id}"),
                "name": team_name,
                "country_code": team_iso2.clone().unwrap_or_default(),
                "gender": match gender.as_deref() { Some("F") => "female", _ => "male" },
                "age_group": "senior",
                "branch": "senior",
            }),
        });
    }

    for (match_id, season_name, home_id, away_id, kickoff_ts, precision, status) in &matches {
        if !team_ids.contains(home_id) || !team_ids.contains(away_id) {
            skipped_team += 1;
            continue;
        }
        let status = match status.as_str() {
            "cancelled" => "canceled",
            "finished" | "scheduled" | "postponed" | "canceled" | "abandoned" | "awarded"
            | "live" => status.as_str(),
            other => {
                tracing::warn!(match_id, other, "v1 import: ismeretlen státusz, kihagyva");
                skipped_status += 1;
                continue;
            }
        };
        let kickoff_at = if precision == "date_only" {
            let date = kickoff_ts.date_naive();
            Utc.from_utc_datetime(&date.and_hms_opt(12, 0, 0).expect("12:00 érvényes"))
        } else {
            *kickoff_ts
        };
        let result = results
            .get(match_id)
            .and_then(|(h, a, hh, ah, he, ae, hp, ap)| match (status, h, a) {
                ("finished", Some(h), Some(a)) => {
                    let decided_by = if hp.is_some() && ap.is_some() {
                        "pens"
                    } else if he.is_some() && ae.is_some() {
                        "et"
                    } else {
                        "ft"
                    };
                    Some(serde_json::json!({
                        "home_ft": h, "away_ft": a,
                        "home_ht": hh, "away_ht": ah,
                        "home_et": he, "away_et": ae,
                        "home_pens": hp, "away_pens": ap,
                        "decided_by": decided_by,
                    }))
                }
                _ => None,
            });
        let source_match_id = format!("v1-{match_id}");
        observations.push(football_ingest::adapter::Observation {
            entity_kind: "match".into(),
            aspect: "match_core".into(),
            natural_key: source_match_id.clone(),
            source_record_key: source_match_id.clone(),
            observed_at: captured_at,
            payload: serde_json::json!({
                "canonical_match_id": null,
                "source_home_id": format!("v1t-{home_id}"),
                "source_away_id": format!("v1t-{away_id}"),
                "kickoff_at": kickoff_at,
                "kickoff_precision": if precision == "date_only" { "date_only" } else { "exact" },
                "status": status,
                "result": result,
                "round_label": null,
                "competition_timezone": "UTC",
                "competition": {
                    "id": comp_id,
                    "name": comp_name.clone(),
                    "country_code": comp_country.clone(),
                    "country_name": comp_country_name.clone(),
                    "tier": 1,
                    "priority": 60,
                },
                "season": {
                    "id": null,
                    "label": football_ingest::footballdata::normalize_season_label(season_name),
                },
                "source_code": "v1-import",
                "captured_at": captured_at,
            }),
        });
        if let Some(quotes) = odds_by_match.get(match_id) {
            let mut merged: Vec<serde_json::Value> = Vec::new();
            let mut seen: std::collections::HashSet<(String, String, String)> =
                std::collections::HashSet::new();
            for q in quotes {
                let key = (
                    q["bookmaker"].as_str().unwrap_or_default().to_string(),
                    q["outcome"].as_str().unwrap_or_default().to_string(),
                    q["phase"].as_str().unwrap_or_default().to_string(),
                );
                if seen.insert(key) {
                    merged.push(q.clone());
                }
            }
            if !merged.is_empty() {
                observations.push(football_ingest::adapter::Observation {
                    entity_kind: "match".into(),
                    aspect: "match_odds".into(),
                    natural_key: source_match_id.clone(),
                    source_record_key: source_match_id,
                    observed_at: captured_at,
                    payload: serde_json::json!({
                        "canonical_match_id": null,
                        "quotes": merged,
                        "market_count": 1,
                    }),
                });
            }
        }
    }

    println!(
        "v1 import terv: {} (ország: {}) | {} meccs, {} csapat, {} oddsolt meccs, kihagyva: {} státusz, {} csapat-hiány",
        comp_name,
        if comp_country.is_empty() {
            "-"
        } else {
            &comp_country
        },
        matches.len(),
        teams.len(),
        odds_by_match.len(),
        skipped_status,
        skipped_team
    );
    if !apply {
        println!("dry-run: nincs írás (--apply az importhoz)");
        return Ok(());
    }

    // Nyers megőrzés: a v1 export manifesztje (audit + provenance-lánc).
    let manifest = serde_json::json!({
        "source": "football_app (v1, read-only)",
        "tournament_id": tournament,
        "tournament": name,
        "target_competition": comp_id,
        "from_year": from_year,
        "to_year": to_year,
        "matches": matches.len(),
        "teams": teams.len(),
        "odds_matches": odds_by_match.len(),
        "imported_at": captured_at,
    });
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let artifact = store
        .put_blob(&manifest_bytes, "json")
        .context("v1 import manifeszt mentése")?;
    let capture = football_storage::repositories::worker::RawCapture {
        endpoint_key: "v1-import",
        request_url: "postgres://football_app (read-only)",
        request_key: &format!("v1-import:{tournament}:{from_year}-{to_year}"),
        requested_at: captured_at,
        received_at: captured_at,
        http_status: 200,
        adapter_version: "1.0",
        parser_version: "1.0",
    };
    let (source_id, capture_id) = football_storage::repositories::worker::persist_raw(
        pool,
        "v1-import",
        "v1 (football_app) import",
        &artifact,
        &capture,
        "application/json",
    )
    .await
    .context("v1 import raw megőrzés")?;

    for obs in &observations {
        football_ingest::observe::insert_observation(
            pool,
            source_id,
            capture_id,
            &obs.entity_kind,
            &obs.aspect,
            &obs.natural_key,
            &obs.source_record_key,
            obs.observed_at,
            obs.payload.clone(),
        )
        .await?;
    }

    let days = football_ingest::normalized::NormalizedDay::grouped_from_observations(&observations)
        .context("v1 import napok csoportosítása")?;
    let mut materialized = 0usize;
    for day in &days {
        football_worker::tick::verify_and_publish(pool, day).await?;
        materialized += 1;
    }
    println!(
        "v1 import kész: {} verseny-szezon csoport feldolgozva (forrás: {source_id})",
        materialized
    );
    Ok(())
}

/// Helyi AI hívások: `search` nyers találatok, `ask` tool-hurok a modelllel.
async fn run_ai(action: AiAction) -> Result<()> {
    let config = football_ai::AiConfig::from_env().map_err(|e| anyhow::anyhow!("{e}"))?;
    match action {
        AiAction::Search { query, limit } => {
            let client = football_ai::SearxngClient::new(&config.searxng_url, config.timeout)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            let results = client
                .search(&query, limit)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("találatok: {}", results.len());
            for (index, result) in results.iter().enumerate() {
                println!("{}. {}", index + 1, result.title);
                println!("   {}", result.url);
                if !result.content.is_empty() {
                    println!("   {}", result.content);
                }
            }
        }
        AiAction::Ask {
            question,
            max_steps,
        } => {
            let agent = football_ai::AiAgent::new(&config).map_err(|e| anyhow::anyhow!("{e}"))?;
            let agent = match max_steps {
                Some(steps) => agent.max_steps(steps),
                None => agent,
            };
            let answer = agent
                .ask(&question)
                .await
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            println!("modell: {}", agent.model_name());
            println!("válasz:\n{}", answer.answer);
            if !answer.sources.is_empty() {
                println!("\nforrások:");
                for (index, source) in answer.sources.iter().enumerate() {
                    println!("{}. {} — {}", index + 1, source.title, source.url);
                }
            }
            for step in &answer.steps {
                println!(
                    "\ntool: {} ({} találat) — {}",
                    step.tool, step.results, step.query
                );
            }
        }
    }
    Ok(())
}

/// S47: az `audit` parancs magyar nyelvű, emberi olvasásra szánt kimenete.
fn audit_report_print(report: &audit_repo::DuplicateAuditReport) {
    println!(
        "Duplikátum-audit {} .. {} — {} meccs",
        report.from, report.to, report.matches_scanned
    );
    let mut by_reason: std::collections::BTreeMap<&str, Vec<&audit_repo::AuditCluster>> =
        std::collections::BTreeMap::new();
    for cluster in &report.clusters {
        by_reason
            .entry(cluster.reason.as_str())
            .or_default()
            .push(cluster);
    }
    for (reason, clusters) in &by_reason {
        println!("  {reason} klaszter: {}", clusters.len());
        for cluster in clusters.iter().take(20) {
            for row in &cluster.matches {
                println!(
                    "    {}  {:<28} {} – {}  [{}] gate={}",
                    row.kickoff_at.format("%m-%d %H:%M"),
                    truncate(&row.competition_name, 28),
                    row.home_name,
                    row.away_name,
                    row.match_id,
                    row.gate_state.as_deref().unwrap_or("-")
                );
            }
        }
        if clusters.len() > 20 {
            println!("    … és további {} klaszter", clusters.len() - 20);
        }
    }
    println!(
        "  dupla szereplés (ugyanaz a csapat 6 órán belül): {}",
        report.double_bookings.len()
    );
    for booking in &report.double_bookings {
        println!("    {} [{}]", booking.team_name, booking.team_id);
        for row in &booking.matches {
            println!(
                "      {}  {:<24} {} – {}  [{}]",
                row.kickoff_at.format("%m-%d %H:%M"),
                truncate(&row.competition_name, 24),
                row.home_name,
                row.away_name,
                row.match_id
            );
        }
    }
    println!(
        "  verseny-duplikátum: {}",
        report.competition_duplicates.len()
    );
    for duplicate in &report.competition_duplicates {
        let names: Vec<String> = duplicate
            .competitions
            .iter()
            .map(|c| {
                format!(
                    "{} [{}] ({} meccs)",
                    c.name,
                    c.country_code.as_deref().unwrap_or("-"),
                    c.match_count
                )
            })
            .collect();
        println!("    {}: {}", duplicate.name_key, names.join(" | "));
    }
    let consistency = &report.consistency;
    println!(
        "  konzisztencia: lezárt eredmény nélkül {}, eredmény nem terminális státusszal {}, visszatartva {}, ország nélküli csapattal {}",
        consistency.finished_without_result,
        consistency.result_without_terminal_status,
        consistency.withheld,
        consistency.matches_with_countryless_team
    );
    if report.is_clean() {
        println!("EREDMÉNY: TISZTA");
    } else {
        println!("EREDMÉNY: {} javítandó találat", report.finding_count());
    }
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
