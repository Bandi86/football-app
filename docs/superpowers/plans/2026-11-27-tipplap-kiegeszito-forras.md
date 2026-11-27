# 2026-11-27 — Tipplap kiegészítő forrás — megvalósítási terv

**Spec:** [`2026-11-27-tipplap-kiegeszito-forras-design.md`](../specs/2026-11-27-tipplap-kiegeszito-forras-design.md)
**Döntések:** [`2026-11-27-tipplap-kiegeszito-forras`](../../decisions/2026-11-27-tipplap-kiegeszito-forras.md)
**Branch:** `main` (a felhasználó 2026-11-19 óta érvényes folyamatos felhatalmazása).
**Státusz:** S0 kész; S1 indul.

---

## 0. Határ és kiindulás

- `main...origin/main`, tiszta munkaterület; az utolsó commit `61ed775`.
- A `tipplap` adapter él (42 + 11 fix ligacím, csak 1X2 opening odds).
- A mai összevetés: 128 Tipplap eseményből 11 hiány (2 ománi kupa, 1 szerb,
  8 walesi 2.). A kiváltó ok a versenycímek kimaradása a fix szótárból.
- A `/kalkulator/markets/{id}` JSON formátuma ellenőrizve élőben
  (`desktop` + `mobile` HTML-töredékek; `data-market-number`,
  `data-market-name`, `data-outcome-name/code/id`, `data-odds`). Egy kérés
  meccsenként; a jelenlegi kvóta 8/nap, ebből 5 használt.

## 1. S0 — döntések, spec, terv (dokumentáció, nincs kód)

- [x] A felhasználó három döntése: (1) csak a mért 3 hiány versenye, (2) kvóta
  emelése és minden linkelt meccs markets-lekérése, (3) review-alapú,
  fail-closed alias-kezelés. Rögzítve: `docs/decisions/2026-11-27-tipplap-kiegeszito-forras.md`.
- [x] Spec és terv elkészülte. A döntések és a kapu-lánc a specben.

## 2. S1 — három versenycím + piac-katalógus és parser (adapter)

**Írási hatókör:** `rust/crates/ingest/src/tipplap.rs`,
`rust/crates/ingest/tests/tipplap.rs`. Nincs migráció, nincs éles írás.

- [ ] **Három új `League` a fix táblába** (`GENERAL_LEAGUES` vagy saját tábla):
  `Walesi 2., Déli csoport`, `Walesi 2., Északi csoport`,
  `Ománi Szultán Kupa`, `Szerb bajnokság`. Az országkódokat körültekintően:
  Wales → `GB-WLS`, Omán → `OM`, Szerbia → `RS`; a `GB/Scotland` ghost-párt
  nem használjuk. A cím pontosan a Tipplap `th.tleft` értéke legyen
  (ékezet- és kisbetű-független `fold_name` egyezés). Az ország/kind párokat
  a forráskatalógus és a `countries.rs` alapján ellenőrizzük.
- [ ] **Piac-katalógus** `MarketKind` enum + `market_for(name) -> (market, line)`
  determinisztikus tábla a spec §3 alapján; ismeretlen piac → `other` + eredeti
  név megőrzése, `quality_flags`-jelzés. A `line` a magyar számformátumból
  (`2,5` → `2.5`), a `data-outcome-code` H/D/V/Igen/Nem → `home/draw/away/
  yes/no` fix leképezés.
- [ ] **`parse_markets()`**: a JSON `desktop` fragment parse-olása: piacfejléc
  (`th.tleft`) → csoport, sorok `data-market-name`/`data-outcome-name`/
  `data-outcome-code`/`data-odds`; duplikátum-szűrés `data-outcome-id` szerint;
  hiányos/sérült JSON → `ParseError::SchemaDrift`; üres piaclista → hiba.
  A `mobile` fragmentopcionális validálás, nem második adatforrás.
- [ ] **Tesztek:** unit (piac-leképezés, vonal, ismeretlen piac, üres JSON)
  és integrációs teszt valódi markets-JSON mintával (a repóba **csak**
  fixture, nyers élő payload nem kerül be). A meglévő 12 + 5 teszt zöld marad.
- [ ] **Kapu:** `cargo fmt --all --check`, `cargo test -p football-ingest`,
  `clippy -D warnings`, `daily-check`, `git diff --check`. Checkpoint + commit;
  S1 után megállás.
- [ ] **Visszaállás:** `git revert <S1 commit>` — a parser és a szótár
  visszaáll, a kanonikus adatot a szelet nem írta.

## 3. S2 — markets jobok, odds-írás, kvóta

**Írási hatókör:** `ops.source_policy` (SQL), `rust/crates/worker/**`,
`rust/crates/ingest/src/tipplap.rs`, `rust/crates/storage/**`, tesztek.

- [ ] **Policy:** a `tipplap` sor `domains` bővítése `markets`-szel,
  `daily_budget` felemelése a mért igényre (~150), az `events` jsonb új
  `refresh` sávval. A `min_delay_ms=3000`, `jitter_ms=1000`, breaker és robots
  változatlan. Visszaállás: egy SQL UPDATE a régi értékekre.
- [ ] **Scheduler:** új ág a `generate_jobs()`-ben — a napi Tipplap
  `match_core` observationökből (linkelt `canonical_match_id` + `data-event`)
  `markets-{event_id}` jobok `(tipplap, markets, refresh, today)`, szétütemezett
  `due_at`-vel. Csak ma-i, linkelt események; a review-sorok kimaradnak.
  Regressziós teszt: üres observation → 0 job; két esemény → 2 job;
  `stale_slot_job` a `markets` domainre is fail-closed.
- [ ] **Adapter:** `plan()` a `markets` domainre `/kalkulator/markets/{id}`;
  `parse()` a markets JSON-t `match_odds` observationné alakítja, az event ID
  stable `source_record_key`-vel, `captured_at`-tal. A dátum-guard a day-domain
  marad; markets-nél a `data-event` egyezés a guard.
- [ ] **Storage/API:** `write_odds()` a teljes piac-blokkot írja (1X2 + BTTS
  + OU + handicap + exact score + other), `market_count` a piacok száma;
  a trust `provisional` marad egy forrásnál. A napi/meccs-API mezői
  változatlanok (a `COALESCE` már ismeri az 1x2/btts/ou-t).
- [ ] **Kapu:** teljes kapulánc + daemon újratelepítés + `/v1/health` +
  `tipplap-replay` offline mérés + egy élő markets job mérés (market_count,
  quote-ok száma, 0 hiba) + `daily-check` + `integrity`. Checkpoint + commit;
  S2 után megállás.
- [ ] **Visszaállás:** `git revert <S2 commit>` + policy SQL visszaállítása;
  a már írt odds-sorok a `write_odds()` upsertjével újrajátszhatók.

## 4. S3 — napi ellenőrzés, hiányjelek, üzemeltetés

**Írási hatókör:** `scripts/daily-check.sh`, `rust/**` (issuedetektor),
`docs/operations/tipplap.md`, checkpoint.

- [ ] A `daily-check` mérje: felvett Tipplap-események, kihagyott ismeretlen
  versenycím/team, markets jobok sikertelensége; új hiányjel-kód csak ha
  indokolt (review-sorok már vannak).
- [ ] `docs/operations/tipplap.md`: új címűek, piac-tábla, kvóta, review
  folyamat, visszaállás; `docs/research/source-catalog.md` frissítése.
- [ ] Éles mérés: reggeli kör után a napi lista Tipplap-aránya, odds-lefedettség,
  0 duplikátum, `integrity OK`.
- [ ] Teljes kapu + checkpoint + commit; S3 után jelzés.

## 5. Globális kapu-lánc

Lásd a spec §8. Minden szeletben kötelező: futtatott parancs + mért szám,
új migrációnál teszt-DB előbb, UI-nál valódi macOS ellenőrzés, magyar
szövegek visszaolvasása, a nem ellenőrzött részek külön kimondása.

## 6. Ellenőr és handoff

A kaput független ellenőr futtatja (builder ≠ verifier). Minden ágens válasza a
handoff formátumban zárul: SZELET / VÁLTOZOTT / FUTOTT / NEM ELLENŐRZÖTT /
KOCKÁZAT / VISSZAÁLLÁS / KÖVETKEZŐ DÖNTÉSI PONT; a `FUTOTT` sor mért szám.

## 7. Nyitva maradó / nem ellenőrzött

- A 11-nél több, ma megfigyelt versenycím nem kerül be automatikusan.
- A review-ra váró HU csapatnevek száma napfüggő; a CLI-s jóváhagyás emberi
  döntés, nem automatizált.
- A markets oldal szerkezetváltozása fail-closed; a pontos kvótát az S2 éles
  mérése igazolja.

## 8. Visszaállás

Szeletenkénti `git revert`; a policy SQL és a job-státuszok visszaállíthatók;
a raw/observation/audit sorok nem törlődnek. A worktree-k és az RFC-spec
érintetlenek.
