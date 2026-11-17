# Backfill üzemeltetés (S23)

A történelmi backfill job-típusa: `source='footballdata'`, `domain='backfill'`,
`entity_key=<liga>`, `time_window=<szezonkód, pl. 2526>`, `event_type='backfill'`.
Egy job = egy liga-szezon CSV (`mmz4281/{szezon}/{LIGA}.csv`).

## Sorba állítás

```bash
footballctl backfill plan --from 2015 --to 2026 --leagues all [--apply]
```

- Idempotens (unique kulcs), dry-run alapból.
- A liga első szezonja a beépített archívum-táblából jön (a korábbi
  szezonok kimaradnak).
- Priority 200: az élő napi jobok mindig előbb futnak.

## Futás és tempó

- A tick 10 jobot igényel körönként; a napi kvóta
  (`ops.source_policy.daily_budget`) a szűk keresztmetszet.
- Backfill-időszakban javasolt: `daily_budget = 450`, a köteg után
  visszaállítás 60-ra (élő napi 22 kérés + tartalék).
- A parse szezon-módban fut: nincs ±7 napos ablak; a szezon-címke a
  kódból jön; a sorok dátumai a szezon sávjába (±2 hónap) essenek.

## Bootstrap-kör (identitás)

1. A batch futása közben/befejezése után:
   `footballctl identity approve-create-all` (országos kontextus; a
   játékvezetők is létrejönnek).
2. A jóváhagyás előtt kimaradt meccsek pótlása: a kész backfill jobok
   újrafuttatása egyszer:
   `UPDATE ops.job SET status='pending', attempts=0, last_error=NULL
    WHERE domain='backfill' AND status='done';`
3. Ismételd az 1–2 pontot, amíg a pending review 0 és a coverage riport
   stabil (jellemzően 2 kör elég; a csapatok szezonok között ismétlődnek).

## Lefedettség-ellenőrzés

```bash
footballctl coverage --from 2015 --to 2026
```

Liga-szezononként: meccs, eredmény %, odds %. A KPI: eredmény ≥99%,
odds ≥90% (ahol a forrás ad oddsot), 0 duplikált csoport.

## Ismert archívum-sajátosságok (kezelve)

- Régi szezonok: nincs `Time` oszlop → `date_only` precízió (12:00 UTC).
- Régi szezonok: 2 jegyű év (`13/08/16`) → a parser plausibilitás-ellenőrzéssel
  újraértelmezi.
- Hibás odds-cellák (0, üres) → kihagyva (a DB `price > 1` megszorítása).

## Visszaállítás

- A backfill sorok a normál kanonikus táblákban élnek (provenance-szel);
  célzott visszaállítás: `dedupe_archive`/`entity_merge_archive` nem
  érinti őket — a forrás-szintű visszaállítás a `core.match` adott
  szezonjainak törlése lenne, csak külön engedéllyel.
- A `daily_budget` visszaállítása kötelező a köteg után.
