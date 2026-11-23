# 2026-11-23 — Meccsrészletező gazdagítása research adatokkal

## Státusz

Jóváhagyott tervezési irány. A research export hibája és hiányai az S112/S112b szeletben javítva; ez a dokumentum a v2 oldali felhasználást írja le.

## Cél és hatókör

A v2 `/v1/match/{id}` válasza és a SwiftUI meccsrészletező jelenleg csak a kanonikus Meccs-adatokat és hat odds-mezőt mutat. A helyi research `api/v1/matches/**` dokumentumok ennél lényegesen többet tartalmaznak: magyar preview, Poisson-előrejelzés, H2H-streak, tabella-pillanatkép, szezonstatisztika, sérülések, fan-poll, teljes stadion-adatok és ~45 fogadási piac.

A cél: ezek **ellenőrzötten, forrással és rögzítési idővel** elérhetővé tenni a v2 API-ban, és a kliensben progresszíven megjeleníteni — úgy, hogy a generált/nem ellenőrzött tartalom egyértelműen elkülönüljön a forrás-tényektől.

Nem része a rendszernek: valódi fogadás, fogadóiroda-integráció, automatikus kifizetés (AGENTS.md §7).

## Forrásmezők és bizalmi modell

| Szekció | Research mező | jelleg | as-of |
|---|---|---|---|
| Preview (hu) | `preview_hu` (`generated: true`, `verified: false`, `source: derived:api/v1`) | **Számított, nem ellenőrzött** | `generated_at` |
| Előrejelzés | `prediction` (`model: poisson_v1`, `note: not a source fact`) | **Számított, nem ellenőrzött** | `generated_at` |
| H2H-streak | `head_to_head.streaks` | Számított a H2H-listából | `generated_at` |
| Tabella | `standings` (`snapshot_kind`, `snapshot_at`) | Forrás-pillanatkép | `snapshot_at` |
| Szezonstatisztika | `season_statistics` (`source: fotmob_table`) | Forrás-tény | `generated_at` |
| Sérülések | `injuries`, `lineups[].unavailable` | Forrás-tény | `generated_at` |
| Fan-poll | `fan_poll.questions` | Forrás-tény, de opcionális | `generated_at` |
| Teljes odds | `odds.markets_raw`, `odds.by_bookmaker`, `odds.markets_source` | Forrás-tény, bookmaker-forrással | `odds.observed_at` |
| Stadion-részletek | `match.venue` (kapacitás, borítás, koordináták) | Forrás-tény | `generated_at` |

A felületnek szakaszszinten kell jeleznie a számított/nem ellenőrzött jelleget, és az összesített adatminőségi sorban ismételnie kell.

## Séma

Új migráció: `db/migrations/0027_match_insight.sql`.

```text
fact.match_insight
  match_id          uuid NOT NULL -> core.match(id) ON DELETE CASCADE
  source_id         uuid NOT NULL -> ops.source(id)
  captured_at       timestamptz NOT NULL        -- research meta.generated_at
  preview_hu        jsonb NULL
  prediction        jsonb NULL
  h2h_streaks       jsonb NULL
  standings         jsonb NULL
  season_stats      jsonb NULL
  injuries          jsonb NULL
  fan_poll          jsonb NULL
  odds_markets      jsonb NULL                  -- markets_raw + by_bookmaker + markets_source
  venue_details     jsonb NULL
  PRIMARY KEY (match_id, source_id)
```

Hiány esetén a mező `NULL`, nem üres objektum és nem kitalált 0. Egy meccshez egy sor tartozik (forrás: research); újragenerálás tranzakciós upsert.

Visszaállítás: `DROP TABLE fact.match_insight;` + a `_sqlx_migrations` 0027 sorának törlése (külön, egyeztetett lépés).

## Adatfolyam

1. `rust/crates/ingest/src/research_insight.rs` — a már tárolt research detail JSON blokkjainak tipizált/JSONB olvasója; séma-drift-tűrő (ismeretlen mezők ignorálva, hiány → None). Egyetlen `match_insight` observation payloadját állítja elő.
2. `rust/crates/ingest/src/research.rs` — `parse_detail` az új observationt is emitálja (a meglévő kód/tesztek mellett).
3. `rust/crates/ingest/src/normalized.rs` — a `NormalizedMatch` új opcionális `insight` mezője; az observation `source_record_key` alapján párosít.
4. `rust/crates/ingest/src/canonicalize.rs` — tranzakciós `write_insight`, forrás + `captured_at` minden sorban.
5. `footballctl match-insight --from … --to … [--apply]` — offline visszatöltés a már tárolt detail JSON-ekből (a research-sync munka mellett), dry-run alapból, auditált.

Nincs automatikus fuzzy egyeztetés; a meccs kizárólag a research `uid` → `identity.source_entity_ref` úton kötődik (fail-closed).

## API-szerződés

`GET /v1/match/{id}` új mezője:

```text
insight: {
  captured_at, source, preview_hu, prediction, h2h_streaks,
  standings, season_stats, injuries, fan_poll, odds_markets, venue_details
} | null
```

A meglévő 66 kulcs változatlan; az új kulcs opcionális, régi fixture-ök miatt `null` megengedett. OpenAPI `components.schemas` regisztráció kötelező. A kontrakt-fixture (`tests/fixtures/contract/match-detail-response.json`) kétoldalas tesztje Ruston és Swift-en is fut.

## Kliens

- Core: `MatchInsight` DTO-k (`APIDecoding.makeDecoder()`), magyar szövegek és dátumformátumok.
- App: a `MatchDetailContent` progresszív szekciói — alap: preview + predikció + sérülések; összecsukható: H2H-streak, tabella, szezonstatisztika, fan-poll, teljes piacok, stadion-részletek. Ragadós hero, 860 pt max szélesség, `reduceMotion` tisztelete.
- Minden szakasz `accessibilityIdentifier` + szöveges címke; a generált tartalom VoiceOver-címkében is „Nem ellenőrzött".

## Tesztek és kapuk

- Ingest egységtesztek: séma-drift, hiány → None, számított jelzők megőrzése.
- Storage integrációs teszt: upsert idempotencia, captured_at, NULL-kezelés.
- API kontrakt: fixture round-trip, OpenAPI útvonal- és séma-teszt.
- Swift kontrakt és app modell tesztek; XCUITest az új szekciókra (AX, összecsukás, hiány).
- Kapu-lánc: `db-reset`, `check-foundation`, `cargo test`, `clippy -D warnings`, `fmt --check`, `swift test`, `xcodebuild test`, `run-ui-tests.sh`, `daily-check.sh`.

## Korlátok / kockázatok

- A preview és a predikció **generated, nem ellenőrzött** — tényként való megjelenítése tilos.
- A licencelt odds-adat (betexplorer/tipplap) csak a lokális API-ban és UI-ban jelenik meg; a publikus repóba nyers payload nem kerül.
- A teljes ~45 piac csoportosítás nélkül használhatatlan — a UI-ban összecsukható rész kell.
- A research JSON 16 napos ablak; historikus mélység csak külön engedélyezett adatfázissal (v1 import) érhető el.
