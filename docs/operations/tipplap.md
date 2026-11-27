# Tipplap forrás (tipplap)

Frissítés: 2026-11-27 (S191–S193: kiegészítő forrás, részletes piacok).
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
- Policy: `rust_http`, `canary`, **`daily_budget=150`** (S192: a markets
  kérések miatt), `min_delay_ms=3000`, `jitter_ms=1000`, domain `day`.
  Események (S36b óta):
  - `discovery`, `interval_minutes=1440` — napi egyszeri teljes nap;
  - `refresh`, `interval_minutes=360` — napon belül 4 sáv (00/06/12/18 UTC),
    összesen 5 kérés/nap.
- **Részletes piacok (S192):** a `markets` domain nem szerepel a policy
  `domains` listájában (így a sablonok nem generálnak érvénytelen `all`/
  `slot-*` jobot); a scheduler a mai, már linkelt Tipplap-eseményekből
  `markets-{event_id}` jobokat generál, **1 kérés/meccs**:
  `/kalkulator/markets/{event_id}`. A napi ellenőrzés méri a done/failed/pending
  jobokat és a linkelt események számát. A `min_delay_ms`/`jitter_ms`, robots és
  breaker változatlan; 429/403 esetén azonnali stop, nincs retry.

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

S191 óta a `GENERAL_LEAGUES` 30 cím: a 2026-11-27-i összevetésben mért
`Walesi 2., Déli csoport` (`Cymru - dél`, GB-WLS), `Walesi 2., Északi
csoport` (`Cymru - észak`, GB-WLS), `Szerb bajnokság` (`Super Liga`, RS) és
`Ománi Szultán Kupa` (`Sultan Cup`, OM). A GB/Scotland ismert ghost-párt
nem másoljuk; döntés: `docs/decisions/2026-11-27-tipplap-kiegeszito-forras.md`.

S36a óta a tábla két részből áll: a nemzetközi/top-ligás `LEAGUES`
(prefix-illesztés, forduló-címkével) és a `GENERAL_LEAGUES` (26 hazai/alsóbb
ligás cím, ékezet- és kisbetű-toleráns **pontos** egyezéssel, forduló-címke
nélkül). A bővítés adatvezérelt volt: csak olyan cím került be, amelyre a
v1 Tipplap-események és a v2 meccsek egzakt csapatpár-egyeztetése legalább
2 szavazatot és ≥80% többséget adott (részletek:
`docs/decisions/2026-11-20-tipplap-league-coverage.md`). Ismeretlen cím
továbbra is kimarad.

## Csapat-feloldás

- Nemzetközi verseny: a csapatnév ország → kanonikus EN név + ISO
  (`countries.rs` lokalizált/angol táblák); ismeretlen név → a sor kimarad.
- Hazai liga: a megfigyelt név megy a fail-closed linkerbe; a
  `footballctl identity seed-v1` a v1 crosswalkból tölt reviewed
  aliasokat (nyers + ékezet nélküli alak), így pl. „Ferencvárosi TC" és
  „Ferencvaros" is feloldódik. Ismeretlen csapat → review-sor.

## Részletes piacok (S192)

A `/kalkulator/markets/{id}` JSON `desktop` fragmentje `data-market-name`,
`data-outcome-name/code/id` és `data-odds` cellákat tartalmaz. A fix
piac-katalógus (`tipplap.rs::market_for`) determinisztikusan képezi le:

| Tipplap piac | `market` | kimenetel | vonal |
|---|---|---|---|
| 1X2 | `1x2` | home/draw/away | 0 |
| Kétesély | `double_chance` | 1x/12/2x | 0 |
| Döntetlennél a tét visszajár | `dnb` | home/away | 0 |
| Mindkét csapat szerez gólt | `btts` | yes/no | 0 |
| Gólszám 0,5 / 1,5 / 2,5 / … | `ou` | under/over | 0,5 / 1,5 / 2,5 / … |
| Hendikep 0:1 / 1:0 | `handicap_-1:<id>` / `handicap_+1:<id>` | home/draw/away | 1 (előjel a marketben) |
| Hazai/vendég clean sheet | `home_clean_sheet` / `away_clean_sheet` | yes/no | 0 |
| Pontos végeredmény | `exact_score` | eredeti kód | 0 |
| Ismeretlen piac | `other:<outcome-id>` | eredeti név | adott esetben > 0 |

Az API által használt kanonikus piacok (`1x2/btts/ou/dnb/double_chance/
clean_sheet`) változatlanok, hogy a `COALESCE` továbbra is megtalálja őket;
minden más piac a forrás `outcome-id`-jével egyedített (a `fact.odds_quote`
elsődleges kulcsa miatt). Ismeretlen piac **nem vész el**.

A markets job a napi `match_odds` observationt frissíti ugyanazzal a
`source_record_key`-vel; a `write_market_odds()` a teljes forrás-blokkot
cseréli és `provisional` trust-ra állítja vissza — a meglévő trust-gate és
publicitási lánc változatlan. Az új piacok megjelenítése külön UI-szelet.

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

## Szótár-bővítés (S22, 2026-11-17)

Új bejegyzések a fix verseny-szótárban: `NB II` (HU, tier 2, priority 10)
és `CONCACAF Nemz.Ligája A/C div` → `CONCACAF Nations League - League A/C`.
Az U19/U21 Eb-selejtező szándékosan kimarad: a Soccerway kanonikus neve a
fordulót is tartalmazza (`Round 2`), ami ablakonként változik — ezt a
verseny-modell külön round_label mezője oldaná meg.

## Napon belüli frissítés (S36b, 2026-11-20)

- A napi `discovery` mellett `refresh` sávok futnak (360 percenként, azaz
  00/06/12/18 UTC-kor); a sávonkénti job a teljes napi listát kéri le, így a
  nap összes meccsének nyitó szorzója frissül.
- Éles mérés a S36a telepítés után: a mai napon **29** oddsos v2-meccs
  (87 Tipplap-quote, 19 verseny) a korábbi 4 helyett; a maradékot a nyitott
  csapat-identitás review-k fogják vissza, nem a liga-lefedettség.
- Visszaállítás: a `refresh` esemény törlése a policy-ból
  (`events` jsonb-ből), a napi discovery marad.
