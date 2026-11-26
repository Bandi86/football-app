# 2026-11-26 — Adatmegjelenítés és mélység — megvalósítási terv (FotMob-szerű nézetek)

**Spec:** [`2026-11-26-adatmegjelenites-fotmob-design.md`](../specs/2026-11-26-adatmegjelenites-fotmob-design.md)
**Korábbi döntések:** [`2026-11-26-app-oldalak-bovites.md`](../../decisions/2026-11-26-app-oldalak-bovites.md)
**Branch:** `main` (a felhasználó 2026-11-19 óta érvényes folyamatos felhatalmazása mellett).
**Státusz:** JÓVÁHAGYOTT terv (2026-11-26); a döntések:
[`2026-11-26-adatmegjelenites.md`](../../decisions/2026-11-26-adatmegjelenites.md).
Az S0 lezárult, **S1 folyik**. A terv maga nem tartalmaz kód-, adat- vagy migrációváltozást.

---

## 0. Határ és kiindulás

**Most ez a terv rögzíti:** a kliens információarchitektúrája, a hiányzó API-mezők és végpontok,
a szükséges migráció, a tesztfixture-ök és a kapu-lánc.

**Nem része:** új forrás-scraping (a research export már tartalmazza a szükséges blokkokat),
valódi fogadás/fogadóiroda, automatikus kifizetés, identitás- vagy fixture-összevonás, éles
kanonikus adat kézi átírása, és az AI-gomb bekapcsolása (külön döntés).

**Kiindulás (ellenőrzött tények):** `main` @ `99205a3`, `main...origin/main [ahead 2]`; az S113
(match-insight) és S173 (egységes `/edit`) már a `main`-en van; a `fact.match_insight` Novemberben
4 563 sort, a `fact.match_lineups`/`match_player_stats`/`match_shots` táblák (`0009`) adatot
tartalmaznak, de **nincs meccs-szintű API rájuk**; a Swift `MatchInsight` mind a 9 szekciót
dekódolja, de csak egy részt rajzol ki; a kliensben **nincs egyetlen kattintható forrás- vagy
entitáslink sem** (a meccs→meccs láncon kívül).

---

## 1. S0 — Döntések és worktree-egyeztetés (nem kód)

- [x] A felhasználó 2026-11-26-on mind a hat ajánlott alapértelmezést elfogadta:
  1. a három worktree kis, tesztelt UI-darabjainak átvétele review után, külön commitban;
  2. eltárolt URL-ek böngészőben nyithatók (`http(s)`-re korlátozva);
  3. `match_report_hu` + forráslinkek betöltése az S2-ben (`0029` migráció, „Számított” jel);
  4. első kód-szelet = S1 (client + csapat ID-k), nem az API-alap;
  5. beállításoldal opcionális S7, AI-gomb továbbra is ki;
  6. logófeltöltés az S8-ban, WebP/PNG, ops token.
- [x] A döntések rögzítve: `docs/decisions/2026-11-26-adatmegjelenites.md`.
- [x] A spec státusza VÁZLAT → JÓVÁHAGYOTT.
- [x] Worktree-diffek review-zva: a `match-detail-navigation` teljes diffje nem alkalmazható
  közvetlenül (`main` 66 committal előrébb, a `MatchDetailSheet` és az UI-tesztek eltérnek),
  ezért a szekciónavigáció kézzel, a jelenlegi `accessibilityPrefix`/`onSelectMatch` felülethez
  igazodva kerül be; a dátumjavítás, a `hasAnyValue`, a `marketsRaw`-védelem és a két kis teszt
  egy az egyben átvehető. A `match-chain-compact-header` munkája érintetlen marad.

### Worktree-k (csak olvasás, írás tilos)

| Path | Branch | Nem commitolt munka | Kezelés |
|---|---|---|---|
| `../football-app-v2-match-detail-navigation` | `feat/match-detail-navigation` | `MatchDetailSheet.swift`: `Section` enum, `match.detail.section.*`, `ScrollViewReader` anchor navigáció; `MatchInsightView.swift`: `mm`→`MM` dátumjavítás, üres `marketsRaw` védelem; új `MatchInsightViewTests.swift` és 2 UI-teszt | S1-ben review után átvenni vagy újraírni — **döntés kell** |
| `../football-app-v2-estimated-match-clock` | `feat/estimated-match-clock` | `MatchCardModel.ClockDisplay`, becsült óra VoiceOver, `match.clock.*` AX | külön szelet, most nem érintjük |
| `../football-app-v2-match-chain-compact-header` | `feat/match-chain-compact-header` | UI- és backend-módosítások, logo-import scriptek | az S162-es döntés szerint nem vettük át; most sem |

**Szabály:** a worktree-kben nem írunk, nem törlünk, nem `checkout`-olunk át; átvétel csak
`git diff` review + külön, azonosított commit útján. A kapu minden átvett darabra újra fut.

---

## 2. S1 — Navigáció és a meglévő adatok láthatóvá tétele

**Cél:** azonnali, látványos javulás új scraping nélkül: szekciófülek, megbízhatósági jelölés,
a már dekódolt insight-részek kirajzolása, entitáslinkek, és a dátumformátum javítása.

**Írási hatókör:** `apps/**`, `rust/crates/{api,storage}/**`, `tests/fixtures/**`, checkpoint.

### Lépések

- [x] **API — csapat ID-k:** `MatchDetailRow` + `home_team_id`, `away_team_id` a storage
      `match_detail.rs` lekérésben (UUID-k: `01a1fa06…`, `01a1fc05…` a fixture meccshez);
      kontrakt 67 → 69 kulcs, Rust round-trip és Swift `DetailContractTests` zöld.
      Az OpenAPI-séma a `ToSchema` derive-ből automatikusan követi a mezőket.
- [x] **Core:** `OddsSnapshot.hasAnyValue` + 1 teszt; `MatchDetail` két új opcionális UUID
      mezője; `AccessibilityLabels`-ben új címkék helyett a linkek saját AX-címkéje.
- [x] **UI — meccsoldal:** `MatchDetailContent` öt szekciója (`Áttekintés`, `Események`,
      `Statisztika`, `Odds`, `Research`) `ScrollViewReader` + `reduceMotion` + AX-címkével,
      a `match.detail.section.*` / `match.detail.sections` azonosítókkal; a meccs→meccs lánc
      és a „Vissza” verem megőrizve. A külön `DataViews.swift`/`TrustPill` nem készült —
      a sáv és a Trust-szöveg a meglévő `MatchInsightView`-ben maradt; ez az S4-ben tisztázódik.
- [x] **UI — insight tartalom:** `seasonStats`, `fanPoll`, tabella `played/record/source`
      és üres `marketsRaw` védelem; dátum `mm`→`MM`; a két ütköző AX-azonosító külön
      elemre került. A `preview.keyPoints` és `byBookmaker` megjelenítése **nem** készült el
      (a meglévő preview-szekciók és a kurált odds változatlanok) — nyitott marad az S4-ig.
- [x] **UI — entitáslinkek:** meccsről hazai/vendég csapat és verseny (`.buttonStyle(.borderless)`),
      keresésből csapat/verseny konkrét adatlapja (nested sheet); `EntityTarget` +
      `DetailOverlay` a több `.sheet` versengésének kiküszöbölésére. A `TeamSheet` és
      `CompetitionSheet` új AX-azonosítója: `team.detail`, `competition.detail`.
- [x] **Fixture-ök/tesztek:** UI `match-detail.json` + új `team-detail.json`,
      `competition-detail.json`; `ui-fixture-server.py` és `run-ui-tests.sh` új útvonalai;
      5 új XCUITest (szekciónavigáció, dátum, két entitás-link, keresés→csapat) + 3 app unit
      (`MatchInsightViewTests`). Az eredeti 22 teszt változatlanul zöld.
- [x] **Kapu:** `db-reset`, `check-foundation`, Rust **626/0**, clippy 0, fmt 0,
      build `--features obscura`, Core **147/25**, app unit **94/0**, UI **26/26**,
      `daily-check 11-25` EXIT=0, `integrity` OK, `git diff --check` 0.
- [x] **Éles ellenőrzés:** daemon újratelepítve (`/v1/health` OK, élő `/v1/match` 69 kulcs),
      app újratelepítve, futó ablak képernyőképe rögzítve. Az `ax-press.swift` a napi
      kártyákon nem nyit részletezőt (harness-korlát); az XCUITest 26/26 valódi macOS-en fut.
- [x] Checkpoint (S180) + commit a zöld kapu után; **S1 után megállás és jelzés**.

### Kockázat / visszaállás

- A `MatchDetailSheet` nagy refaktor könnyen eltöri a meccsláncot — az új XCUITestek ennek
  elsőkapui. Visszaállás: `git revert <S1 commit>`; fixture-ök visszaállíthatók.
- A daemon újratelepítése gateway-jóváhagyást igényelhet — nem szabad megkerülni; ha blokkolt,
  a kliens kontrakt-teszte zöld marad, de az élő mezők nem jelennek meg: ezt expliciten jelezzük.

---

## 3. S2 — Forráslinkek és magyar riport (adat + API + UI)

**Cél:** minden megjelenített tényhez kattintható, ellenőrizhető forrás; a research
`match_report_hu` tartalma is elérhető legyen, egyértelmű „Számított” jelöléssel.

**Írási hatókör:** `db/migrations/**`, `rust/**`, `apps/**`, `tests/fixtures/**`.

### Lépések

- [ ] **Migráció `0029_source_links_report.sql`:** `BEGIN…COMMIT`, magyar visszaállítási megjegyzés:
  - `fact.match_insight` új oszlop `match_report_hu jsonb NULL` (a meglévő `0027` sorai
    érintetlenek, `NULL` = hiány);
  - `fact.match_link`: `id uuid PK`, `match_id uuid FK core.match ON DELETE CASCADE`,
    `kind text CHECK (source|reference|standing|bookmaker|other)`, `label text`,
    `url text CHECK (http(s))`, `source_id uuid FK ops.source`, `captured_at timestamptz`,
    `UNIQUE (match_id, kind, url, source_id)`, index `(match_id, kind)`.
- [ ] **Infrastruktúra:** `scripts/db-check.sh` táblaszám +1; `rust/crates/storage/src/test_support.rs`
      TRUNCATE-lista; `rust/crates/storage/tests/connection.rs` táblaszám. Migráció **előbb a teszt-,
      majd a dev DB-re**, rollback dokumentálva.
- [ ] **Ingest:** `rust/crates/ingest/src/research_insight.rs` `SECTIONS` bővítése
      `match_report_hu`-val; URL-kinyerés séma-drift-tűrően a `match.links.source`,
      `source_urls[]`, `standings.url`, `links.reference`, `odds.by_bookmaker.*.sources` és
      `provenance[]` mezőkből; `research.rs` / `normalized.rs` / `canonicalize.rs` tranzakciós
      upsert; az offline `footballctl match-insight-import` ugyanazokat írja. Egységtesztek:
      hiány → üres lista, ismeretlen mezők ignorálva, URL-séma szűrés, duplikátum-idempotencia.
- [ ] **Storage/API:** `repositories/match_link.rs` + lista lekérés; `MatchDetailRow.links[]` és
      `insight.match_report_hu`; OpenAPI-szinkron; kontrakt-fixture és kétoldalas teszt;
      `GET /v1/match/{id}` 67 → ~69 kulcs, minden régi kulcs változatlan.
- [ ] **UI:** `LinkList` + `SourceLine` `Link`-el (`http(s)` ellenőrzés); a riport szakasz
      „Számított / nem ellenőrzött” jelöléssel; ha nincs URL: „Nincs forráslink” + javítási akció.
- [ ] **Adatfeltöltés:** `footballctl match-insight-import --from 2026-11-24 --to 2026-11-24
      [--apply]` dry-run → apply, mérés: sorok száma, linkek száma, 0 árva, `integrity OK`.
      Csak engedélyezett fázisban, valódi forrásadat a repóba nem kerül.
- [ ] **Kapu:** §4 + daemon újratelepítés + `daily-check 2026-11-25` + `integrity`.
- [ ] Checkpoint + commit; S2 után jelzés.

### Kockázat / visszaállás

- A `0029` migráció visszaállítása: `ALTER TABLE fact.match_insight DROP COLUMN match_report_hu;
  DROP TABLE fact.match_link;` + a `_sqlx_migrations` sor törlése — külön egyeztetett lépés.
- A `match_report_hu` **generated**, nem ellenőrzött — tényként való megjelenítése tiltott.
- Az URL-ek nem tartalmazhatnak tokent/cookie-t; a linkek csak böngészőben nyílnak, a kliens
  nem indít új scrape-et (AGENTS.md §7).

---

## 4. S3 — Meccs-szintű játékosadatok API

**Cél:** a már tárolt `0009` táblákból read-only végpontok, OpenAPI-vel és kontrakt-teszttel.

**Írási hatókör:** `rust/**`, `tests/fixtures/**`. Nincs migráció, nincs éles írás.

### Lépések

- [ ] **Adatfelmérés (read-only SQL, dev DB):** hány meccshez van `match_lineups`,
      `match_player_stats`, `match_shots`, `match_incidents` sor a 11-08…11-26 ablakban;
      a számokat a checkpoint rögzíti. Ha a lefedettség lényegesen alacsonyabb a mintanagnál
      (100/187, 70/187, 42/187), **megállás és jelzés** — nem találunk ki adatot.
- [ ] **Storage:** `rust/crates/storage/src/repositories/match_micro.rs` — `list_lineups`,
      `list_player_stats`, `list_shots` `match_id` szerint, `source_id`, `captured_at`,
      `player_entity_id` és `player_source_id` megőrzésével; üres eredmény = `[]`, nem hiba.
- [ ] **API:** `routes/lineups.rs`, `routes/player_stats.rs`, `routes/shots.rs` — read scope,
      magyar 400 (rossz UUID) / 401 / 404 (ismeretlen meccs); OpenAPI 23 → 26 útvonal,
      új `ToSchema` típusok regisztrálva; `tests/openapi.rs` száma szinkronban.
- [ ] **Kontrakt:** `tests/fixtures/contract/match-lineups-response.json`,
      `match-player-stats-response.json`, `match-shots-response.json` + Rust round-trip;
      API integrációs teszt (auth-mátrix, üres lista, 404, captured_at).
- [ ] **Kapu:** §4; daemon újratelepítés a 3 új route miatt; `daily-check`, `integrity`.
- [ ] Checkpoint + commit; S3 után jelzés.

### Kockázat / visszaállás

- Az `0009` táblák `source_player_id`-je nem mindig oldódik fel `core.entity`-re — a DTO-ban
  mindkettő szerepel, a UI a feloldatlan nevet mutatja, nem kitaláltentitást.
- Visszaállás: `git revert <S3 commit>`; a táblákat a szelet nem módosítja.

---

## 5. S4 — Felállás, játékosmetrika, lövéstérkép UI

**Cél:** a FotMob-féle mélység: pálya, rendezhető játékosstatisztika, xG-térkép, momentum,
szűrőkkel és hiányállapottal.

**Írási hatókör:** `apps/**`, `tests/fixtures/**`.

### Lépések

- [ ] **Core:** `MatchMicroModels.swift`, `APIDecoding.swift`, `MatchMicroService.swift`
      (3 végpont), `hu_HU` formátumok, `AccessibilityLabels.swift` új címkék + tesztek.
- [ ] **UI:** `DataViews.swift` új komponensei: `LineupPitch` (formáció, kezdők/cserék, hiányzók,
      kattintható játékos → `PlayerView`), `PlayerStatTable` (oszloprendezés, oldal/pozíció/
      minimum-perc szűrő), `ShotMap` (x, y, xG, xGoT), `MomentumStrip`; `EmptySection` minden
      hiányzó blokkhoz. `reduceMotion`, világos/sötét mód, `accessibilityIdentifier`.
- [ ] **Integráció:** `MatchDetailSheet.swift` új szekciói a meglévő `SectionTabBar`-ban;
      a szűrők `@State`-ből `UserDefaults`-ba kerülnek (S7-től közös mechanizmus).
- [ ] **Fixture-ök:** `tests/fixtures/ui/match-lineups.json`, `match-player-stats.json`,
      `match-shots.json`; `scripts/ui-fixture-server.py` UUID-map + 3 új route;
      `scripts/run-ui-tests.sh` változatlan marad (már minden portnak átadja a fixture-öket).
- [ ] **Tesztek:** 3 új XCUITest (szekció megjelenés, játékossor → játékosoldal, szűrő + hiány);
      Core és app unit tesztek; `run-ui-tests.sh` zöld.
- [ ] **Valódi macOS:** telepítés (`install-football-app.sh`), `ax-probe match.detail.section.stats`,
      képernyőkép, magyar címkék visszaolvasása.
- [ ] Checkpoint + commit; S4 után jelzés.

### Kockázat / visszaállás

- Nagy payload: a táblákat lapozni kell, az API p95 < 150 ms célját mérni kell; ha lassú,
  a szekció háttérben töltődik külön állapottal (nem végtelen spinner).
- Visszaállás: `git revert <S4 commit>`; az API végpontok (S3) megmaradnak.

---

## 6. S5 — Verseny- és csapatoldal mélység

**Cél:** a két főoldal is részletes, szűrhető és linkelt legyen; a játékosoldal se legyen
csonkolt lista.

**Írási hatókör:** `rust/**`, `apps/**`, `tests/fixtures/**`.

### Lépések

- [ ] **API — csapat:** `/v1/team/{id}` bővítése `players[]`-szel (as-of `core.player` +
      `core.player_team_spell`, `valid_from`/`valid_to`, `loan`), `aliases[]`-szel
      (`identity.alias`), és `links[]`-szel **csak ha van tárolt URL**; egyébként a mező `[]`,
      a UI „Nincs forráslink”-et mutat. Meglévő mezők változatlanok.
- [ ] **API — verseny:** `/v1/competition/{id}` tabella-soraihoz `team_id` (linkhez),
      form és hazai/vendég bontás **csak ha a forrás tartalmazza** (hiány `null`, nem `0`);
      meccslista szűrési paraméterei (`status`, `team_id`, `round`), kompatibilis alapértelmezéssel.
- [ ] **API — játékos:** `/v1/player/{id}` meglévő `prefix(10)`/`prefix(20)` korlátjának
      feloldása szerver-oldali `limit`/`offset` + `total` mezővel (a jelenlegi `limit` alapértelmezés
      és max 200 megőrzésével).
- [ ] **UI:** `TeamsView`/`TeamSheet`: keret, aliasok, review/összevonási állapot, hiányjelek,
      szűrő, kattintható formameccsek; `CompetitionsView`/`CompetitionSheet`: mélyebb tabella,
      kattintható csapatsor, szűrő/rendezés, hiányjel; `PlayerView`: teljes lista lapozással,
      minden sor meccsre/csapatra linkel, szezon-/versenyszűrő.
- [ ] **Fixture-ök:** a `ui-fixture-server.py` jelenleg **nem** ismeri a
      `/v1/competitions`, `/v1/competition/{id}`, `/v1/teams`, `/v1/team/{id}` útvonalakat —
      ezek + `team-detail.json`, `competition-detail.json` fixture-ök kötelezők, különben
      az S5 UI-tesztje nincs fedezve.
- [ ] **Tesztek:** Core, app unit, új XCUITestek (csapat→játékos, verseny→csapat, szűrők);
      `run-ui-tests.sh` zöld; valódi macOS AX + képernyőkép.
- [ ] Checkpoint + commit; S5 után jelzés.

### Kockázat / visszaállás

- Az identitás-review és az összevonási előzmény **csak olvasható**; a szelet nem old fel review-t
      és nem végeztet merge-et (AGENTS.md §6).
- Visszaállás: `git revert <S5 commit>`; az API régi mezői kompatibilitási okból megmaradnak.

---

## 7. S6 — Jegyzetek és előzmények a meccsoldalon

**Cél:** az S1–S3 API teljes kihasználása: az ügynökök és a felhasználó közös jegyzetei,
mező-szintű idővonal a meccsoldalon.

**Írási hatókör:** `apps/**`, `tests/fixtures/**`.

### Lépések

- [ ] **Core/service:** `OpsService` meglévő `/v1/match/{id}/notes` (GET/POST/PATCH) és
      `/v1/match/{id}/history` hívásainak bekötése a meccsoldal modelljébe; írás csak ops
      tokennel, read tokennel olvasás (a `2026-11-26` döntés 4. pontja).
- [ ] **UI:** `NotesPanel` (típuscímke `problem`/`fix`/`question`/`info`, szerző, idő, szerkesztés)
      és `HistoryPanel` (before → after, forrás, idő) a `Források` szekcióban; magyar üres/hiba/
      betöltés állapot; VoiceOver + AX-azonosító.
- [ ] **Fixture + teszt:** `ops-history.json` és új `match-notes.json` fixture, 2 XCUITest;
      `run-ui-tests.sh` zöld; valódi macOS ellenőrzés.
- [ ] Checkpoint + commit; S6 után jelzés.

---

## 8. S7 — Közös szűrők, keresési entitásugrás, beállítások (opcionális)

- [ ] A szűrők közös `FilterState` + `UserDefaults` perzisztencia (a `LeagueCollapse`/
      `LeagueFavorites` mintájára), „szűrők törlése” akcióval.
- [ ] Keresési találat → konkrét entitás megnyitása minden `kind`-ra (ha az S1 nem oldotta meg).
- [ ] Beállításoldal: téma, frissítési intervallum (`AutoRefreshPolicy`), csökkentett mozgás,
      AI-kapcsoló **alapból ki**. Titok nem kerül `UserDefaults`-ba sem a repóba.
- [ ] Kapu: §4; XCUITest a perzisztenciára; valódi macOS kattintásos ellenőrzés.
- [ ] Checkpoint + commit; S7 után jelzés.

---

## 9. S8 — Logófeltöltés UI (a régi S5 szelet)

- [ ] `POST /v1/asset/upload`: multipart, tartalom-hash, duplikátum-detektálás,
      `origin='user_upload'`, `uploaded_by='ui'`, `ops.asset` sor, `asset_uploaded` audit;
      kézzel írt kódhoz `cargo fmt --all` a kapu-lánc előtt.
- [ ] Biztonság: csak WebP/PNG, MIME-sniff, 2 MB korlát, entitás-ID ellenőrzés token + UI
      subject alapján; SVG továbbra is külön review tárgya.
- [ ] UI: `TeamSheet`, `CompetitionSheet` és a meccs-szerkesztő «Logó feltöltése» gomb,
      előnézet, hibaüzenetek magyarul.
- [ ] Kapu: §4 + feltöltési teszt + képméret/policy teszt; `daily-check`, `integrity`.
- [ ] Checkpoint + commit; S8 után jelzés.

---

## 10. S9 — AI-asszisztencia (külön döntés, alapból ki)

- [ ] Csak akkor indul, ha a felhasználó külön engedélyezi. Javaslat-only:
      `match_note(kind='info', author='agent:ai')`, amit a felhasználó elfogad/elutasít;
      kanonikus adatot nem ír, review-t nem old fel.
- [ ] A helyi `football-ai` (lfm3 + SearXNG, `FOOTBALL_AI_REASONING_EFFORT=none`) hívása
      a hiány kontextusával; forrás URL-ekkel együtt; robots/policy itt is érvényes.
- [ ] Kapcsoló alapból ki; a kapcsoló állapota nem kerül a publikus repóba.
- [ ] Kapu: §4 + külön döntési checkpoint.

---

## 11. Globális kapu-lánc (minden kód-szelet után, ebben a sorrendben)

```bash
cd ~/Documents/Code/football-app-v2
set -a && source .env && set +a
cargo fmt --all                         # kézzel írt kód után, a lánc ELŐTT
./scripts/db-reset.sh                   # CSAK a football_v2_test adatbázist
./scripts/check-foundation.sh
cd rust
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p footballd -p footballctl --features obscura
cd ..
cd apps && xcodegen generate && cd ..
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path apps/FootballAppCore
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild test \
  -project apps/FootballApp.xcodeproj -scheme FootballApp -destination 'platform=macOS' \
  -only-testing:FootballAppTests
./scripts/run-ui-tests.sh               # -only-testing:FootballAppUITests alapból
./scripts/db-reset.sh
./scripts/daily-check.sh 2026-11-25      # EXIT=0, ZÖLD
footballctl integrity --from 2026-11-25 --to 2026-11-26   # verdict OK
git diff --check                        # 0
```

**Megjegyzések:** a crate-nevek `football-*` (nem `-p ingest`); a teszt-DB közös, ezért
`--test-threads=1`; a binárisokat mindig `--features obscura`-val; új API-végpont/mező után
`./scripts/install-footballd-agent.sh` és `/v1/health` ellenőrzés (a port minden induláskor
változhat, a logból kell olvasni). A gateway-blokkolt telepítést nem szabad megkerülni.

**Minden szeletben kötelező bizonyíték:** futtatott parancs + mért szám; új migrációnál teszt-DB
előbb, mint dev DB; UI-nál valódi macOS `ax-probe`/`ax-press` + képernyőkép; magyar szövegek
visszaolvasása; a nem ellenőrzött részek külön kimondása (AGENTS.md §8).

---

## 12. Ellenőr és handoff

A kaput **független ellenőr** futtatja (builder ≠ verifier; `agents/README.md`). A kód írója
nem pipálja ki a saját kapuját. Minden ágens válasz a handoff formátumban zárul:

```
SZELET: <azonosító> — ZÖLD|PIROS
VÁLTOZOTT: <fájlok, mit és miért>
FUTOTT: <parancs → mért eredmény: tesztszám / exit kód / adatszám>
NEM ELLENŐRZÖTT: <mi maradt ki és miért>
KOCKÁZAT: <ha van>
VISSZAÁLLÁS: <hogyan>
KÖVETKEZŐ DÖNTÉSI PONT: <egy mondat>
```

A `FUTOTT` sor mért szám, nem összegzés. Piros kapu esetén A/B-vel kell igazolni, hogy nem
regresszió (mint az S169-ben), különben a szelet nem commitolható.

---

## 13. Nyitva maradó / nem ellenőrzött

- A három worktree nem commitolt munkája érintetlen marad az S0 döntéséig.
- A `2026-11-23-modern-predikcios-rendszer-terv.md` nem commitolt RFC-t nem érintjük.
- A jelenlegi 22/22 UI-teszt és a Rust 619/0, Core 146/0, app unit 90/0 számok a
  `2026-11-26` checkpointből valók; a terv nem állítja, hogy a mai futás is zöld — ezt minden
  szelet elején újra kell mérni.
- A tényleges VoiceOver-hang emberi ellenőrzése (fül kell hozzá) továbbra is nyitott.
- A `fair_odds` modell-ár tudatosan nem jelenik meg valódi bookmaker-odds-ként.

---

## 14. Visszaállás

- Kód: szeletenként külön commit → `git revert <commit>`; a régi API-mezők és a
  `PUT /v1/match/{id}/manual-result` változatlanok.
- Adat: az S2 `0029` migrációja külön egyeztetett lépésben állítható vissza
  (`DROP COLUMN match_report_hu`, `DROP TABLE fact.match_link`, migrációs sor törlése).
  A `match-insight-import` és a link-ek upsertje idempotens, az audit-események megmaradnak.
- Tesztadat: `./scripts/db-reset.sh` csak a `football_v2_test`-et érinti.
- Worktree-k: semmilyen művelet nem történik bennük a tervben.

---

## 15. Következő döntési pont

**S0 döntései** (spec §10 / §1): worktree-átvétel, külső linkek, `match_report_hu` betöltése,
első szelet (S1), beállítások/AI, logófeltöltés. A döntések után **S1 indul** — és S1 zöld kapuja
után ismét megállás és jelzés.
