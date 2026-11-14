# S2d-2 — Meccs-mikro adatok: match_team_stats, lineups, shots, incidents, player_stats

**Dátum:** 2026-11-14
**Állapot:** brainstorming-jóváhagyott terv; felhasználói spec-review függőben
**Kapcsolódó dokumentumok:** [source-catalog séma-rés lista](../../research/source-catalog.md)
(18 tétel; 2. ciklus: 2., 7., 9., 10., 11.), [S2d-1 spec](../specs/2026-11-14-s2d1-schema-gaps-design.md),
[2026-11-14 checkpoint](../../checkpoints/2026-11-14/checkpoint.md)
**Branch:** `feat/s2d2-match-micro` (a `feat/s2d1-schema-gaps` csúcsáról — sem a
S2c, sem a S2d-1 nincs merge-elve a main-be)

## 1. Hatókör

Az S2d második ciklusa: a meccs-szintű mikro-adatok sémája az S3-adapterekhez
(FotMob elsődleges, Livescore tervezett). Öt új tábla, writer nélkül:

- `fact.match_team_stats` — csapat-metrikák periódusonként (keyed).
- `fact.match_player_stats` — játékos-metrikák játékosonként (keyed).
- `fact.match_lineups` — kezdők/cserék oldalanként.
- `fact.match_shots` — lövésenkénti shotmap (xG-domain).
- `fact.match_incidents` — meccs-események (gól, lap, csere…).

### 1.1 Jóváhagyott döntések (2026-11-14)

1. **Játékos-hivatkozás: dupla** — minden játékos-szintű táblában
   `source_player_id text` + `player_entity_id uuid NULL REFERENCES
   core.entity(id)`; a crosswalk/fail-closed linker az S3 adapteré.
2. **Nyílt taxonómiák: szabad szöveg + dokumentált készlet** (incident_type,
   shot_type, stat_key, position) — nincs CHECK; a séma-drift őr a parser
   rétegben. A zárt modell-dimenziók (side, period, role) **CHECK**-et kapnak.
3. **Nincs writer/olvasó ebben a szeletben** — a táblák író nélkül jönnek
   létre (mint a S2d-1 `match_stats`); az S3 adapterek töltik, idempotens
   per-`(match, source)` blokk-cserével.

### 1.2 Nem célok (YAGNI)

- Adapter/writer, API, Swift, trust-logika a mikro-adatokra (S3).
- `player_availability`, `season_team_stats`, `season_player_stats`,
  asset-nyilvántartás, Elo (a maradék S2d-csoportok).
- Forrás-specifikus extra mezők a katalóguson túl (pl. shot outcome) — az
  S3-adapter igénye esetén bővítjük.

## 2. Séma — 0009 migráció

Közös minta: `match_id uuid NOT NULL REFERENCES core.match(id) ON DELETE
CASCADE`, `source_id uuid NOT NULL REFERENCES ops.source(id)`,
`captured_at timestamptz NOT NULL`; per-source sorok; a hiányzó adat NULL.

### 2.1 `fact.match_team_stats`

- PK: `(match_id, source_id, side, period, stat_key)`.
- `side text NOT NULL CHECK (side IN ('home','away'))`.
- `period text NOT NULL CHECK (period IN ('all','first_half','second_half'))`.
- `stat_key text NOT NULL`; `value numeric NULL`.
- Documentált stat-készlet (FotMob 14): possession, xg, shots, shots_on,
  big_chances, accurate_passes, yellows, corners, woodwork, shots_in_box,
  passes, offsides, reds, fouls.
- Index: `(match_id, side)`.

### 2.2 `fact.match_player_stats`

- PK: `(match_id, source_id, source_player_id, stat_key)`.
- `source_player_id text NOT NULL`; `player_entity_id uuid NULL REFERENCES
  core.entity(id)`.
- `side text NOT NULL CHECK (side IN ('home','away'))`.
- `stat_key text NOT NULL` (a rating kulcsként: `rating`); `value numeric NULL`.
- Index: `(source_player_id)`.

### 2.3 `fact.match_lineups`

- PK: `(match_id, source_id, side, source_player_id)`.
- `side` CHECK mint fent; `role text NOT NULL CHECK (role IN ('starter','sub'))`.
- `source_player_id text NOT NULL`; `player_entity_id uuid NULL`.
- `position text NULL` (pl. `GK`, `CB`…); `shirt_number integer NULL`
  (CHECK `shirt_number > 0`); `rating numeric(3,1) NULL`.
- Index: `(match_id, side)`, `(source_player_id)`.

### 2.4 `fact.match_shots`

- PK: `(match_id, source_id, shot_no)`; `shot_no integer NOT NULL
  CHECK (shot_no > 0)` — adapter-oldali, determinisztikus sorszám.
- `side` CHECK; `source_player_id text NULL`; `player_entity_id uuid NULL`.
- `minute integer NULL`; `x numeric NULL`, `y numeric NULL` (normalizált
  koordináták); `xg numeric NULL`, `xgot numeric NULL`; `shot_type text NULL`.
- Index: `(match_id)`.

### 2.5 `fact.match_incidents`

- PK: `(match_id, source_id, incident_no)`; `incident_no integer NOT NULL
  CHECK (incident_no > 0)`.
- `side` CHECK; `incident_type text NOT NULL`.
- `player_source_id text NULL` + `player_entity_id uuid NULL`;
  `related_player_source_id text NULL` + `related_player_entity_id uuid
  NULL` (pl. csere: be/ki; gól: gólszerző/assziszt).
- `minute integer NULL`; `detail text NULL`.
- Index: `(match_id)`.

### 2.6 DB-kapu

A 0009 **+5 tábla** → `scripts/db-check.sh` elvárt táblaszám: **40**;
`storage/tests/connection.rs` 40; `test_support` TRUNCATE-lista bővül.

## 3. Határok és adatkezelés

- Writer/olvasó nincs; a pipeline (canonicalize/trust/day) változatlan, a
  demo érintetlen.
- Idempotencia-szerződés az S3-nak: per `(match, source)` delete+insert;
  a `shot_no`/`incident_no` az adapter determinisztikus sorszáma.
- Provenance: a mikro-sorok `source_id` + `captured_at` forrást hordoznak
  (mint az odds); a mezőszintű `fact.provenance` a kanonikus meccs-mezőké
  marad.
- Játékos-entitás: a forrás-azonosító text; az entitás-FK a fail-closed
  linkeré (review-queue); fuzzy összevonás tilos.

## 4. Tesztelés

- Új `storage/tests/s2d2_match_micro_schema.rs` a `seed_min_match` láncra:
  CHECK-ek (side/period/role, sorszámok), PK-duplikátumok (mind az 5 tábla),
  FK-hibák (match/source/entity), kaszkád (meccs-törlés → mind az 5 tábla
  ürül), nullable oszlopok, keyed stat-PK.
- `connection.rs`: 40 tábla; `test_support`: 5 új tábla a TRUNCATE-listán.
- Kapuk: `./scripts/check.sh` (161+ teszt), `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo fmt --all --check`, db-check 40.
- 0 élő kérés, 0 nyers payload; a demo pipeline nem fut újra (nem változott).

## 5. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| Forrás-taxonómia későbbi bővítése | szabad text + dokumentált készlet; a séma nem köt típuskészletet |
| Sorszám-PK instabilitás | idempotens blokk-csere szerződés; a sorszám per-ingest determinisztikus |
| Táblák író nélkül | tudatos (S3-előkészítés); a DoD séma + constraint-tesztek |

## 6. Visszaállítási út

- Az 5 tábla dropja (a 0009 inverze); teszt DB: reset; dev DB: a migráció
  a Task 2-ben, külön engedéllyel, dry-run ellenőrzéssel; kód: branch-revert.

## 7. Következő lépés

1. Felhasználói spec-review (ez a dokumentum).
2. writing-plans → `docs/superpowers/plans/2026-11-14-s2d2-match-micro.md`.
3. Implementáció inline execution-nel, zöld task után **committal**.
4. Utána a maradék S2d-csoportok (B: szezon/elérhetőség, E: konvenciók,
   F: identitás/asset, G: Elo) vagy S3.
