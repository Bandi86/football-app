# 2026-11-26 — App-oldalak bővítése, S3: detektor, globális audit és összefoglaló

Spec: [`2026-11-25-app-oldalak-bovites-design.md`](../specs/2026-11-25-app-oldalak-bovites-design.md) §7.1 + §10.3.
Előzmény: S168 (adatmodell + API, `1e41241`), S169 (Üzemeltetés UI, `1eb3ee9`).
Döntések: [`2026-11-26-app-oldalak-bovites.md`](../../decisions/2026-11-26-app-oldalak-bovites.md)
— a jeleket a `daily-check`/`integrity` írja az adatbázisba; a globális audit-feed
az S2-ben kimaradt, most jön.

## Határ

Most: (1) hiányjel-detektor CLI + bekötés a `daily-check`-be, (2) globális
audit-végpont, (3) magyar összefoglaló végpont, (4) jel megoldásának végpontja,
(5) ezek kliens-oldali megjelenítése az Üzemeltetés oldalon, tesztekkel.

**Nem most:** teljes meccs-szerkesztés (S4), logófeltöltés (S5), Versenyek/
Csapatok oldalak (S6), jegyzetek panel a meccsrészletezőben (S7), AI-gomb (S8).

## S3a — Backend

- [ ] `rust/crates/storage/src/repositories/detect.rs` (új): napi jelölések
      lekérése meccs-szinten a `fetch_day_gaps` SQL-jéből (date_only,
      open_past 10/150 perc, hiányzó odds, hiányzó csapat-/versenylogó),
      plusz `identity.review_queue` (team/competition) és visszatartott
      publish_gate. `DetectedIssue { subject_kind, subject_id, code, severity,
      detail }`; a detektált jelek natural key-je megegyezik az `ops.issue`
      partial unique indexével.
- [ ] `footballctl issues detect --date … [--apply]` (dry-run alapból,
      magyar összegzés): `upsert_seen` minden jelre; a nap korábban nyitott,
      most már nem detektált jeleit `resolve`-olja; audit esemény
      (`issues_detected`, `issues_resolved`) darabszámmal. A detektor CsAK
      `ops.issue`-t ír — kanonikus adatot nem.
- [ ] `scripts/daily-check.sh`: a detektor hívása `--apply`-vel a meglévő
      ellenőrzések után (ha a CLI nem érhető, ne buktassa a napi checket —
      de a hiba látszódjon a kimeneten).
- [ ] `GET /v1/ops/audit?limit=&subject_kind=&subject_id=` — globális
      `ops.audit_event` feed, Ops scope, utoipa + regisztráció; teszt.
- [ ] `GET /v1/ops/summary?date=` — napi/heti összefoglaló: hiányok száma
      kódonként, trend (előző naphoz képest), 3–5 kiemelt akció, egymondatos
      magyar szöveg. Read scope; a napi feedbe nem kerülnek benne jegyzetek.
- [ ] `POST /v1/issue/{id}/resolve` — Ops scope, auditált, idempotens.
- [ ] Kapu: `cargo fmt --all`, `db-reset`, `check-foundation`,
      `cargo test --workspace -- --test-threads=1`, `clippy -D warnings`,
      `build --features obscura`, `daily-check`, `integrity`, `git diff --check`.

## S3b — Kliens

- [ ] Core: `OpsAuditDTO`, `OpsSummaryDTO`, `OpsActionDTO`; `OpsService`
      bővítése `audit()/summary()/resolveIssue()`; hu_HU formátum és új
      AccessibilityLabels + unit tesztek; a közös `ops-summary-response.json`
      kontrakt-fixture (Rust + Swift).
- [ ] App: `OpsModel` summary/audit állapot + `loadAudit/loadSummary`,
      `OperationsView`: összefoglaló sáv a kártyák fölött (magyar mondat +
      kiemelt akciók), negyedik szekció a globális audit-feednek (szűrővel),
      hiánysor akciója „Megoldottnak jelölés" (ops token, auditált).
- [ ] UI fixture-szerver: `/v1/ops/audit`, `/v1/ops/summary`; új UI-teszt
      (összefoglaló mondat, audit feed, megoldás akció).
- [ ] Kapu: Core + app unit + UI-teszt (izoláltan is), valódi macOS AX,
      `daily-check`, `git diff --check`, checkpoint, commit.

## Kockázatok / visszaállás

- A detektor tévesen „megoldottnak" jelölhet jeleket, ha a forrás átmenetileg
  nem ad odsot → a `resolve` csak akkor történjen, ha a jel már nem detektálódik
  és a forrás friss; különben a jel újra keletkezik (idővonal megmarad).
- A globális audit feed `detail` mezője bizonyítékot/belső adatot is hordozhat;
  Ops scope, nem a nyilvános napi feed része.
- Visszaállás: `git revert`; a detektor által írt `ops.issue` sorok az audit
  eseménnyel és az `ops.issue.resolved_at`/törlés útján visszaállíthatók;
  a kanonikus adatot S3 nem módosítja.
