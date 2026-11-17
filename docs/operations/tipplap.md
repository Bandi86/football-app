# Tipplap forrás (tipplap)

Frissítés: 2026-11-17 (S18 canary).
Katalógus: `docs/research/source-catalog.md` → `tipplap` bejegyzés.

## Mit hoz

- Napi kalkulátor-lista: meccsek + **Tippmix 1X2 nyitó szorzók**
  (a v2 odds-pipeline `provisional` trust-tal tárolja — egy forrás).
- HU helyi időben adott kezdés (Europe/Budapest → UTC), dátum-guarddal.
- A forrásnak nincs stabil csapat-ID-ja: a csapatnevek (HU) a
  crosswalk-aliasokon keresztül oldódnak fel (lásd lent).

## Kérés és policy

- URL: `https://www.tipplap.hu/kalkulator/labdarugas/{YYYY-MM-DD}`
  (egy kérés/nap; a múlt-napi kérés a MAI listát szolgálja ki → a
  dátum-guard fail-closed megállítja).
- robots: engedett (`/kalkulator/`), katalógus-spike 2026-11-11.
- Policy: `rust_http`, `canary`, `daily_budget=6`, `min_delay_ms=3000`,
  `jitter_ms=1000`, domain `day`, napi discovery (1440 perc).

## Parser-őrök

- **Dátum-guard:** `<select id="dates">` selected option értéke == kért nap,
  különben `DateGuardMismatch` (nincs írás).
- **Üres lista:** nincs adatsor → `SchemaDrift` (decoy-gyanú).
- **Speciális tippek** (kérdőjel, zárójel, „NL " kezdet) kimaradnak.
- **Ismeretlen ligacím → a sor kimarad** (fix szótár, naming-stability
  szabály: nem hozunk létre új verseny-entitást).
- Desktop tábla only (`#calcTableDesktop`); a mobil tábla duplikál.

## Verseny-szótár (fix)

A kulcs a Tipplap magyar ligacím-előtagja, az érték a v2 kanonikus név
(egyeznie kell a Soccerway/footballdata névvel, hogy a meccsek
összeérjenek). Beépített: UEFA Nations League A–D, Africa Cup of Nations -
Qualification, CONCACAF Nations League B, Friendly International,
Premier League, La Liga, Serie A, Bundesliga, Ligue 1, NB I.
Új ligát csak a szótár bővítésével és stabil kanonikus névvel szabad
felvenni.

## Csapat-feloldás

- Nemzetközi verseny: a csapatnév ország → kanonikus EN név + ISO
  (`countries.rs` lokalizált/angol táblák); ismeretlen név → a sor kimarad.
- Hazai liga: a megfigyelt név megy a fail-closed linkerbe; a
  `footballctl identity seed-v1` a v1 crosswalkból tölt reviewed
  aliasokat (nyers + ékezet nélküli alak), így pl. „Ferencvárosi TC" és
  „Ferencvaros" is feloldódik. Ismeretlen csapat → review-sor.

## Canary-eredmény (2026-11-17)

- 92 adatsor, 53 kihagyva (ismeretlen liga / speciális tipp / hiányzó idő
  vagy odds), 39 feldolgozott esemény.
- **36 meccs** a meglévő Soccerway-sorokhoz illesztve (Africa Cup 16,
  Friendly 9, Nations League B 6, CONCACAF B 2→3, Nations League A 2),
  **0 duplikált csoport**; a nap összlétszáma változatlan (190).
- Mind a 36 meccshez Tippmix 1X2 snapshot (`provisional`); a nap-API
  megjeleníti (fázis-preferencia: closing, ha nincs → opening; Tippmix a
  B365 után esik be a COALESCE-láncba).
- Nyitott review: `China`, `Dominica` (duplikált kanonikus entitás-párok →
  `multiple_candidates`, entitás-merge szelet kell), `Jordan` (Soccerway).

## Visszaállítás

- Forrás kikapcsolása: `UPDATE ops.source_policy SET enabled=false WHERE
  source_slug='tipplap';`
- Alias-seed visszavonása: `DELETE FROM identity.alias WHERE
  method='import';` (a linker ezután nem old fel belőlük).
- A Tipplap-oddsok a többi forrás sorait nem érintik; a nap-API a
  trust-sorrenddel választ (verified előbb).
