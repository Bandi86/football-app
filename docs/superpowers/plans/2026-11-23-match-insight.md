# 2026-11-23 — Meccsrészletező gazdagítása — megvalósítási terv

Spec: [`2026-11-23-match-insight-design.md`](../specs/2026-11-23-match-insight-design.md). Branch: `main` (a user folyamatos felhatalmazása mellett, Commit a zöld kapu után).

## Szeletek

### S1 — Séma és adatfolyam (ingest → storage)

- [ ] `db/migrations/0027_match_insight.sql` — `fact.match_insight` tábla (PK `(match_id, source_id)`, JSONB oszlopok, `captured_at`), tranzakciós, rollback-megjegyzéssel. Táblaszám 53 → **54** (`scripts/db-check.sh`, `rust/crates/storage/tests/connection.rs`, `test_support.rs` TRUNCATE).
- [ ] `rust/crates/ingest/src/research_insight.rs` — research detail blokkok séma-drift-tűrő olvasója + `match_insight` observation payload; egységteszt (hiány → None, számított jelzők).
- [ ] `research.rs::parse_detail` — az új observation emitálása.
- [ ] `normalized.rs` — `NormalizedMatch.insight` (opcionális) + párosítás `source_record_key` szerint; a meglévő konstruktorok/fixture-ök frissítése.
- [ ] `canonicalize.rs` — `write_insight` tranzakciós upserttel, forrás + `captured_at` minden sorban.
- [ ] Kapu: `cargo fmt --all`, `db-reset`, `check-foundation`, `cargo test --workspace`, `clippy -D warnings`.

### S2 — API-szerződés

- [ ] `MatchDetailRow` új `insight` mezője (opcionális) + storage lekérés.
- [ ] `openapi.rs` — `MatchInsightRow` séma regisztrációja; a teszt útvonal-száma változatlan (15).
- [ ] `tests/fixtures/contract/match-detail-response.json` — az `insight` kulcs hozzáadása (valódi adatból), Rust round-trip + Swift decode teszt.
- [ ] Kapu: kontrakt-tesztek, `cargo test`, `swift test`.

### S3 — Offline visszatöltés

- [ ] `footballctl match-insight --from … --to … [--apply]` — a már tárolt research JSON-ekből, dry-run alapból, audit-eseménnyel.
- [ ] 11-23 (mintanag) apply + mérés: sorok, hiányzó szekciók, `integrity`.
- [ ] Kapu: `daily-check.sh`, `integrity`, `reverify`.

### S4 — Kliens UI

- [ ] Core DTO-k `MatchInsight*` + magyar formátumok, `APIDecoding`.
- [ ] `MatchDetailSheet` progresszív szekciói: preview, predikció, sérülések alap; H2H-streak, tabella, szezonstatisztika, fan-poll, teljes piacok, stadion-részletek összecsukhatóan. Szakaszszintű „Számított / nem ellenőrzött" jelölés.
- [ ] AX-azonosítók és VoiceOver-címkék; `reduceMotion`; UI-fixture + XCUITest.
- [ ] Kapu: `xcodebuild test`, `run-ui-tests.sh`, valódi macOS ablak-ellenőrzés.

### S5 — Záró ellenőrzés

- [ ] Teljes kapu-lánc (Rust + Swift + UI + napi check), checkpoint, commit.

## Döntések (jóváhagyva)

1. Progresszív szekciók (alap + összecsukható részletek).
2. Szakaszszintű megbízhatósági jelölés + összefoglaló minőségi sor.
3. Kurált odds + teljes piacok JSONB-ben összecsukható UI-résszel.
4. Egy `fact.match_insight` tábla JSONB oszlopokkal (nem lazy fájlolvasás az API-ban).
5. Külön `research_insight.rs` az ingestben; a canonicalize írja a fact táblát.

## Visszaállás

- Minden szelet külön commit → `git revert`.
- A 0027 migráció visszaállítása `DROP TABLE fact.match_insight;` + migrációs sor törlése.
- Az import sorai `research` source_id + `captured_at` alapján törölhetők; az audit-események megmaradnak.
