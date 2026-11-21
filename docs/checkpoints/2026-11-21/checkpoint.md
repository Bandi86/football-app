# 2026-11-21 — napi checkpoint

## Induló állapot és kérés

- A felhasználó 14 AFC U17-selejtező eredményt talált a SofaScore-on a 2026-11-20-i
  naphoz, és kérte: „ezt kezeld és robald meg a tobbit is”.
- Pontosításra került: a scope a **2026-11-20-i egész nap**, a forrás a
  **SofaScore (felhasználói kézi kiemelés)**.
- Kiindulás: 244 meccs, 225 eredmény; 19 eredmény nélküli sor, ebből 9 AFC U17,
  8 egyéb kis liga és 2 `postponed` (helyesen eredmény nélkül).
- A `rust/crates/worker/src/tick.rs` módosítás a session elején is jelen volt
  (nem commitolt, cross-competition auto-dedupe); **nem lett megváltoztatva**.

## Biztonsági mentés és visszaállítás

- Mentés az apply előtt: `/tmp/football_v2_dev_20261121_pre_sofascore.dump`
  (560 MB, `pg_dump -Fc -Z 5`); visszaállítás `pg_restore --clean --if-exists`-szel.
- Az első betöltési kísérlet hibás volt: a `source_entity_id` csak a kanonikus UUID
  első 8 karakterét használta, és több csapatnak azonos az eleje (`01a1e8a9…`),
  így a `team_ids` térkép felülírta önmagát — 6 meccs rossz csapatpárt kapott.
  A DB-t a mentésből visszaállítottam (244 meccs / 225 eredmény / 0 sofascore
  source), a generátort teljes UUID-re javítottam, és újra betöltöttem. Végeredmény
  mind a 14 meccsnél ellenőrizve.

## S68 — 11-20 hiányzó eredmények pótlása

### SofaScore (kézi kiemelés, `ingest-fixture` út)

- [x] **AFC U17 Qualification, 14 meccs**: 9 meglévő `scheduled` sor eredménye
      kitöltve, 5 hiányzó meccs létrehozva (Philippines–Afghanistan 2-2,
      Chinese Taipei–Hong Kong 1-1, Bhutan–Syria 0-10, Pakistan–Jordan 2-2,
      Yemen–Turkmenistan `canceled`). Csoportcímkék (Group A–G) bekerültek.
      A forrás az `ops.source.code='sofascore'`, a nyers payload content-addressed
      artifactként és `stage.observation` + `fact.provenance` néven rögzítve.
- [x] A SofaScore-kiemelés nem tartalmaz kezdési időt: a fixture-út alapból
      `exact`-re írta a pontosságot, ezért auditált SQL-lel
      (`match_kickoff_precision_revision`) visszaállítottam `date_only`-ra a 14 meccsnél.
- [x] **3 további meccs** SofaScore API-ból ellenőrizve (esemény-ID + UTC időbélyeg
      a DB kickoffjával egyezik): Heroes de Zaci 0-1 Delfines de Coatzacoalcos
      (`sfs:16926807`), Huracanes Izcalli 2-4 Gorilas de Juanacatlan
      (`sfs:16927012`), Black Bulls 0-3 Ferroviario Maputo (`sfs:17591413`).
      Az utóbbinál a SofaScore pontos kezdése (17:00Z) bekerült `exact` pontossággal.

### Research forrás (intézményesített út)

- [x] A 11-20-i research napi envelope 01:17-kor frissült (Florida State–UCF
      4-1), de az utolsó job 00:30-kor futott. Az adott day-job újraqueue-olva,
      `footballctl worker tick` feldolgozta: a meccs `live` → `finished`, 4-1.

### Collateral integritási javítás

- [x] Az `integrity` 3 aktív refet talált összevont entitásra (research forrás,
      `fm:9851`, `espn:169`, `espn:8089`; keletkezési idejük 03:52, azaz a jelen
      session előtti). Audittal a túlélő entitásokra irányítottam át őket
      (`source_ref_repoint_after_merge`) — nem a SofaScore-betöltés okozta.

### Ellenőrzések

- [x] `footballctl reverify --from 2026-11-20 --to 2026-11-20` → **275 meccs, 0 hiba**
      (117 verified / 157 provisional / 1 withheld).
- [x] `footballctl integrity --from 2026-11-20 --to 2026-11-20` → duplikált kickoff 0,
      duplikált fixture 0, árva provenance 0, aktív ref összevont entitásra 0,
      **verdict OK**. (Függő review 1027, withheld 24, failed job 2, nyitott
      breaker 1 — mind pre-existing, nem kemény hiba.)
- [x] `footballctl coverage-day 2026-11-20` → **276 meccs / 269 lezárt / 269 eredmény**
      (kiindulás: 244 / 225 / 225).
- [x] `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets
      -- -D warnings` exit 0; `cargo test --workspace -- --test-threads=1` exit 0,
      **97 zöld tesztblokk, 0 hiba**.
- [x] Daemon újraindítva (`launchctl bootstrap`, port **64606**), `/v1/health` 200,
      `/v1/day/2026-11-20` **276 sor**; az új eredmények és a `canceled` státusz
      az API-ban is látszanak.

## Nem pótolt tételek (forrás hiányában, nem találgatva)

- [ ] `Atletico Hidalgo – Cruz Azul Lagunas` (Liga Premier: Serie B) — a SofaScore
      csak egy 11-22-i `notstarted` eseményt ismer erre a párosra.
- [ ] `Furiani Agliani – US Le Pays du Valois` (National 1: Group A) — SofaScore
      keresése 0 eseményt ad; a research envelope `scheduled`, eredmény nélkül.
- [ ] `Ranchers Bees – Plateau United` (NPFL) — SofaScore 0 esemény; a research
      11-21-i fájlja is `scheduled`, eredmény nélkül.
- [ ] `Kickstart N – HOPS N` (IWL női) — SofaScore 11-20-i eseménye `notstarted`
      (15:50 local), eredmény nélkül.
- [x] 2 `postponed` (Oberliga: Bayern North) és 1 `canceled` (Yemen–Turkmenistan)
      helyesen eredmény nélkül maradt.

## Visszaállítás

- `/tmp/football_v2_dev_20261121_pre_sofascore.dump` → `pg_restore --clean
  --if-exists -d "$DATABASE_URL" <dump>` (az egész dev DB-t visszaállítja).
- Minden kanonikus írás (precision, repoint) `ops.audit_event` sorral rendelkezik;
  a SofaScore-import nyers payloadja és observation-jei a capture-hez kötve.

## Következő döntési pont

- A 4 forrás nélküli meccshez a felhasználótól SofaScore-kiemelés vagy más
  hiteles forrás kérése; a `tick.rs`-ben lévő nem commitolt auto-dedupe változás
  külön szelet (builder ≠ verifier, független ellenőrzéssel).
