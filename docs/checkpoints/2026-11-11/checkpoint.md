# 2026-11-11 — Napi checkpoint

**Állapot:** kész, zöld (Task 4 befejezése + Task 5 + Task 6 + Task 7 + Task 8)
**Előző nap:** 2026-11-10
**Kapcsolódó terv:** [Valós adat bekötése](../../superpowers/plans/2026-11-11-real-data-wiring.md), Task 1–8
**Branch:** `feat/real-data-wiring` (commitolatlan)

## Napi cél

A SwiftUI főoldal a SampleData helyett a footballd API valós válaszából épüljön fel; a backend
megjelenítési tényezőket kapjon (tier/prioritás, tabella, odds snapshot, HT/ET/pens), a demóadat
bővített fixture-párként töltődjön a dev adatbázisra.

## Kiinduló állapot (az előző sessionből átvéve)

- [x] Task 1–3 kész: `0003_display_facts.sql` migráció (23 tábla), fixture v2 observationök,
      kanonizáló materializálás; mindkét DB-n lefutott migráció.
- [x] Task 4 implementáció kész (`storage/repositories/day.rs`, `api/routes/day.rs`), de a
      `day_row_includes_display_fields` teszt **hiányzott** — a session itt szakadt meg.
- [x] A tesztDB-ben 12 üzleti sor maradt az előző futtatásból → `db-reset.sh` kellett.
- [x] Task 5–8 nem kezdődött el (nem volt demo fixture, Swift-módosítás).

## Elvégzett szeletek és bizonyítékok

### Task 4 — API /v1/day bővítés befejezése

- [x] TesztDB reset: `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/43, teszt 23/6/0.
- [x] Hiányzó teszt hozzáadva: `rust/crates/storage/tests/day_query.rs`
      (`day_row_includes_display_fields`). Magyar szöveg visszaolvasva („Demó”, „Újpest”, „Győr”).
- [x] `cargo test -p football-storage --test day_query -- --test-threads=1` → 2 passed.
      Megjegyzés: párhuzamos futtatásnál a két teszt a közös tesztDB-n ütközött
      (`country_code_key`); a projekt konvenciója az egy szál (`scripts/check.sh`).
- [x] `cargo fmt --all --check`, `cargo test --workspace -- --test-threads=1` (21 teszt),
      `cargo clippy --workspace --all-targets -- -D warnings` → zöld.
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/43, teszt 23/6/0.
- Eltérés: a TDD piros fázis elmaradt, mert az implementáció az előző sessionből már kész volt;
      a teszt utólag került be és azonnal zöld lett.

### Task 5 — Demo fixture-pár + dev pipeline E2E

- [x] Ideiglenes generátor: `/tmp/gen-demo-fixtures.mjs` (nem commitolandó) → 26 fájl
      (`tests/fixtures/demo_day_{a,b}_<liga>.json`), JSON-validálás hibátlan.
- [x] Magyar csapatnevek visszaolvasva a generált fájlokból (Ferencváros, Újpest, Győr,
      Szentlőrinc, Saint-Étienne, Bayern München, Atlético Madrid).
- [x] Betöltés dev DB-re: `footballctl ingest-fixture` mind a 26 fájllal; az első forrás
      `provisional_publish`, a második után mind a 20 meccs `verified_publish`.
- [x] `footballctl day --date 2026-11-11` → 20 meccs, mind verified_publish, új mezőkkel.
- [x] `footballctl doctor` → `db=1 gates=21`.
- [x] psql dev: `core.match` 21, `fact.odds_snapshot` 20, `core.standing` 38,
      `ops.publish_gate` verified_publish 21.
- [x] `footballctl day --date 2026-11-09` → 1 meccs (a régi nap megmaradt, új mezői NULL-ok).
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/979, teszt 23/6/0.
- Ismert egyszerűsítés: a terv UUID-sémája szerint a kupás Liverpool külön kanonikus csapat
      (nincs standing sora); a tabella csak a tier ≥ 1 ligákban él.

### Task 6 — Swift Core DayMatch bővítés

- [x] Két bukó teszt (`decodesDisplayFields`, `missingDisplayFieldsDecodeAsNil`) → piros
      (fordítási hiba: nincs `competitionName` tag).
- [x] `OddsSnapshot` + 18 opcionális `DayMatch` mező, snake_case CodingKeys.
- [x] `cd apps/FootballAppCore && swift test` → 15 teszt / 4 suite PASS.

### Task 7 — App HomeView valós adat + vizuális ellenőrzés

- [x] `apps/FootballAppTests/LeagueGroupTests.swift` (3 teszt) → piros (`LeagueGroup` nincs a scope-ban).
- [x] Új fájlok: `MatchCardModel.swift`, `LeagueGroup.swift` (CountryDisplay táblával).
- [x] `MatchListView.swift`: LeagueGroup/MatchCardModel alap, prediction blokk törölve,
      odds sor csak meglévő oddsnál, forrás-szöveg a meccs saját `oddsSource`-ából.
- [x] `SampleData.swift`: MatchOdds/Outcome átköltözött, Prediction törölve; a mintaadat preview-célra marad.
- [x] `HomeView.swift`: DayModel + `.task(id:)` betöltés, állapotgép (loading/loaded/empty/error),
      „MINTA ADAT” badge törölve, FilterBar országlistája a valós adatból.
- [x] `xcodegen generate`, `xcodebuild test` → 14 teszt PASS (11 régi + 3 új).
- [x] Vizuális ellenőrzés futó appban (footballd + xcodebuild build + ablak-screenshotok,
      `/tmp/shots/`, nem repó-tartalom):
  - mai nap: liga-csoportok prioritás-sorrendben, tabella-helyezések, odds nyerő-kiemeléssel,
    élő pulzálás perc nélkül, nincs badge és nincs előrejelzés blokk;
  - lista alja: FA Kupa `11-es 4–3 → 120' 2–2 → 90' 1–1 → HT 1–0`, helyezés-zárójel nélkül;
  - „Befejezett” chip (4 meccs), ország-menü valós magyar országnevekkel, Magyarország szűrő
    (NB I + NB II), üres szűrő üzenet;
  - naptár popover (2026. november, magyar hétköznapok, Ma gomb);
  - üres nap (november 12.): „Nincs meccs ezen a napon.”;
  - hibaállapot daemon nélkül: „Nem érhető el a helyi API. Fut a daemon?” + Újra gomb;
    daemon újraindítás után Újra → üres nap, Ma → 20 meccs.
- Eltérés: a vizuális kör közben két app-példány futott (az `open`-nel indított és a Computer
      Use által indított); az előbbi screenshotjai a másik ablakot mutatták. A felesleges,
      általam indított példányt leállítottam, és a screenshotokat az operált ablakon újra felvettem.

### Task 8 — Záró ellenőrzés

- [x] `./scripts/check.sh` → reset + fmt + clippy + szekvenciális workspace-tesztek (21) zöld.
- [x] `./scripts/db-reset.sh && ./scripts/db-check.sh` → dev 23/6/979, teszt 23/6/0.
- [x] `swift test` (Core) → 15 zöld; `xcodebuild test` (app) → 14 zöld.
- [x] `git status --short --branch` → `feat/real-data-wiring`, commitolatlan változások.

## Futtatott parancsok összefoglalva

```bash
./scripts/db-reset.sh && ./scripts/db-check.sh
cd rust && cargo test -p football-storage --test day_query -- --test-threads=1
cargo fmt --all --check && cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
node /tmp/gen-demo-fixtures.mjs
cargo run --manifest-path rust/Cargo.toml -p footballctl -- ingest-fixture --path <26 fájl>
cargo run --manifest-path rust/Cargo.toml -p footballctl -- day --date 2026-11-11
cargo run --manifest-path rust/Cargo.toml -p footballctl -- doctor
cd apps/FootballAppCore && swift test
cd apps && xcodegen generate && DEVELOPER_DIR=… xcodebuild test …
./scripts/check.sh
```

## Nyitott kérdések / következő döntési pont

1. Commit engedélyezése a `feat/real-data-wiring` ágon (a szelet teljes, minden ellenőrzés zöld).
2. Következő szelet kijelölése: meccs-detail valós adat, live frissítés, vagy identitás-kezelés.
3. A demo fixture-ök sorsa: maradnak-e tartós demóadatként a repóban, vagy csak a dev DB betöltéséig.

## Visszaállítási út

- Kód: a változások commitolatlanok a `feat/real-data-wiring` ágon; elvetésük `git checkout`/`restore`
  csak kifejezett engedéllyel.
- Dev DB: a demo betöltés visszajátszható (`footballctl ingest-fixture`), a 0003 előtti állapot a
  migrációs láncból nem állítható vissza adatvesztés nélkül — a dev DB demoadata tudatosan vállalt.
- Teszt DB: `./scripts/db-reset.sh` bármikor visszaállítja 23/6/0-ra.