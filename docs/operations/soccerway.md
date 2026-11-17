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
- **Egy job = egy nap (S9):** a napi oldalon minden verseny szerepel; a
  parser MINDEN csoportot besorol (ország/liga/kupa + női/utánpótlás) és
  versenyenként materializál. A kiemelt kör (top-5 + NB I/II) a
  kliensben alapértelmezett szűrő, nem ingest-korlát.

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
  ('soccerway', 'obscura', 'canary', 6, 60000, 15000, 3, 120, '1.0', '2.0',
   '["day"]'::jsonb,
   '[{"event_type":"discovery","offset_minutes":null,"interval_minutes":1440}]'::jsonb,
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
  hiboldalt ad** — ezért 60 s + 15 s jitter min_delay, napi 6 kérés
  (ma + tegnap ablak), 120 s breaker-cooldown.
- A kliens-oldali lista miatt egy render 10–45 s; a worker fetch-timeoutja
  **60 s** (2026-11-15 óta; a 30 s-os plafon élőben levágta a lassú
  rendereket). Három egymás utáni fetch-hiba (timeout/transport) nyitja a
  breakert; a nyitott breaker alatt a jobok `failed`-be mennek.
- Kézi canary kör: `cargo build -p footballctl --features obscura`, majd
  `OBSCURA_BIN=<obscura> footballctl worker tick` — a CLI ugyanúgy lazy
  indítja az Obscura-szervert, mint a daemon. A jobok `min_delay` miatt
  tickenként egyenként mennek; ütemezetten (≥ breaker-cooldown) futtatva.
- Nulla-nap (nincs meccs a szűrt ligákban) → `no_data` coverage, nem
  hiba (2026-11-15-i javítás).

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

## 6. Első teljes meccses nap (runbook, 2026-11-16)

A 2026-11-15-i canary nap válogatott szünet miatt 6/7 domainre `no_data`
volt (1 domain napi kvóta miatt a következő napra maradt). Az első teljes
meccses nap menete:

1. **Daemon workerrel** (ajánlott): `WORKER_ENABLED=true DATA_DIR=./data
   OBSCURA_BIN=<obscura> cargo run -p footballd --features obscura` — a
   napi `discovery` esemény 7 jobot generál (napi kvóta 14, 60 s + jitter).
   Kézi ellenőrzéshez ugyanez tickkel: `footballctl worker tick`
   (60–70 s ütemmel, a breaker/cooldown tiszteletével).
2. **Review-bootstrap:** a lokalizált Soccerway-csapatnevek az első körben
   review-sorokat nyitnak; `footballctl identity review` →
   `identity approve <id> --entity <uuid>` (meglévő klub) vagy
   `identity approve-create <id>` (új NB I/NB II csapat). Fuzzy egyeztetés
   tilos; kétség esetén a sor pending marad.
3. **Második kör:** a feloldott csapatokkal kanonikus meccsek születnek; a
   napi lista a trust-gate szerint publikál (`verified_publish`).
4. **Ellenőrzés:** `footballctl worker status` (függő review, kvóta,
   breaker), `identity crosswalk-check`, a kliens Üzemeltetés nézete.

## 8. Oldal-orientáció és kód-stabilitás (S20, 2026-11-17)

- **Kód = stabil csapat-ID:** a meccs-URL `<slug>-<Code>` szegmensében a
  `Code` a csapathoz kötődik és renderenként stabil (pl. `MTVhw6Go` mindig
  Hungary), a slug-szöveg is ugyanazt a csapatot írja le (lokalizált
  renderben lokalizált szöveggel).
- **A sor kijelzése renderenként tükröződhet:** ugyanaz a fixture egyik
  renderben `Canada - Guatemala`, másikban `Guatemala - Canada`
  (a pontszámok is tükrözve). Ez upstream viselkedés; a parser a **slug**
  sorrendjét tekinti kanonikusnak, a kijelzési neveket a slug-szöveghez
  párosítja (ékezet/kisbetű-független egyezés), és fordított kijelzésnél a
  pontszámokat (és a büntetőket) oldalra cseréli.
- **Lock-first politika:** a kanonikus sor orientációját az első
  létrehozó futás rögzíti; későbbi fordított render a meglévő sorra megy
  (fordított materializálás), nem hoz létre új sort.
- **Létrehozási párosítás a refekhez:** a `source_entity_ref` kód→entitás
  kötése a stabil kód→slug-szöveg térképből épül (nem a render
  megjelenítési sorrendjéből) — a 2026-11-17-i javítás 188 refet
  korrigált, 16-ot pótolt, 74 elavult/keresztezett refet superseded-re
  állított (audit: `identity_ref_repair`, `identity_ref_supersede`).
- **Őr:** ha a két oldal ugyanarra a kanonikus csapatra oldódik (keresztezett
  ref maradéka), a materializer figyelmeztetéssel kihagyja a meccset
  (`azonos csapat mindkét oldalon`), nem száll el a `match_check`
  megszorításon.
- **Ismert korlát:** semleges pályás (válogatott) meccseknél a home/away
  upstream szinten kétértelmű lehet; a lock-first + második forrás
  (OpenFoot/Tipplap) az irányadó, az eredmény-orientáció konfliktusnál a
  dedupe merge csak akkor fut, ha nem veszne el egyetlen eredmény sem.
