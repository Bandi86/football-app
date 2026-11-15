# Soccerway forrás — üzemeltetési útmutató (S4d)

A `soccerway` a JS-renderelt nap-feed forrás az **Obscura** browser-fetcher
mögött (`source_policy.transport = 'obscura'`). A napi oldal a localizált
útvonalon érhető el, a meccssorok kliens-oldalon renderelődnek. A parser
a kanonikus observation-sémát állítja elő (`team_identity`, `match_core`);
a meccs-eredmény a 90 perces FT-konvenciót követi, ET/pens `decided_by`-jel.

## 1. Döntések (2026-11-15 research)

- **Nap-útvonal:** ma → `https://www.soccerway.com/`, tegnap →
  `/tegnap/` (a helyi nap szerint). Más dátum → üres fetch-terv (a
  naptár-popup útvonala nincs mérve, backfill egyelőre nincs).
- **`?date=` NEM működik:** a Soccerway figyelmen kívül hagyja.
- **Readiness:** a fetcher a `div.event__match[data-event-row]` markerre
  vár (max ~20 s); az 1,5 s settle csak héj-HTML-t ad.
- **Időzóna:** az Obscura gyerekfolyamat `TZ=UTC`-vel indul, ezért a
  listán megjelenő kezdési idők **UTC-ben** értendők
  (`competition_timezone = UTC`). Élő mérés (2026-11-15): 19:45 UTC =
  20:45 CET nemzetközi kezdés — a pin érvényesül.
- **Státusz-mapping (hu lokalizált stage-szöveg):**

  | stage-szöveg | státusz | decided_by | eredmény |
  |---|---|---|---|
  | `Vége` | finished | `ft` | FT = fő pontszám |
  | `Hossz. után` | finished | `et` | ET = fő pontszám, FT NULL |
  | `Büntetők után` | finished | `pens` | ET = fő pontszám, pens = `(4)` sup |
  | `Elhalasztva` | postponed | — | nincs |
  | `Törölve` | canceled | — | nincs |
  | `Elmarad` | abandoned | — | nincs |
  | ütemezett sor (`event__match--scheduled`) | scheduled | — | kickoff HH:MM |
  | élő sor (`event__match--live`) | live | — | nincs eredmény |

  Ismeretlen szöveg + pontszám → `finished`, `decided_by` NULL;
  ismeretlen szöveg pontszám nélkül → a sor kimarad (warn).
- **Kickoff-precízió:** pontos idő csak ütemezett sornál van; máskor
  12:00 UTC helyőrző `date_only` precízióval (a kanonikalizáló nem
  rontja le a korábbi pontos kickoffot).
- **Decoy-200:** a Soccerway ismételt gyors kérések után 200-zal ad
  „The requested page can't be displayed" hiboldalt (`404_page`
  dataLayer). A parser `DecoyDetected`-del fail-closed elutasítja.
- **Egy job = egy liga:** a napi oldalon minden verseny szerepel; a
  parser a `domain` (liga-slug) csoportjára szűr.

## 2. Policy- és trust-seed (dev DB)

```sql
INSERT INTO ops.source (id, code, display_name)
VALUES (gen_random_uuid(), 'soccerway', 'Soccerway')
ON CONFLICT (code) DO NOTHING;

INSERT INTO ops.source_policy
  (source_slug, transport, mode, daily_budget, min_delay_ms, jitter_ms,
   breaker_threshold, breaker_cooldown_secs, adapter_version, parser_version,
   domains, events, enabled, max_bytes)
VALUES
  ('soccerway', 'obscura', 'canary', 14, 15000, 5000, 3, 120, '1.0', '1.0',
   ARRAY['anglia/premier-league','spanyolorszag/laliga','olaszorszag/serie-a',
         'nemetorszag/bundesliga','franciaorszag/ligue-1',
         'magyarorszag/nb-i','magyarorszag/nb-ii'],
   ARRAY['{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440}'::jsonb],
   false, 26214400)
ON CONFLICT (source_slug) DO UPDATE SET
  transport = EXCLUDED.transport, mode = EXCLUDED.mode,
  domains = EXCLUDED.domains, enabled = EXCLUDED.enabled;

INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
SELECT id, 'result', 'medium', NULL FROM ops.source
WHERE code = 'soccerway'
ON CONFLICT (source_id, domain) DO NOTHING;
```

Az induló állapot **`enabled = false`** (élő bekapcsolás külön döntés).
Az élő körhöz a `footballd` build `--features obscura`, és a gépén
elérhető `tools/obscura/obscura` bináris (lásd [obscura.md](obscura.md)).

## 3. Terhelés és cooldown

- A Soccerway oldal ismételt, gyors renderelésre **elakad vagy 200-as
  hiboldalt ad** — ezért 15 s + 5 s jitter min_delay, napi 14 kérés
  (7 liga × 2 napos kör), 120 s breaker-cooldown.
- A kliens-oldali lista miatt egy render ~10–20 s; a worker
  fetch-timeoutja (30 s) szűk lehet erős terhelésnél — figyelni kell a
  ledger `latency_ms` értékét.

## 4. Kétfázisú bootstrap

A Soccerway csapatnevei **lokalizáltak** (hu), ezért az első kör a
footballdata-hoz hasonlóan review-sorokat nyit:

1. **1. kör:** `footballctl worker tick` → `identity review` listázza a
   pending sorokat; a lokalizált neveket a meglévő entitásokhoz kell
   kötni (`identity approve <id> --entity <uuid>`), vagy új entitást
   létrehozni (`identity approve-create <id>`).
2. **2. kör:** a feloldott csapatokkal kanonikus meccsek születnek; a
   napi lista a gate szerint publikál.

Batch-jóváhagyás: `identity approve-create-all` (csak akkor, ha az új
entitás a helyes döntés).

## 5. Visszaállítási út

- Forrás kikapcsolása: `UPDATE ops.source_policy SET enabled = false
  WHERE source_slug = 'soccerway';` (a tick ezt követően tiltott
  policy-ként kezeli).
- Policy-sor törlése: `DELETE FROM ops.source_policy WHERE
  source_slug = 'soccerway';` (a trust-sor és a ledger auditként marad).
- Kód: branch-revert; séma-változás nincs. A stage-observationök és a
  kanonikus sorok forrás-kötöttek; a Soccerway-hivatkozású
  `identity.source_entity_ref` sorok a review-queue-ban visszafordíthatók.
