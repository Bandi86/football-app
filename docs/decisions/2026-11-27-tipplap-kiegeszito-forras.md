# Döntés — 2026-11-27 — Tipplap mint kiegészítő forrás

## Kontextus

A 2026-11-27-i kézi összevetés (Tipplap kalkulátor ↔ `/v1/day/2026-11-27`)
szerint a Tipplap 128 mai eseményéből 117 megvan a rendszerben, 11 hiányzik:
2 ománi kupameccs, 1 szerb bajnokság-beli, 8 walesi 2. osztályú. A hiány oka
a Tipplap-adapter fix ligcímeinek lefedettsége (42 + 11 női), nem a jobok
hiánya. A Tipplap `/kalkulator/markets/{id}` végpont 32 piaccsoportot és
több mint 100 odds-ot ad meccsenként, de ez jelenleg nincs bekötve; az adapter
csak 1X2 nyitó szorzókat ír.

A felhasználó kérésére a Tipplapot kiegészítő forrásként kezeljük: minden
reggeli napi lista összeállításakor a rendszernek meg kell vizsgálnia, hogy
innen tud-e meccset emelni, és a gazdag piacokat is hasznosítania kell.
Egyben hangsúlyozta: a verseny- és csapatnevek először áttekintendők, a
megfelelő egyezést körültekintően kell megtalálni; sok az eltérő alias.

## Döntések

1. **Versenyek köre:** csak a mért három hiány — `Walesi 2., Déli csoport`,
   `Walesi 2., Északi csoport`, `Ománi Szultán Kupa`, `Szerb bajnokság` --
   kerül be az adapter fix szótárába, a
   [`2026-11-20-tipplap-league-coverage`](2026-11-20-tipplap-league-coverage.md)
   adatvezérelt szabálya szerint (≥2 szavazat, ≥80% többség, két tábla).
   A többi, ma megfigyelt címet nem vesszük fel találgatásra; a napi ellenőrzés
   fogja mérni és jelezni a további hiányokat. A `("GB","Scotland")`
   ismert ghost-entitásokat okozó párost nem másoljuk
   ([2026-11-23-gb-subregion-competitions](2026-11-23-gb-subregion-competitions.md)).

2. **Részletes piacok:** a `daily_budget`-ot emeljük, és **minden kanonikus
   meccshez linkelt Tipplap-eseményre** lekérjük a markets JSON-t. A kérések
   továbbra is a policy `min_delay_ms`/`jitter_ms` és robots/breaker őrei alatt
   futnak; 429/403 esetén a breaker azonnal stopol, nincs retry. A piacokat a
   meglévő `fact.odds_quote` sémába írjuk (`market`, `outcome`, `line`, `phase`,
   `bookmaker='Tippmix'`); új oszlop vagy új tábla nem kell. Egy forrásos odds
   ezért továbbra is **provisional** marad a trust gate szerint.

3. **Név-/identitáskezelés:** review-alapú, fail-closed. Új vagy feloldatlan
   Tipplap csapatnév → `identity.review_queue` pending sor + hiányjel, a meccs
   **kimarad**; csak reviewed alias, `active_ref`, strong/medium crosswalk vagy
   egzakt név ad `Exact`-et. Fuzzy egyezés (**0.6 küszöb**) kizárólag jelölt,
   automatikus összevonás tilos (AGENTS.md §6, S2b1 spec). Nem hozunk létre
   versenyt vagy csapatot „create_on_miss" módon. A meglévő CLI
   (`footballctl identity review/approve`) marad a döntéshozatal eszköze;
   külön review-UI nem része ennek a döntésnek.

## Következmények

- A napi listát intéző reggeli kör változatlanul fut (07:30 körül); a
  kiegészítés a meglévő Tipplap `discovery`/`refresh` jobok és egy új
  markets-job-család útján történik — nem új scrape-szerver, nem kézi import.
- A markets kérések száma napi ~100–130; a pontos kvótaértéket és a
  visszalépési (429/breaker) viselkedést a spec rögzíti.
- Az összevetésben a webfetch-szövegben szereplő 5 jegyű számok a
  `data-market-number` értékek voltak, nem event ID-k; a valódi event ID 7 jegyű
  (`data-event`, pl. `5621252`). A meccsek száma és a 11 hiány csapatnév + idő
  alapján változatlan.

## Visszaállás

- A döntés visszavonása: az új ligacímek eltávolítása a szótárból, a markets
  domain és a megnövelt kvóta visszaállítása, a jobok `cancelled` státuszba
  tétele. Az aliasok és review-sorok megőrizhetők (nem destruktív).
