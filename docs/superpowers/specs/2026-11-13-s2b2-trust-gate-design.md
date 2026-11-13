# S2b-2 — Bizalmi szintes publish gate (design)

**Dátum:** 2026-11-13
**Státusz:** design jóváhagyva szekciónként a felhasználó által; spec review-kapu előtt
**Bemenetek:**
- [S1 forráskatalógus](../../research/source-catalog.md) — jóváhagyott bizalmi mátrix
  (magas/közepes domainek, kétforrásos odds-szabály, display-only domainek,
  provider-függetlenség: „3 felület = 1 forrás”);
- [S1 záró review döntések](../../decisions/2026-11-12-s1-review-dontesek.md) — 16. pont
  (mátrix + adoptációs sorrend jóváhagyva);
- [Rendszertervezési spec](2026-11-09-football-app-v2-design.md) §7.10.1
  (mérvadó forrás vagy elegendő független megerősítés; szavazás nem elég);
- [S2b-1 spec](2026-11-13-s2b1-identity-layer-design.md) — az identitás-réteg,
  amire a gate épül;
- AGENTS.md (crate-határok, migrációs fegyelem, tesztelés, publikus repó).

## 1. Hatókör

Az S2b második alszelete: a meglévő verification gate **backward kompatibilis
bővítése** a jóváhagyott bizalmi mátrixszal.

### 1.1 Határvonalak (jóváhagyott döntések)

- **Trust tárolás:** új `ops.source_trust` tábla (source × domain × osztály +
  provider), adatvezérelt, a katalógusból seedelve; az S3 adapterek regisztrálnak.
- **Odds kétforrásos szabály MOST épül meg** (az S1 spec szerint S2 hatókör):
  a `fact.odds_snapshot` PK-ja `(match_id, source_id)`-re vált, `trust_state`
  oszlopot kap; a szabály mezőszintű (a meccs publish állapotát nem rontja).
- **Trust default:** trust-sor nélküli forrás = **medium** (konzervatív);
  a demo források seedelt medium sorokat kapnak provider NULL-lal.
- **Odds tolerancia:** paraméter, **default 0.0 (egzakt)**; az S3 állítja be
  a mért eltérés alapján.
- **Nem változik:** a gate állapot-logika (`evaluate_publish_state`), a
  parser-őrök (90 perc, élő részeredmény — S2a), az identitás-réteg (S2b-1),
  a display-only domainek (nincs gate-szabály), a shadow ág.

### 1.2 Nem célok (YAGNI)

- Nincs teljes mezőszintű provenance-trust mátrix minden domainre (C
  megközelítés elvetve; S8 kliens-jelzésnél újra elővehető).
- Nincs trust-regisztrációs CLI alparancs (a tábla seed SQL-lel karbantartható;
  az S3 adapterek regisztrációs felülete akkor jön, amikor kell).
- Nincs pre-gate trust-score réteg (B megközelítés elvetve: a logika két
  helyre szakadna).
- Nincs új külső függőség.

## 2. Architektúra

```
tick / footballctl ingest-fixture
        │ 1. trust + odds betöltés (storage)
        ▼
verification crate
        ├── VerificationContext (bővül: source_trust, odds_values, odds_tolerance)
        ├── TRUST_LEVEL szabály (a SINGLE_SOURCE helyébe, provider-függetlenséggel)
        ├── evaluate_odds_trust (külön függvény, NEM Finding → trust_state)
        └── gate (változatlan logika: blocker→withheld, warning→provisional)
        ▼
storage: ops.source_trust + fact.odds_snapshot (composite PK, trust_state)
        ▼
api /v1/day: odds objektum + trust_state (additív)
```

### 2.1 Crate-határok

- **`domain` bővül:** `TrustClass::{High, Medium, DisplayOnly}` (+ `as_db_str`),
  `SourceTrust { source_code, domain, trust_class, provider: Option<String> }`,
  `SourceOdds { source_code, provider: Option<String>, odds: OddsValues }`,
  `OddsValues` (home/draw/away/btts_yes/btts_no/over25/under25, f64),
  `OddsTrustState::{Verified, Provisional, NoOdds}`,
  `ODDS_TOLERANCE_DEFAULT: f64 = 0.0`. Tiszta, IO-mentes.
- **`verification` bővül:** a `VerificationContext` új mezői
  (`source_trust: &[SourceTrust]`, `odds_values: &[SourceOdds]`,
  `odds_tolerance: f64`); `TrustLevelRule` (új, a `SingleSourceWarningRule`
  törlődik — szemantikáját átveszi); `evaluate_odds_trust(odds_values,
  tolerance) -> OddsTrustState` tiszta függvény; a persist réteg írja a
  `trust_state`-et.
- **`storage` bővül:** `repositories::trust` (source_trust CRUD + lekérdezés
  forráskódokra), az odds_snapshot repository composite PK-val és
  trust_state-tel; a `day` repo az odds trust_state-et is visszaadja.
- **`worker` + `footballctl`:** a hívók betöltik a trust/odds adatokat a
  contextbe; a gate után meghívják az odds-kiértékelést és persistálják.
- **`api`:** a `/v1/day` odds objektuma additívan bővül `trust_state`-tel.
- **Swift kliens:** a DayModels Codable decode-ja az új mezővel nem bukhat;
  ha strict decode, a kliens modell frissítése a terv része (Core teszt).

## 3. Adatmodell (0006 migráció)

Tranzakciós migráció: `0006_trust_gate.sql`; sorrend: tesztDB → zöld → devDB
(külön engedéllyel); visszaállítási út dokumentálva.

| Változás | Részletek |
|---|---|
| `ops.source_trust` (új) | `source_id` uuid FK ops.source, `domain` text NOT NULL, `trust_class` text NOT NULL CHECK (high/medium/display_only), `provider` text NULL, `created_at`; PK (source_id, domain); index source_id-re |
| `fact.odds_snapshot` PK-váltás | `match_id` PK → composite `(match_id, source_id)`; a meglévő sorok megmaradnak; `trust_state` text NOT NULL DEFAULT 'provisional' CHECK (verified/provisional) |
| Seed | demo források: medium, provider NULL, domain fixture/kickoff + result + odds; a katalógus igazolt forrásai (footballdata high fixture/result, thefa high fixture, ESPN/FotMob/OpenFoot/Sky/Tipplap medium, stb.) az operations doc seed SQL-jében |

### 3.1 Trust-feloldás (a TRUST_LEVEL szabály logikája)

A meccs anyagi domainjei: **fixture/kickoff** és **result** (FT/HT).

1. A hozzájáruló források = a `source_values` forráskódjai.
2. Forrásonként a trust osztály a `source_trust` táblából (domainre);
   **hiányzó sor → medium** default.
3. **Ha bármely hozzájáruló forrás high** az adott domainben → nincs lelet
   (verified-eligible, egymagában is).
4. Egyébként a medium hozzájárulók **független provider-csoportjai**:
   provider NULL = önálló csoport; azonos provider = 1 csoport (a spec
   „3 felület = 1 forrás” szabálya). **≥2 csoport → nincs lelet;
   1 csoport → warning** („Egy független forrás (közepes bizalom) —
   provisional.”).
5. **Display-only domainek** (lineups/stats/logó/meta/Elo): nem
   gate-tényezők, nincs szabály.
6. A `display_only` osztályú hozzájárulás **nem számít** az anyagi
   domainekbe (nem ad verified-et, nem is ront).

A szabály domainenként külön értékeli a fixture/kickoff és result
domaineket; a leletek a meglévő Finding formátumban készülnek (rule_key
TRUST_LEVEL, severity warning, field a domain neve).

### 3.2 Odds-kiértékelés (mezőszintű, NEM Finding)

`evaluate_odds_trust(odds_values: &[SourceOdds], tolerance: f64) -> OddsTrustState`:

- **Verified:** ≥2 független provider-csoport, és a csoportok képviselőinek
  minden mezője (7 mező) egyezik ±tolerance-on belül. A csoport képviselője
  a csoport első forrása `source_code` szerint rendezve (determinisztikus);
  a csoporton belüli eltérő odds értékek nem blokkolnak (a képviselet a
  rendezett első forrás).
- **Provisional:** 1 csoport, vagy ≥2 de eltérő értékek.
- **NoOdds:** nincs odds sor.
- Az eredmény a `fact.odds_snapshot.trust_state`-be íródik (a meccs összes
  odds sorára ugyanaz az állapot); a meccs publish állapotát NEM érinti.
- A tolerance paraméter a contextben (default `ODDS_TOLERANCE_DEFAULT = 0.0`);
  az S3 éles adapterek állítják be mért adat alapján.

### 3.3 Backward kompatibilitás

- A demo 2 forrása medium + NULL provider → 2 független csoport → nincs
  warning → **20/20 verified_publish**; az odds párok egzakt egyezők →
  **verified odds**.
- A meglévő `ingest-fixture` CLI út viselkedése azonos (additív lépések).
- A `/v1/day` régi mezői változatlanok; az új `trust_state` additív.
- A SINGLE_SOURCE szabály törlése szemantikus csere: a TRUST_LEVEL veszi át
  (medium 1 csoport → warning). A meglévő tesztek frissítendők.

## 4. Adatfolyam

### 4.1 A verify_and_publish bővített folyamata

1. **Materialize** (változatlan) → kanonikus meccs + odds_snapshot sorok
   (forrásonként, composite PK).
2. **Trust betöltés:** a storage trust repo a hozzájáruló forráskódokra
   lekéri a source_trust sorokat; hiányzó sor → medium default.
3. **Odds betöltés:** a meccs odds_snapshot sorai forrásonként (provider-rel)
   → SourceOdds lista.
4. **VerificationContext:** snapshot + identitás flagek (S2b-1 linker) +
   duplikátum + source_values + source_trust + odds_values + odds_tolerance.
5. **Engine:** meglévő szabályok + TRUST_LEVEL → report → persist → gate
   (változatlan állapot-logika).
6. **Odds-kiértékelés a gate után:** evaluate_odds_trust → trust_state
   UPDATE a meccs odds sorain. NEM Finding, a gate állapotát nem érinti.
7. **Shadow ág:** változatlan (nincs kanonikus írás, nincs trust/odds
   kiértékelés).

### 4.2 API és CLI

- `/v1/day`: az odds objektum additívan bővül `trust_state`-tel
  (verified/provisional); a kliens megjelenítés S8.
- `footballctl day`: a JSON kimenet tartalmazza az odds trust_state-et.
- Swift DayModels: Codable decode az új mezővel (Core teszt; ha strict
  decode, a modell frissítése a terv része).

## 5. Hibakezelés és fail-closed garanciák

1. **Trust betöltési hiba → medium default:** a context üres trust-listával
   áll fel → minden forrás medium → egymagában provisional. A gate sosem
   enged át high-ként betöltetlen forrást. DB-leállásnál a materialize is
   bukik — a tick naplóz és folytat (S2a minta).
2. **Odds UPDATE hiba → provisional marad** (default); a gate állapota
   megmarad; a hiba naplózott, a következő tick újraértékeli.
3. **Ismeretlen domain a trust táblában → medium default.**
4. **Provider-ütközés:** azonos provider = 1 csoport; NULL = önálló csoport.
5. **Karantén-kapcsolat:** a trust/odds réteg nem karanténol; a parse-karantén
   (S2a) és az identitás-withheld (S2b-1) érintetlen.
6. Nincs production unwrap/expect; magyar hibaüzenetek; típusos hibák a
   meglévő anyhow-láncon.

## 6. Tesztelés és ellenőrzés

1. **Domain unit tesztek:** TrustClass db-stringek; OddsValues egyezés
   ±tolerance (határeset: pontosan a tolerance); provider-csoportosítás
   (NULL önálló, azonos provider egy csoport).
2. **TRUST_LEVEL szabály tesztek:** high egymagában → nincs lelet; medium
   1 csoport → warning; medium 2 független → nincs lelet; azonos provider
   2 forrás → 1 csoport → warning; display_only nem számít; trust-sor
   nélküli → medium default; mindkét anyagi domain külön.
3. **evaluate_odds_trust tesztek:** 2 független egyező → Verified; 2 eltérő
   → Provisional; 1 forrás → Provisional; üres → NoOdds; tolerance határeset.
4. **Storage integrációs tesztek:** source_trust CRUD + default; odds
   composite PK (két forrás egy meccsen), trust_state UPDATE; 0006
   séma-teszt (PK, CHECK, meglévő sorok megmaradnak).
5. **E2E (worker + CLI):** demo → 20/20 verified + verified odds; egyforrású
   medium → provisional_publish; high egymagában → verified; eltérő odds
   párok → provisional odds, de a meccs verified marad; shadow érintetlen.
6. **API/kliens:** /v1/day odds trust_state; Swift DayModels decode teszt.
7. **Regresszió:** teljes workspace suite zöld; demo 20/20.

**Kötelező ellenőrzések a szelet végén:** `./scripts/check.sh`,
`db-reset + db-check` (tesztDB 0 üzleti sor), magyar visszaolvasás +
glyph-scan, `footballctl day --date 2026-11-11` → 20/20 verified + odds
trust_state, Swift Core teszt.

## 7. Sikerkritériumok (Definition of Done)

- Minden teszt futott és zöld; parancsok + kimenetek a checkpointban.
- Élő hálózati kérés 0; nyers payload a repóban 0.
- Demo 20/20 verified_publish + verified odds.
- 0006 migráció tesztDB → zöld → devDB (külön engedéllyel); visszaállítási
  út dokumentálva.
- Operations doc: `docs/operations/trust.md` (trust tábla karbantartása,
  seed SQL, odds szabály, provider szabály).
- Spec + terv + checkpoint magyarul, visszaolvasva.

## 8. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| A demo odds párok nem egzakt egyezők → a 20/20 verified odds bukik | **Ellenőrizve (2026-11-13): mind a 20 odds pár egzakt egyező (0 eltérés), canonical id szerint joinolva** — a default 0.0 toleranciával a demo verified odds zöld |
| A Swift kliens strict decode az új mezőn bukik | Core decode teszt az új mezővel; ha strict, a modell frissítése a terv része |
| A provider-csoportosítás túl szigorú (valódi független források azonos provider) | a provider mező NULL = önálló csoport; a seed csak valódi provider-csoportokat ad (pl. ugyanazon adatszolgáltató felületei); S3-ban felülvizsgálandó |
| A trust default medium új forrásokat provisional-ban tart | a regisztráció (seed SQL) az operations docban dokumentált; az S3 adapterek regisztrálnak |

## 9. Visszaállítási út

- Kód: feature branch (`feat/s2b2-trust-gate`), commit csak felhasználói
  kérésre; elvetés `git restore` csak kifejezett engedéllyel.
- DB: a 0006 tranzakciós; visszaállítás: odds_snapshot PK visszaállítása
  match_id-re (a composite sorok deduplikálandók — dokumentált), trust_state
  oszlop drop, source_trust drop; tesztDB-n gyakorolva.
- Gate: a TRUST_LEVEL visszaállítható a SINGLE_SOURCE-ra git reverttel
  (a demo pipeline érintetlen).

## 10. Következő lépés

A spec felhasználói review-ja után a writing-plans skill következik
(megvalósítási terv: `docs/superpowers/plans/2026-11-13-s2b2-trust-gate.md`).
Implementáció csak a terv jóváhagyása után, szeletenkénti stop-pontokkal.
