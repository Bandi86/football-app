# Versenyek, Csapatok, Üzemeltetés — bővítési terv (vázlat)

**Dátum:** 2026-11-25
**Státusz:** VÁZLAT / tervezési dokumentum — még nincs jóváhagyva, nem tartalmaz kódot
**Cél:** a hátralevő három főoldal (`Versenyek`, `Csapatok`, `Üzemeltetés`) olyan
bővítése, hogy a felhasználó **átlássa a adatproblémákat, be tudja avatkozni,
és az ügynökökkel közös jegyzeteket tudjon vezetni**. Az Üzemeltetés lesz a
központi „irányítópult"; a másik két oldal a saját entitásaira szűkített
nézetet ad.

---

## 1. Alapelvek (a meglévő rendszerhez igazodva)

1. **A forrás az elsődleges, a kézi beavatkozás auditált revízió.** A mostani
   `PUT /v1/match/{id}/manual-result` mintájára: minden kézi írás
   `raw.capture → stage.observation → fact.provenance → ops.audit_event` nyomot
   hagy, és az újraellenőrzés (publish gate) újrafut — a kliens nem kerüli meg
   a verification láncot (`docs/operations/manual-entry.md`).
2. **Bizonytalan adat = NULL/státusz, soha nem kitalált érték** (AGENTS.md §6).
   A UI a hiányt hiányként mutatja, nem monogram/logó nélküli „0"-ként.
3. **Nincs automatikus fuzzy merge** (AGENTS.md §6). A javasolt entitás- és
   fixture-összevonások mind **kézi, bizonyíték-alapú döntés** maradnak.
4. **Minden megjelenített tényhez tartozik forrás, rögzítési idő, ellenőrzési
   állapot** (AGENTS.md §6). A „mi történt, mikor" idővonal kötelező.
5. **Publikus repó** (AGENTS.md §7): a feltöltött logók és jegyzetek nem
   kerülhetnek a repóba; csak a tartalom-címzett asset-tárba (`ops.asset`),
   illetve az adatbázisba. Titok, személyes adat, token nincs a UI-ban.
6. **Fokozatos bevezetés**: egy szelet = egy mappás, tesztelt változás; minden
   szelet végén kapu (`cargo test`, `clippy`, `fmt`, UI teszt, `daily-check`).

---

## 2. Jelenlegi állapot (amire épülünk)

| Terület | Van | Nincs / hiányos |
|---|---|---|
| **Versenyek** (`CompetitionsView`, `CompetitionsModel`) | napi versenyek, keresés, létszámok, `CompetitionSheet` | nincs verseny-szintű hiányösszefoglaló, nincs „meccsáttekintés", nincs verseny-logó feltöltés, nincs jegyzet |
| **Csapatok** (`TeamsView`, `TeamsModel`, `/v1/teams`, `/v1/team/{id}`) | csapatlista, csapat részlet, `TeamSheet` | nincs logófeltöltés UI, nincs identitás-review/konfliktus nézet, nincs „mely meccsei hiányosak" nézet |
| **Üzemeltetés** (`OperationsView`, `OpsModel`, `/v1/ops/status`) | `OpsStatus`: pending/leased jobok, quaratine, pending review, source-állapot, `OpsIntegrity` (duplikátum, árva provenance, withheld, failed job, breaker) | csak pillanatkép számok; **nincs idővonal**, nincs részletes hiba-jel, nincs keresés/szűrés, nincs javítási akció, nincs audit-esemény feed, nincs Gemma-kapcsoló |
| **Kézi szerkesztés** | státusz, kickoff/pointosság, FT/HT/ET/pens, indoklás (`manual-entry.md`) | nem szerkeszthető a **teljes meccsadat** (csapatok, verseny, forduló, logók), nincs mező-szintű „mi változott" előnézet |
| **Logók** | 6411 csapat-, 124 verseny-asset; import scriptek (`team-logos.md`) | sok entitásnak nincs képe; nincs UI feltöltés; a hiány az identity-duplikátumoktól is jön (`remaining-gaps.md`) |
| **Jegyzetek** | `ops.audit_event` (actor, action, subject, detail, created_at) | nincs külön notes tábla, nincs API, nincs UI; az audit nem szerkeszthető (az nem is cél) |
| **AI** | `football-ai` crate, `footballctl ai search|ask`, Magnitude gemma 4 + SearXNG (`README`, `2026-11-20-local-ai.md`) | nincs UI kapcsoló, nincs „keressen segítséget a hiányhoz" gomb |

A napi `daily-check` és az `integrity` riport ma is `OK`/`ZÖLD`; a hiányok
(a `docs/operations/remaining-gaps.md` szerint) **nem kemény hibák**, hanem
láthatatlanok a felhasználó előtt — ezt kell megváltoztatni.

---

## 3. Közös adatmodell-kiegészítés (tervezett)

Új táblák (migrációval, tranzakciósan, visszaállítási útmal):

### 3.1 `ops.match_note` — ügynök/felhasználói jegyzet meccshez

| oszlop | jelentés |
|---|---|
| `id` | UUIDv7 |
| `match_id` | `core.match.id` FK |
| `author` | `ui` \| `agent:<név>` \| `import:<forrás>` |
| `body` | szöveg (a felhasználó és az ügynökök egymásnak írják) |
| `kind` | `info` \| `problem` \| `fix` \| `question` \| `resolved` |
| `created_at`, `updated_at` | időbélyeg (a „mi történt, mikor" alapja) |
| `resolved_at` | opcionális, ha `kind=fix`/`resolved` |

Szabály: a `body` **nem** kerülhet a nyilvános napi feedbe; csak ops tokenes
végponton. A jegyzet nem írja felül a kanonikus adatot, és nem vált ki gate-et
( Ellenben az audit esemény igen: `note_added`, `note_edited`).

### 3.2 `ops.issue` — strukturált hiány/jelzés („hibára utaló jel")

| oszlop | jelentés |
|---|---|
| `id`, `subject_kind` (`match`/`team`/`competition`), `subject_id` | mit érint |
| `code` | pl. `missing_logo`, `missing_odds`, `open_past`, `duplicate_suspect`, `identity_review`, `stale_status`, `source_conflict` |
| `severity` | `info` \| `warning` \| `hard` |
| `detail` | JSON (forrás, érték, elvárt érték, utolsó láttatás) |
| `first_seen_at`, `last_seen_at`, `resolved_at` | idővonal |
| `source` | melyik ellenőrzés generálta (`daily-check`, `integrity`, `manual`, `agent`) |

Ez a tábla a **detektált jelek tartalma**, nem csak pillanatnyi számláló —
így a UI meg tudja mutatni: *mióta van a probléma, mikor volt utoljára látható,
kioldotta-e már valaki*.

### 3.3 `ops.asset` kiterjesztés — feltöltött logó

A meglévő `ops.asset` táblához: `origin` (`import` \| `user_upload`),
`uploaded_by`, `original_filename` (csak a megjelenítéshez; nem tartalmaz
titkot), `mime_type`, méret-korlát (pl. 2 MB), engedélyezett formátum
(WebP/PNG/SVG). A fájl tartalom-címzett kulccsal a már meglévő asset-tárba
kerül (`docs/operations/team-logos.md` mintájára), nem a repóba.

---

## 4. API-kiegészítés (tervezett, mind ops tokenes írás)

| Végpont | Művelet |
|---|---|
| `GET /v1/issues?scope=day&date=…&kind=…&severity=…` | hiányok/jelek listája idővonallal |
| `GET /v1/match/{id}/notes`, `POST …`, `PATCH /v1/note/{id}` | jegyzetek olvasása/írása/szerkesztése |
| `GET /v1/match/{id}/history` | mező-szintű változástörténet (forrás, idő, before/after) — az `ops.audit_event` + provenance alapján |
| `PUT /v1/match/{id}/full` | **teljes meccsadat** kézi szerkesztése (l. 5.) |
| `POST /v1/asset/upload` | logó feltöltése lokális gépről (multipart; entity_kind + entity_id + origin=user_upload) |
| `GET /v1/ops/summary` | napi/heti összefoglaló: hol van hiány, mennyi, trend, kiemelt akciók |
| `POST /v1/ops/ai-assist` | későbbi fázis: a hiány kontextusával hívja a helyi gemma 4-et (SearXNG-vel), javaslatot ad — **csak javaslat**, nem írás |

Az írási végpontok mind: 401 ops token nélkül, 400 magyar mezőhibával,
409 natural-key ütközésnél, 500 váratlan hibánál; minden írás auditált.

---

## 5. Teljes meccsadat kézi szerkesztése

A mai `manual-result` helyett/kiegészítésére egy **„Teljes szerkesztés"** mód:

- **Identitás:** home/away csapat (csak létező entitásból, kereséssel — nincs
  szabad szöveges létrehozás), verseny, szezon, forduló, kickoff + pontosság.
- **Státusz/eredmény:** a mai szabályok változatlanul (FT/HT/ET/pens,
  `status_admits_result`, `decided_by`).
- **Logók:** a csapatokhoz rendelt asset kiválasztása vagy feltöltése (l. 6.).
- **Előnézet:** a mentés előtt mezőnként `régi → új` + forrás + indoklás;
  a natural-key ütközést a mai `409 conflict_match_id` jelzi.
- **Hatás:** ugyanaz a bizonyíték- és provenance-lánc, mint a `manual-result`
  (`policy='manual-full-1.0'`), újraellenőrzés + publish gate; a megváltozott
  kickoff új publish-dátumot ír (a mai `kickoff_move` logika alapján).

Döntés kell: a `manual-result` és a `full` végpont **egyesüljön** (egy végpont,
opcionális mezőkkel) vagy külön maradjon. Javaslat: egy `PUT /v1/match/{id}/edit`
a régi végpont megőrzésével (kompatibilitás).

---

## 6. Logó feltöltése lokális gépről

**UI:** a `TeamSheet`-ben és a meccs-szerkesztőben „Logó feltöltése" gomb
(`asset.upload`); fájlválasztó, előnézet, méret/formátum-ellenőrzés, majd
mentés. Verseny-logó ugyanígy a `CompetitionSheet`-ben.

**Backend:** multipart feltöltés, tartalom-hash, duplikátum-detektálás,
`ops.asset` sor `origin='user_upload'` + `uploaded_by='ui'` + audit esemény.
A fájl **csak** az agent asset-könyvtárába kerül (a repón kívül), az API
`/v1/asset/{storage_key}` útvonalon szolgálja ki — ez már ma is működik.

**Biztonság:** név-kiterjesztés ellenőrzés, MIME-sniff, méretkorlát, SVG-nél
szkript-mentesítés vagy SVG tiltása az első szeletben (döntés kell). A feltöltő
nem adhat meg más entitás ID-t, mint amelyiket a UI épp szerkeszt (token + UI
által küldött subject ellenőrzés).

---

## 7. Az oldalak részletes képe

### 7.1 Üzemeltetés (`OperationsView`) — **kiemelt fontosságú**

Négy fül/szekció:

1. **Állapot most** — a mai `OpsStatus` kártyák, de **számláló helyett
   kattintható hivatkozással**: pl. „árva provenance: 0" zöld, „failed job: 1"
   sárga → kattintva a konkrét események listája (idővel, forrással, részlettel).
2. **Hiány-jelzések (`ops.issue`)** — szűrhető `subject`/`code`/`severity`/
   `first_seen` szerint; minden sornál: *mi a hiba, mióta, utoljára mikor
   látták, ki oldotta meg*. Keresés, rendezés, „csak új", „csak rám bízott".
3. **Idővonal / audit** — az `ops.audit_event` és a `match_note` egységes,
   időrendi feedje: `ki | mikor | mit | melyik meccs/entitás | before→after`.
   Itt látszik az is, hogy az ügynök „itt valami rossz" / „ezt javítottam"
   bejegyzései mikor történtek.
4. **Akciók** — javítási műveletek közvetlenül a sorból: „megnyit a meccsnél",
   „jelöl megoldottnak", „újraellenőrzés", „logó feltöltése", „AI-támogatás
   kérése" (későbbi fázis). Csak ops tokenes művelet, mind auditált.

**Összefoglaló (`/v1/ops/summary`)**: napi/heti trend (pl. „a hiányzó logók
száma 24 → 15", „3 új duplikátum gyanú"), kiemelt 3–5 akció, és egy mondatos
„hol van a legnagyobb gond" magyar szöveg. A cél: belépéskor 10 másodperc
alatt átlátható legyen, mit kell ma megjavítani.

### 7.2 Versenyek (`CompetitionsView`)

- Versenyenként: meccsszám, lezárt/eredmény/odds/logó arány, **hiány-jelzések
  száma** (az `ops.issue` alapján), utolsó frissítés, források.
- Kattintás → `CompetitionSheet`: a verseny meccsei, rendezhető státusz/
  hiány szerint; verseny-logó feltöltése; verseny-szintű jegyzet.
- „Meccsáttekintés" gomb → az adott verseny összes hiányos meccse egy listában,
  átugrással a teljes szerkesztőbe.

### 7.3 Csapatok (`TeamsView`)

- Csapatonként: logó (vagy monogram + „nincs logó" jelzés), meccsszám,
  identitás-státusz (aktív / review / összevont), **hiány-jelzések**.
- `TeamSheet`: logó feltöltése/cseréje, aliasok, forrás-hivatkozások,
  összevonási előzmény (`dedupe_archive`), csapat-szintű jegyzet,
  „mely meccsei hiányosak" áttekintés.
- Szűrés: „csak logó nélküliek", „csak review alatt", „csak összevontak".

### 7.4 Meccs-részletező (közös érintési pont)

- „Jegyzetek" panel: az ügynökök és a felhasználó egymásnak írt bejegyzései,
  típus-címkével (`problem`/`fix`/`question`), idővel, szerkeszthetően.
- „Előzmények" panel: a meccs mező-szintű változásai (forrás + idő + ki írta).
- „Teljes szerkesztés" gomb az 5. pont szerint.

---

## 8. Gemma 4 / AI-támogatás (későbbi fázis, most csak vázlat)

- Az Üzemeltetés hiány-soránál és a meccs-szerkesztőben egy **„Kérjen
  segítséget"** gomb; a háttérben a meglévő `football-ai` crate hívja a helyi
  gemma 4-et SearXNG-vel, a hiány kontextusával (csapat, verseny, dátum, forrás).
- **Csak javaslat**: az AI nem ír kanonikus adatot, nem old meg review-t, nem
  tölt fel logót. A javaslat `match_note(kind='info', author='agent:ai')`
  formájában jelenik meg, amit a felhasználó elfogadhat vagy elutasíthat.
- A gomb **default kikapcsolt**; bekapcsolás külön döntés (helyi modell fut-e,
  SearXNG elérhető-e). A kapcsoló állapota nem kerülhet a publikus repóba.
- A keresési eredményeket forrással együtt kell megjeleníteni, és a robots/
  policy-szabályok itt is érvényesek (AGENTS.md §7).

---

## 9. Szeletek (javasolt sorrend — mindegyik külön terv + kapu)

| # | Szelet | Fő elemek | Kapu |
|---|---|---|---|
| S1 | Adatmodell + API alap | `ops.issue`, `ops.match_note`, migráció, `GET issues`, notes CRUD, history | Rust teszt + API kontrakt + integritás |
| S2 | Üzemeltetés — állapot és idővonal | kattintható kártyák, issues lista, audit feed, szűrés | UI teszt + valódi AX ellenőrzés |
| S3 | Üzemeltetés — összefoglaló és akciók | `/v1/ops/summary`, magyar összefoglaló, akció-gombok | UI teszt + daily-check |
| S4 | Teljes meccs-szerkesztés | `PUT edit`, előnézet, domain validáció, provenance/audit | `cargo test` + manual-entry kontrakt + UI |
| S5 | Logófeltöltés UI | multipart API, TeamSheet/CompetitionSheet/szerkesztő, asset biztonság | feltöltési teszt + képméret/policy teszt |
| S6 | Versenyek és Csapatok bővítése | hiány-arányok, listák, szűrők, sheet-jegyzetek | UI teszt + AX |
| S7 | Jegyzetek a meccsrészletezőben | notes panel, típusok, szerkesztés, ügynök-auth | UI + API teszt |
| S8 | AI-támogatás (opcionális) | gemma 4 gomb, SearXNG, javaslat-only, kapcsoló | külön döntés + teszt |

Minden szelet végén: `cargo test --workspace`, `cargo clippy --all-targets -D warnings`,
`cargo fmt --check`, Core + app unit tesztek, `run-ui-tests.sh`, `daily-check`,
`footballctl integrity`, `git diff --check` — és csak zöld kapu után commit/push.

---

## 10. Döntések, amiket előbb kell tisztázni

1. **Írási jogosultság:** a teljes meccs-szerkesztés és a logófeltöltés is
   ops tokenes maradjon (javaslat: igen), vagy a felhasználói read-token is
   írhat bizonyos mezőket (javaslat: nem)?
2. **`manual-result` és `full edit`:** egy végpont vagy kettő (javaslat: új
   egységes végpont, régi kompatibilitás megőrzése).
3. **`ops.issue` generálása:** a napi `daily-check`/`integrity` írja az adatbázisba
   (javaslat: igen, ahogy most a riportot írja), vagy a UI számolja ki élőben
   (javaslat: nem — az idővonalhoz kell a perzisztencia)?
4. **Jegyzet-láthatóság:** a jegyzetek csak ops felületen, vagy a read-tokenes
   kliens is olvashatja őket (javaslat: olvashatja, de a szerző/ügyfél ne
   kerüljön a nyilvános napi feedbe)?
5. **Logó formátum:** SVG engedélyezett-e az első szeletben (javaslat: nem,
   csak WebP/PNG; SVG külön biztonsági review után).
6. **AI-gomb:** alapból ki legyen kapcsolva (javaslat: igen) — a lokális
   modell és a SearXNG elérhetőségét külön üzemeltetési döntésként kell rögzíteni.

---

## 11. Kockázatok

- **Túl sok egyszerre:** a nyolc szelet nem egy nap munka; a felhasználó
  napi egy-szelet szabálya (AGENTS.md §2) itt is érvényes, hacsak nem ad
  kifejezett felhatalmazást a sorozatos haladásra.
- **Identity-merge mellékhatások:** a csapatok/versenyek bővítése közben a
  téves entitás-összevonás (l. `remaining-gaps.md`) újabb duplikátumokat
  okozhat — ezért marad minden merge kézi és auditált.
- **Audit-adat mérete:** az `ops.issue` és a jegyzetek növekedhetnek; a régi,
  megoldott jeleket retention-szabállyal kell kezelni (l. `validation-retention.md`).
- **UI-komplexitás:** a három oldal és a meccs-szerkesztő közös komponenseket
  érdemes használni (`IssueBadge`, `TimelineRow`, `NoteEditor`, `LogoPicker`),
  különben párhuzamosan eltolódnak a szövegek és az akadálymentességi címkék.

---

## 12. Visszaállítás

A dokumentum önmagában nem változtat adatot vagy kódot. A későbbi szeletek
migrációihoz és API-változásaihoz minden szelet saját checkpointban rögzíti
az érintett fájlokat, a futtatott kapukat és a visszaállítási lépéseket
(`git revert <commit>` + adatrevízió audit eseménnyel).
