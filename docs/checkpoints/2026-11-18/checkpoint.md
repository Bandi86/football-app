# Checkpoint — 2026-11-18

## Induló állapot (2026-11-17 este)

- Backfill köteg fut: **668 job pending** (2015–2026: maradék; 1993–2014:
  447; new-ligák: 15), a napi kvóta (450) 2026-11-17-én elfogyott.
- Kész: 58 job; coverage bizonyíték: PL 2020/21–2025/26 teljes
  (380 meccs, 100% eredmény/odds), Championship 2025/26 teljes.
- Napi élő ingest ép: 2026-11-17 = 255 meccs, 0 duplikátum, 0 pending
  review; `daily_budget` footballdata = 450 (ideiglenes).
- Utolsó commitok: `a43c829` (new ligák), `670aa8d` (S23e terv).

## 2026-11-17 esti lezárás (a mai 3 feladat kész)

- Keresztellenőrzés (17 liga v1↔footballdata) → ~40k verified; saját Elo
  (728 csapat); kliens Szezonok fül + seasons endpoint; minden kapu zöld.
- Backfill: 726 job done; a bootstrap top-up pass (korán kimaradt meccsek)
  **elhalasztva**: a párhuzamos gyorsítás megnyitotta a breakert
  (rate-limit védelem), utána a site throttle-olt (~1 job/perc) — ezért a
  pass ma nem futott le; a jobok done-on állnak, az első menet adatai
  megvannak. A pótlás egy későbbi, szekvenciális szelet (lásd lent).
- Keretek visszaállítva: footballdata daily_budget 60, min_delay 2000,
  jitter 1000, WORKER_TICK_SECS 60. Pending review 0.

## Mai teendők (ebben a sorrendben)

- [ ] **0. Top-up pass (elhalasztott):** a korai szezonok kimaradt meccseinek
      pótlása: `UPDATE ops.job SET status='pending', attempts=0
      WHERE domain='backfill' AND time_window < '2010'` (vagy célzott
      liga-szezonok), ideiglenes budget + szekvenciális tick; a site
      throttle miatt csak udvarias tempóban. Utána approve-create +
      reverify + coverage.
- [ ] **1. Köteg állapot:** `SELECT status, count(*) FROM ops.job WHERE
      domain='backfill' GROUP BY 1;` — ha van `failed`, a `last_error`
      alapján javítás + requeue; ha minden `done`, mehet a 2. pont.
- [ ] **2. Bootstrap-kör:** `footballctl identity approve-create-all`
      (majd maradék `--allow-unknown-country`); utána a kész jobok
      egyszeri újrafuttatása:
      `UPDATE ops.job SET status='pending', attempts=0, last_error=NULL
       WHERE domain='backfill' AND status='done';`
      — ismételd az 1–2 pontot, amíg pending review = 0 és a coverage
      stabil (jellemzően 2 kör).
- [ ] **3. Coverage KPI:** `footballctl coverage --from 1993 --to 2026` →
      liga-szezononként eredmény ≥99%, odds ≥90%, 0 duplikált csoport
      (`dedupe` scan), duplikátumok esetén `dedupe matches --apply`.
- [ ] **4. Budget visszaállítás:** footballdata `daily_budget` 450 → 60
      (élő napi 22 kérés + tartalék); ellenőrzés:
      `SELECT source_slug, daily_budget FROM ops.source_policy;`
- [x] **5. Újraellenőrzés (2026-11-17 este lefutott):** 39 219 meccs
      újraszámolva; a holnap készülő új köteg-meccsek már a javított
      betöltővel futnak. Ha a köteg után új szezonok zárulnak, futtasd
      újra: `footballctl reverify --from 2015-01-01 --to 2027-12-31`,
      majd `footballctl data-status` — a snapshotot írd a checkpointba.
- [ ] **6. Zárás:** checkpoint-frissítés (S23c/d/e eredmények), commit+push,
      majd a következő szelet (v1 import köteg vagy kliens szezon-nézet).

## Fontos kontextus

- Runbook: `docs/operations/backfill.md` (sorba állítás, tempó, bootstrap,
  visszaállítás).
- A köteg tempója ~1 job/tick; a keret a szűk keresztmetszet.
- A backfill sorok a normál kanonikus táblákban élnek provenance-szel;
  a napi élő ingesttel nem ütköznek (lock-first + dedupe).
