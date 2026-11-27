# 2026-11-27 — Tipplap mint kiegészítő forrás — rendszertervezés

**Státusz:** JÓVÁHAGYOTT (2026-11-27)
**Döntések:** [`2026-11-27-tipplap-kiegeszito-forras`](../../decisions/2026-11-27-tipplap-kiegeszito-forras.md)
**Megvalósítási terv:** [`2026-11-27-tipplap-kiegeszito-forras`](../plans/2026-11-27-tipplap-kiegeszito-forras.md)

---

## 1. Cél és határ

**Cél:** a Tipplap napi kalkulátora és `markets` JSON-je legyen a meglévő
források kiegészítője: (a) a mért hiányzó versenyek meccsei jussanak be a napi
listába, (b) a gazdag piacok (BTTS, gólszám, hendikep, pontos eredmény, stb.)
kerüljenek be az odds-tárolóba, (c) minden reggeli napi-lista-összeállítás
vegye figyelembe ezt a forrást.

**Nem része:** új fogadóiroda, valódi pénzes fogadás, automatikus kifizetés;
fuzzy entitás-összevonás vagy `create_on_miss`; kézi adatbevitel; a research /
footballdata / Sroma források módosítása; a Tipplap HTML-szerkezetének
megkerülése (bot challenge, rate limit). Nem része a 11-nél több új ligacím
felvétele sem — azt a napi mérés fogja kérni.

## 2. Kiindulás (ellenőrzött tények)

- A `tipplap` adapter már él: `rust/crates/ingest/src/tipplap.rs`, regisztrálva
  a `footballd` és `footballctl` binárisokban. Napi egy kérés a
  `/kalkulator/labdarugas/{date}` oldalra, `data-event` ID, `data-event-name`,
  `data-odds`, dátum-guard (`#dates selected`) és üres-tábla guard fail-closed.
- Fix szótár: `LEAGUES` (16), `GENERAL_LEAGUES` (26), `WOMEN_LEAGUES` (11).
  Ismeretlen cím → sor kimarad, nem jön létre verseny.
- Csapatnevek: nincs adapter-szintű alias-szótár; a nyers HU nevet az
  `identity` linker oldja fel (reviewed alias → exact → crosswalk → fuzzy
  jelölt → review). Fuzzy **soha** nem Exact (AGENTS.md §6).
- Odds ma: `bookmaker='Tippmix'`, `market='1x2'`, `phase='opening'`, 3 quote.
  A `/kalkulator/markets/{id}` JSON (`desktop` + `mobile` HTML-fragment,
  `data-market-number`, `data-market-name`, `data-outcome-name/code/id`,
  `data-odds`) nincs bekötve.
- Kvóta (dev DB `ops.source_policy`): `daily_budget=8`, `min_delay_ms=3000`,
  `jitter_ms=1000`, `domains=["day"]`, `events=[discovery 1440, refresh 360]`.
  Ma reggel 5 kérésből 3-ször ismétlődött a napi parse (89 sor, 60 kihagyva).
- A 2026-11-27-i összevetés: Tipplap 128 esemény, 117 fedett, 11 hiány
  (2 ománi, 1 szerb, 8 walesi 2.). A 11-ből a versenycímek hiányoznak a
  szótárból; a csapatnevek többségét a linker már feloldaná, de új HU nevek
  review-sort nyitnak (élő példák: `Inter Kashi FC`, `Nacional Madeira`).

## 3. Adatmodell és piac-leképezés

Új migráció **nem kell**: a `fact.odds_quote` (`match_id`, `source_id`,
`bookmaker`, `market`, `outcome`, `line`, `phase`, `price`) alkalmas minden
piacra; az `odds_snapshot.market_count` a lekérések számát növeli.

A Tipplap magyar piacneveiből determinisztikus, verziózott leképezés (fix
tábla, nem találgatás):

| Tipplap `data-market-name` (minta) | `market` | `outcome` | `line` |
|---|---|---|---|
| `1X2` / `Kétesély` (hazai/döntetlen/vendég) | `1x2` | `home`/`draw`/`away` | 0 |
| `Mindkét csapat szerez gólt` | `btts` | `yes`/`no` | 0 |
| `Gólszám 2,5` / `Gólszám 1,5` / `3,5` | `ou` | `over`/`under` | 1.5 / 2.5 / 3.5 |
| `Hendikep 0:1`, `1:0`, `2:0` | `handicap` | `home`/`draw`/`away` | −1 / +1 / +2 |
| `Pontos végeredmény` | `exact_score` | `01`, `02`, … | 0 |
| `1. félidő - 1X2` | `ht_ft`-szerű `ht_1x2` | `home`/`draw`/`away` | 0 |
| Ismeretlen piac | `other` | `raw` kód | 0 |

Szabály: az ismeretlen piacot **nem szabad eldobni** (adatvesztés), de
`market='other'` alatt, az eredeti névvel kell tárolni, és a napi ellenőrzés
jelezze a katalogizálatlan piacot. A `data-outcome-code` (H/D/V/Igen/Nem)
fix leképezés; a `line` a piacnévből, szabály alapján. A `desktop` fragments
az elsődleges (a `mobile` duplikát, csak validálásra használható).

A Quote `phase='opening'`, `bookmaker='Tippmix'`, `source_id=tipplap` —
ugyanaz a snapshot-sor, mint az 1X2-nél; `write_odds()` teljes forrás-blokkot
cserél, tehát a markets jobnak **az összes piacot együtt** kell írnia.

## 4. Identitás és névkezelés (kötelező őrök)

1. A versenycímek kizárólag a fix `League` táblákon keresztül jutnak be. Új
   cím = új táblaelem, kézzel, két szavazattal és ≥80% többséggel; a GB-SCT
   párost nem használjuk.
2. A csapatnév HU alakját az adapter változatlanul küldi az
   `identity::linker`-nek. Reviewed alias/crosswalk/exact nélkül a meccs
   **kimarad**, review-sor nyílik (fail-closed). A fuzzy jelöltet a CLI-vel
   kell reviewed aliasként jóváhagyni — a rendszer nem választ automatikusan.
3. A forrás stabil `data-event` ID crosswalk-jelölt: az event ID-t az
   `identity.source_entity_ref`-ben kell megőrizni; strong crosswalk csak
   kézi/ellenőrzött lépéssel létesül.
4. A piacok az **adott kanonikus meccsre** írónak; ha a forrás-sor csak
   névvariáns révén linkelhető (S37b), a link confidence és a kanonikus sor
   érintetlensége megmarad.
5. Minden megjelenített tényhez source/captured_at/provenance tartozik
   (AGENTS.md §6); a Tipplap odds egyforrásos → provisional.

## 5. Job-lánc és kvóta

- Meglévő reggeli kör: `discovery 1440` (entity_key `all`) + `refresh 360`
  (`slot-00..03`) = 4 + 1 kérés. A napi lista többi forrása változatlan.
- Új `markets` domain a `tipplap` policy-ben: `event_type='refresh'`,
  napi/órás sáv; a scheduler a napi `match_core` observationökből generál
  `(tipplap, markets, "markets-{event_id}", refresh, today)` jobokat. A
  `TipplapAdapter::plan()` a `markets` domainre `/kalkulator/markets/{id}`-t
  ad, `parse()` a JSON-t olvassa és `match_odds` observationöket emits.
- Kvóta: `daily_budget` a mért igényre emelendő (kb. 130–150); a delay/jitter,
  robots és breaker változatlan. A szekvenciális feldolgozás és a 429/403
  fail-closed marad; a túllépett napi keret miatt a hátralévő jobok
  `pending`-ben maradnak és a következő napon **nem** pótolódnak régi dátumra
  (stale-slot szabály).
- A markets jobok csak **linkelt** (kanonikus `match_id`-vel rendelkező)
  eseményekre jönnek létre; a feloldatlan review-sorok kimaradnak.

## 6. Napi ellenőrzés és megfigyelhetőség

- `daily-check`: a Tipplap napi lefedettségét mérje (felvett események száma,
  kihagyott ismeretlen cím/team, markets jobok sikere), és hiányjelet írjon
  az új, katalogizálatlan nevekről.
- A `docs/operations/tipplap.md` frissül: új címűek, piac-tábla, kvóta, review
  folyamat, visszaállás.

## 7. Szeletek (részletes terv külön fájlban)

- **S0** — döntések, spec, terv (dokumentáció).
- **S1** — három új versenycím + piac-katalógus és parser (adapter + tesztek).
- **S2** — markets jobok: scheduler + policy + worker + adapter plan/parse,
  observation → odds írás, teljes kapulánc.
- **S3** — napi ellenőrzés, hiányjelek, üzemeltetési doc, éles mérés.

## 8. Kapu-lánc (minden kód-szelet után)

```bash
set -a && . ./.env && set +a
cd rust && cargo fmt --all --check
cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p footballd -p footballctl --features obscura
cd .. && ./scripts/check-foundation.sh
./scripts/daily-check.sh 2026-11-26          # EXIT=0
footballctl integrity --from 2026-11-25 --to 2026-11-27  # OK
git diff --check
```

Adapter-változásnál a `tipplap-replay` offline útján kell igazolni a parser
változást valódi, tárolt HTML-en; hálózati kérést csak a policy és robots
engedélyez. Új jobnál a `footballd` újratelepítése és `/v1/health` kötelező.

## 9. Kockázatok / visszaállás

- **Kockázat:** a markets végpont 1 kérés/meccs; ha a site szerkezete változik,
  a parser fail-closed (üres lista → `SchemaDrift`), a nyers JSON a tárban marad.
- **Kockázat:** a review-sorok száma megnőhet (új HU nevek); ez a rendszer
  kívánt viselkedése, nem hiba — a napi ellenőrzés jelzi.
- **Visszaállás:** szeletenkénti `git revert`; a policy kvótája és domains
  listája visszaállítható egy SQL-lel; a jobok `cancelled` státuszba tétele
  nem törli a már rögzített raw/observation/audit sorokat. Az új ligacímek
  eltávolítása után a következő napi kör ismét kihagyja ezeket a meccseket
  (a kanonikus adat nem változik).
