# Üzemeltetési triage console — tervezési specifikáció

**Dátum:** 2026-11-27

**Státusz:** JÓVÁHAGYOTT TERVEZÉS — a felhasználó a „triage-first Operations Console” irányt elfogadta

**Branch:** `feat/ops-triage-console`
**Előzmény:** `2026-11-25-app-oldalak-bovites-design.md`, `2026-11-26-app-oldalak-bovites.md`, a 2026-11-27-i kliens- és backend-audit

## 1. Cél

Az Üzemeltetés oldal elsődleges kérdése nem „mi minden történt?”, hanem:

> **Mi a mostani problémám, miért fontos, mit kell tenni, és mennyire sürgős?**

Az oldal ember és ügynök számára is használható lesz. A belépő első 10 másodpercben
látja a rendszer verdiktjét, a 3–5 legfontosabb teendőt és a források egészségét.
A részletes bizonyíték, idővonal és műveletek egy kontextusban, az érintett entitás
mellett maradnak.

A tervezés három alapelve:

1. **Globális és napi scope soha nem keveredik.**
2. **Gyors állapotkép és drága teljes ellenőrzés külön legyen.**
3. **Csak valódi, engedélyezett és auditált művelet jelenik meg akcióként.**

## 2. Határ

### Ebben a munkában

- Az Üzemeltetés ötszakaszos információs építménye.
- Explicit `Globális / Mai nap` scope.
- Gyors status read model és külön integritás-pillanatkép.
- Cursor-alapú issue-, audit- és work-item listák.
- Emberi entitásnevek, csoportosítás, keresés, szűrés és rendezés.
- Egységes, típusozott munkafeladat-modell.
- Részletes inspector bizonyítékkal, jegyzettel és idővonnalal.
- Állapot-, hiba-, stale és részleges betöltési modellek.
- Determinisztikus billentyűzet- és VoiceOver-támogatás.
- A megbízható működés előfeltételei: raw capture azonosítás, worker fencing,
  atomi kvótafoglalás, tranzakcionális audit és scope-javítás.

### Nem ebben a munkában

- Teljes observability platform vagy új event store.
- AI-támogatás vagy automatikus ügynök-írás; az AI alapból kikapcsolt marad.
- Nyers `validation_run` / `validation_finding` megjelenítése.
- Nyers forráspayload, secret vagy runtime token megjelenítése.
- Valódi pénzes fogadás, fogadóiroda-integráció vagy automatikus kifizetés.
- Versenyek és Csapatok oldalak teljes áttervezése.
- Olyan job-, review- vagy forrásművelet, amelyhez még nincs jóváhagyott backend
  szerződés és jogosultsági modell.

## 3. Auditált alapállapot

A mérések 2026-11-27-i auditpillanatban készültek.

| Terület | Mert állapot |
|---|---|
| `GET /v1/ops/status` | 4,54–4,62 mp három egyedi hívásban |
| `GET /v1/issues?scope=all&open_only=true` | 1510 sor, 527 826 bájt, 110,6–115,1 ms |
| `GET /v1/ops/summary?date=2026-11-27` | `open_total=0`, miközben globálisan 1510 nyitott jel van |
| Nyitott jelek | 889 `missing_logo`, 602 `missing_odds`, 19 `open_past` |
| Pending review | 214 |
| Jobok | 2 pending, 0 failed, 0 leased a mérés pillanatában |
| Audit | 83 035 sor |
| Validation előzmények | 1 483 478 run + 2 213 569 finding, kb. 1,56 GB; nincs UI/API-fogyasztó |
| Integritás | S215 óta a valós duplikátumok és eredményhiányok keményebb jelzést kapnak |

A fő teljesítmény- és integritásproblémák:

- A status minden policy-re külön számol és lekérdez, majd teljes integritás-riportot futtat:
  `rust/crates/api/src/routes/ops.rs:81`.
- Hibás forráslekérdezés csendesen `0 attempts` és `closed breaker` lesz:
  `rust/crates/api/src/routes/ops.rs:82`.
- A két legdrágább integritásmetrika egyenként 1,97–2,04 mpig tartott, és nem is szerepel
  a status válaszában.
- Az issue repository `fetch_all`-t használ, nincs cursor vagy teljes találatszám:
  `rust/crates/storage/src/repositories/issue.rs:56`.
- A summary csak a kiválasztott nap nyitott issue-it olvassa be és Rustban aggregálja:
  `rust/crates/api/src/routes/summary.rs:84`.
- A kliens a négy erőforrást 30 másodpercenként sorban tölti:
  `apps/FootballApp/OpsModel.swift:229`.

A bizalmi blockerök:

1. `raw.capture` egyedi `(source_id, request_key)` kulcsa miatt új fetch új artifactet hozhat,
   de az observationt régi capture-hez köti: `raw.capture` semma és
   `storage/repositories/worker.rs:816`, `worker/src/tick.rs:532`.
2. Nincs worker-singleton, lease fencing vagy atomi kvótafoglalás; a daemon és a kézi
   `footballctl worker tick` egyszerre futhat.
3. `POST /v1/issue/{id}/resolve` a változást és auditot külön tranzakcióban kezeli:
   `rust/crates/api/src/routes/issue_action.rs:57`.
4. A meccstörténet Read scope alatt ad vissza teljes belső `detail` / `before` / `after`
   adatot: `rust/crates/api/src/routes/history.rs:61`.
5. A jelenlegi UI státuszkártyái más adatmodellekre szűrnek, és a nem nap nézetben
   a determinisztikus Tab-sorrend nem fedi az Ops vezérlőket.

## 4. Sikerkritériumok

### Felhasználói

- Belépés után 10 másodpercen belül egyértelmű: a rendszer `OK`, `FAIL`, `DEGRADED`
  vagy `STALE` állapotban van-e.
- 3–5 priorizált teendő látható minden további görgetés nélkül.
- Minden szám mellett szerepel a scope és az `as_of` idő.
- Egy issue sorából egy kattintással elérhető az érintett meccs vagy entitás.
- A 1510 jel nem 1510 külön döntés; a fő csoportok és a következő akció egyértelmű.
- A hibás, üres, betöltő és elavult állapotok nem hasonlítanak egymásra.

### Teljesítmény

- `/v1/ops/status` p95 **< 150 ms**.
- Első interaktív Ops-tartalom **≤ 1 s** az app indulása után.
- Issue lista p95 **< 250 ms**, alap 50, maximum 200 elem.
- Alap lista-válasz **≤ 128 KB**; nagy részletek külön endpointon.
- A status SQL-lekérdezések száma nem nő a források számával.
- A request-pathban nincs teljes-történeti integritás-scan.
- Erőforrásonként legfeljebb egy in-flight kérés; backgroundban Ops-hálózat nem fut.

### Megbízhatóság

- Hibás forrás nem jelenik meg egészséges alapértelmezett értékkel.
- Minden írás és audit ugyanabban a tranzakcióban commitol.
- Minden mutáció visszakövethető actorral, indokkal és idempotencia-védelemmel.
- Read scope nem lát belső audit/provenance részleteket.
- A worker nem képes egyszerre több ticket futtatni, és stale worker nem zárhat le
  újabb lease-t.

## 5. Információs építmény

### 5.1 Globális fejléc

Minden Ops nézet tetején:

- `Globális / Mai nap` választó;
- rendszerverdikt: `OK`, `FAIL`, `DEGRADED`, `STALE`;
- `Frissítve: …` és `Adatkora: …`;
- kézi frissítés és utolsó hiba rövid üzenete.

A scope választó minden alárendelt listára és összesítőre vonatkozik. Napi scope esetén
a globális job/review/breaker számlálók nem jelennek meg napi issue-szám mellett.

### 5.2 Állapot most

Landing oldal tartalma:

1. **Verdikt és magyarázat.** Egy mondat: mi a legfontosabb probléma és miért.
2. **Prioritáslista.** 3–5 művelethez kötött teendő.
3. **Egészségsáv.** Pending/leased job, pending review, karantén, nyitott breaker,
   withheld gate és nyitott issue — mind scope-hoz igazítva.
4. **Forrásbetegségek.** Csak azok a források, ahol van figyelmeztetés vagy hiba.
5. **Legutóbbi változások.** Rövid, 5–10 soros audit előnézet; a teljes feed külön nézet.

### 5.3 Teendők

Egységes munkalista, amely képes különböző típusú elemeket mutatni:

- `issue`: tartós hiányjel;
- `job`: függő, sikertelen vagy elakadt job;
- `identity_review`: emberi döntést igénylő azonosítás;
- `source_incident`: breaker, kvóta, policy vagy coverage probléma;
- `publish_gate`: visszatartott vagy ellenőrizetlen publikáció.

Minden elem tartalmazza:

- típust és súlyosságot;
- emberi címet és tárgynevet;
- első/utolsó észlelést vagy létrehozást;
- forrást és aktuális állapotot;
- javasolt következő lépést;
- művelet elérhetőségét és deep linket.

A lista csoportosítható típus, kód, forrás, verseny, súlyosság és kor szerint.

### 5.4 Források

Forrásonként:

- policy mód és szöveges engedélyezési állapot;
- napi próbálkozások / budget / maradék kvóta;
- breaker állapot, failure count, `opened_at`, `next_probe_at`, utolsó siker;
- utolsó hiba és hiba kora;
- coverage: done, no_data, failed, blocked, quarantined;
- linkelt és review-ra váró események.

A szín nem egyedül hordoz állapotot.

### 5.5 Adatminőség

- Integritás-verdikt és számolt mutatók.
- Feloldatlan duplikátumcsoportok.
- Lejárt, de még scheduled eredmény nélküli meccsek.
- Lezárt, eredmény nélküli meccsek.
- Árva provenance és merged entitás-referenciák.
- Publish gate állapotok.
- Issue-csoportok és trend.

A teljes validációs előzmények nem kerülnek ide. Csak a levezetett issue-k,
összesítések és emberileg értelmezhető eredmények jelennek meg.

### 5.6 Audit és jegyzetek

- Kereshető, cursor-alapú globális audit.
- Subject-, actor-, action- és időszűrés.
- Read scope-hoz engedélyezett audit mezők.
- Teljes belső bizonyíték csak Ops scope-hoz.
- Meccshez kötött jegyzetek és `before → after` idővonal az inspectorban.
- A jegyzet nem kerül a napi feedbe.

## 6. Adatmodellek

### 6.1 `OpsWorkItem`

```text
id
kind: issue | job | identity_review | source_incident | publish_gate
severity: info | warning | hard | critical | unknown
title_hu
subject_kind
subject_id
subject_label
source
first_seen_at?
last_seen_at?
age_seconds
recommended_action_hu?
action_availability: available | unavailable | forbidden | unknown
deep_link
```

A prioritás determinisztikus: először `critical`, majd `hard`, `warning`, `info`;
ugyanabban a súlyosságban a blokkoló állapot, aztán a nagyobb kor, végül az entitás
neve és `id`. A javasolt akció csak akkor emel, ha valóban elérhető; a nem elérhető
művelet nem ad hamis prioritást.

A `critical` csak olyan létező backend-állapotra használható, amely valóban blokkolja
a megbízható üzemeltetést. Az ismeretlen értékek megmaradnak, nem találgatunk magyarázatot.

### 6.2 `OpsStatusSnapshot`

- `as_of`, `stale`, `degraded`;
- globális vagy napi scope;
- job/review/quarantine/breaker/gate/issue számlálók;
- források rövid állapota;
- integritás-összefoglaló és utolsó sikeres teljes ellenőrzés ideje;
- per-erőforrás hiba és `partial_failure` lista.

A status nem tartalmaz teljes integrity reportot és nem hajt végre teljes scan-t.

### 6.3 `OpsIssuePage`

- `items`, `total`, `next_cursor`, `has_more`, `as_of`;
- `scope`, `q`, `code`, `severity`, `source`, `subject_kind`, `open_only`, `sort`;
- `facets`: code, severity, source és competition szerinti darabszámok.

Minden issue sor emberi tárgynevet kap. Meccs esetén legalább:

- hazai – vendég;
- verseny;
- kezdés;
- státusz;
- hiányzó adat típusa.

### 6.4 `OpsIntegritySnapshot`

- `verdict: OK | FAIL | DEGRADED | STALE`;
- `computed_at`, `scope`, `stale_after`;
- duplikátum-, provenance-, review-, gate- és eredményhiány-mutatók;
- `failed_checks` és `source_errors`.

A snapshot külön, háttérben vagy explicit kérésre frissül. A landing ezt az utolsó
példányt jeleníti meg, és jelzi, ha elavult.

## 7. API-szerződés

### 7.1 Közös lista-szerződés

Minden új vagy bővített lista:

```text
GET ...?limit=50&cursor=...&q=...&sort=...
```

- `limit`: alap 50, maximum 200.
- `cursor`: opakálható, determinisztikus keyset cursor.
- Rendezés: explicit `sort` és stabil `id` tie-break.
- Válasz: `items`, `next_cursor`, `has_more`, `total`, `as_of`.
- A `total` a szűrés teljes találatszáma, nem a visszaadott oldal mérete.
- A kurzor és az `as_of` egy pillanatképhoz tartozik; frissítés után új kurzor kell.

### 7.2 Olvasó végpontok

| Végpont | Scope | Cél |
|---|---|---|
| `GET /v1/ops/status` | Ops | gyors, cache-elt vagy read-model állapotkép |
| `GET /v1/ops/integrity` | Ops | utolsó integritás-pillanatkép és explicit újraszámítás |
| `GET /v1/issues` | Read | cursor-lapozott, gazdagított issue lista |
| `GET /v1/issue/{id}` | Read | egy issue részletes, allowlistette DTO |
| `GET /v1/ops/work-items` | Ops | egységes, priorizált munkafeladat-lista |
| `GET /v1/ops/audit` | Ops | cursor-alapú globális audit |
| `GET /v1/match/{id}/history` | Read | allowlistette, nem bizalmas változástörténet |
| `GET /v1/ops/match/{id}/history` | Ops | teljes belső bizonyíték és before/after |
| `GET /v1/ops/sources` | Ops | forráspolitika, breaker, kvóta és coverage részletek |

A meglévő `/v1/ops/summary` átalakulhat, de addig nem törölhető, amíg a régi kliens
használja. Az új summary mindkét scope-ra képes, és sosem kever globális számot
napi issue-listával.

### 7.3 Író végpontok

Minden írás:

- Ops scope;
- `Idempotency-Key` és payload-hash ellenőrzés;
- actor a hitelesített tokenből;
- kötelező indok olyan műveletnél, amely adatot vagy üzemeltetési állapotot módosít;
- `expected_version` vagy `If-Match`, ahol van verzió;
- válaszban `audit_id`, `reused`, érintett rekord és szükség esetén reverify állapot.

Első műveleti hullám:

- `POST /v1/issue/{id}/resolve`.

Későbbi, külön tervezett műveletek:

- job retry/cancel;
- identity review approve/reject/create;
- quarantine feloldás;
- source policy változtatás;
- újraverifikáció.

Egy művelet addig nem jelenik meg aktív gombként, amíg a végpont, jogosultság,
audit és visszaállítás nincs implementálva.

## 8. Backend-invariantok

### 8.1 Raw capture és provenance

- Egy fetch minden esetben új `raw.capture` sort hoz létre.
- `(source_id, request_key)` nem lehet egyedi korlátozás, mert a `request_key` stabil
  kérésidentitás, nem capture-identitás.
- `persist_raw` a ténylegesen beszúrt capture azonosítót adja vissza.
- Minden observation a saját capture-jére mutat.
- Új artifact nem maradhat árva egy ütköző capture miatt.
- A migráció nem töröl régi capture-t; szükség esetén archiválási/forward-fix útvonal
  dokumentálandó.

### 8.2 Worker és kvóta

- Egy adatbázis-szintű singleton lock megakadályozza a daemon és kézi tick párhuzamos
  futását.
- Job claim egyedi lease/fencing tokent kap.
- Completion, requeue és coverage-frissítés csak a saját lease tokennel történhet.
- A worker lease-t heartbeatel tart életben; lejárt lease nem zárhat le újabb munkát.
- Kvótaszámítás, min-delay ellenőrzés és ledger-placeholder beszúrás egy per-source
  tranzakcióban történik.
- Breaker állapotátmenet CAS-sel vagy zárolással történik.
- A manual tick nem kerülheti meg a singleton lockot.

### 8.3 Audit és idempotencia

- Resolve, note create/edit, asset upload és minden új ops-művelet tényleges változása
  és audit-bejegyzése ugyanabban a tranzakcióban commitol.
- Audit hiba esetén a tényleges változás sem commitolódhat.
- Ugyanaz az idempotencia-kulcs más payload-hash-szel nem ad `reused=true` eredményt.
- A klienstől kapott actor nem hitelesített actor.
- A művelet válasza és audit azonos tranzakcióból származó azonosítót kap.

### 8.4 Fail-closed status

- DB-hiba nem alakul csendesen `0` vagy `closed` értékké.
- Forrásonként látható hiba és `unknown` állapot.
- Ha egy erőforrás nem tölthető be, az egész oldal helyett a többi jó adat marad meg,
  az állapot pedig `DEGRADED` vagy `PARTIAL`.
- A `FAIL` verdikt nem zárható le csak a jobok alapján; a pending review, nyitott breaker,
  withheld gate és eredményhiány is része az értékelésnek.

### 8.5 Read model és lekérdezési költség

- A status nem hajt végre policy-számú lekérdezést.
- A status nem futtat teljes integritás-riportot.
- A status legfeljebb 5 másodperces processzintű cache-t használ; írás vagy explicit
  frissítés invalidálja. A válasz mindig tartalmazza a cache `as_of` idejét.
- Az issue lista SQL-oldalon lapoz, szűr és rendez.
- Az issue összesítés SQL aggregate, nem Rustban betöltött teljes lista.
- Minden új Ops SQL-hoz production-méretű `EXPLAIN (ANALYZE, BUFFERS)` és terheléses
  ellenőrzés tartozik.

## 9. Kliensarchitektúra

### 9.1 Nézetbontás

A jelenlegi monolitikus `OperationsView` felosztása:

- `OperationsView`: shell, scope és navigáció;
- `OperationsStatusView`: verdikt, számlálók, prioritáslista;
- `WorkQueueView`: keresés, szűrők, csoportok, lapozás;
- `SourcesView`: források, breaker, kvóta, coverage;
- `QualityView`: integritás és issue-csoportok;
- `AuditNotesView`: audit és jegyzetek;
- `OpsInspectorView`: részletes bizonyíték és műveletek;
- közös Ops sor-, badge-, filter- és állapotkomponensek.

Egy fájl egyetlen világos felelősséget kapjon. A Core magyarázó szövegei és
accessibility-címkéi továbbra is közös, tesztelt forrásból jöjjenek.

### 9.2 Állapotkezelés

- `@MainActor`, `@Observable` modell.
- Erőforrásonként külön állapot: idle, loading, loaded, empty, error, stale.
- Generációszám és `Task` cancellation a régi eredmények elvetésére.
- In-flight deduplikáció erőforrásonként.
- `scenePhase`: aktív nézetben és foregroundban auto-refresh; háttérben leáll.
- Utolsó jó adat megmarad hiba esetén.
- Írás külön mutation-state, nem az egész lista error-state-je.
- Szűrőváltás után a korábbi kurzor és oldal eldobható.

### 9.3 Adatfolyam

1. Landing: cache-elt status + legutóbbi integritás-pillanatkép.
2. Priorizás: work-item vagy issue-facet aggregate.
3. Teendők: első cursor-oldal, majd load-more.
4. Részletek: csak kiválasztott elemhez részletes endpoint.
5. Írás: mutation → célzott invalidation → új helyi eredmény vagy háttérfrissítés.
6. Auto-refresh: csak látható, aktív erőforrások; nem minden 30 másodpercben minden.

## 10. Állapotok és interakciók

### 10.1 Betöltés

- Landing skeleton a verdikt és a prioritáslista helyén.
- Számláló nem `0`, hanem `—`, amíg nem ismert.
- Korábban betöltött adat mellett diszkrét frissítésjelző.

### 10.2 Üres

- Valódi üres: „Nincs nyitott teendő ebben a scope-ban.”
- Szűrt üres: „A szűrők mellett nincs találat.” + szűrők törlése.
- Nem tűnhet üres állapotnak egy sikertelen lekérdezés.

### 10.3 Hiba és elavultság

- Per-panel hiba, nem teljes oldal hiba.
- Utolsó jó adat megjelenítése `Elavult` jelzéssel.
- `Újrapróbálás` az adott erőforrásra.
- Rendszerverdikt `DEGRADED`, ha valamelyik alapvető adatforrás nem elérhető.

### 10.4 Szűrés és rendezés

- Keresés 250–350 ms debounce után.
- Szűrők: scope, típus, kód, súlyosság, forrás, verseny, subject, nyitott/lezárt.
- Rendezés: prioritás, utolsó észlelés, legrégebbi, súlyosság, tárgy.
- Aktív szűrők láthatók és egyenként eltávolíthatók.

### 10.5 Műveletek

- Resolve előtt megerősítés és indok.
- Művelet közben busy állapot; dupla kattintás tiltva.
- Sikeres válasz után célzott lista- és summary-frissítés.
- Hiba esetén a korábbi állapot megmarad, magyar hibaüzenet jelenik meg.
- Tömeges művelet csak olyan csoporton, ahol minden elem ugyanazt a biztonságos
  akciót és jogosultságot támogatja.

## 11. Vizuális és akadálymentességi követelmények

- OLED-fekete `#000000` alap és a meglévő AppTheme használata.
- Kompakt, de legalább 44 pt érintési/vezérlőmagasság, ahol a macOS vezérlő teszi.
- Súlyosság: szín + ikon + magyar szöveg.
- Forrásengedélyezés: szín + szöveg + AX-címke.
- Keskeny ablakban reszponzív elrendezés; inspector sheet vagy teljes képernyős nézet.
- Nincs fix szélességű, túlcsorduló HStack.
- Minden vezérlőhöz stabil accessibility-identifier.
- Determinisztikus Tab-sorrend minden Ops nézetben.
- Látható fókuszgyűrű, Escape bezárja az inspector-t.
- VoiceOver teljes magyar mondat: tárgy, állapot, kor, forrás, művelet.
- Reduced motion esetén állapotváltás animáció nélkül.
- A tényleges VoiceOver-hang továbbra is külön, emberi ellenőrzés; a projekt jelenlegi
  döntése szerint ez skip-elt.

## 12. Biztonság

- Csak localhost API; a publikus repóban nincs token, secret vagy nyers payload.
- Read scope: allowlistette olvasható DTO.
- Ops scope: teljes audit/provenance detail és minden írás.
- Actor hitelesített tokenből.
- Minden írás auditált és tranzakcionális.
- Nincs automatikus fuzzy merge.
- AI alapból kikapcsolt; nincs külső szolgáltatás hívás ebben a munkában.
- A logófeltöltés és teljes meccsszerkesztés meglévő, külön döntései változatlanok.

## 13. Tesztelés és elfogadás

### 13.1 Backend

- Raw capture: ugyanaz a `request_key` két külön fetchje két capture-t és két helyes
  observation-hivatkozást készít.
- Worker: daemon és manual tick párhuzamos indulásakor csak egy tick fut.
- Lease: stale worker nem frissíthet újabb lease-t.
- Kvóta: párhuzamos tick nem lépheti túl a napi budgetet.
- Audit: kényszerű audit-hiba esetén a tényleges módosítás sem commitol.
- Idempotencia: más payload ugyanazzal a kulccsal elutasít.
- Scope: Read token nem kap belső `detail` / `before` / `after` mezőt.
- Pagination: stabil oldalak, teljes `total`, nincs ismétlődés vagy kihagyás kurzoros
  frissítés nélkül.
- Status: query-szám nem nő a policy-számmal, és nem fut teljes integritás-scan.
- Teljesítmény: production-méretű adaton a cél p95 értékek mérése.

### 13.2 Swift/Core

- DTO-k és contract-fixturek minden oldalalakhoz.
- Ismeretlen enum-érték nem dob.
- Race, cancellation és in-flight deduplikáció tesztek.
- Partial error és last-good-data viselkedés.
- `scenePhase` és auto-refresh leállás.
- Szűrő-, cursor- és mutation-state tesztek.
- Accessibility-label és Tab-sorrend unit tesztek.

### 13.3 macOS UI

Célzott UI-tesztek:

- Globális/napi scope váltás nem keveri a számokat.
- Prioritáslista megnyitja a megfelelő elemet.
- Keresés, szűrés, rendezés és load-more.
- Resolve megerősítés, busy, hiba és sikeres frissítés.
- Offline/partial/stale állapot.
- Üres és nagyon hosszú lista.
- Keskeny ablak és inspector sheet.
- Tab/Shift+Tab/Escape és látható fókusz.
- Magyar szövegek és AX-címkék visszaolvasása.

A teljes `run-ui-tests.sh` nem rutin kapu; a szelethez szükséges 1–2 célzott eset
futtatható. Valódi macOS ellenőrzés kötelező minden Ops UI-szelet végén.

## 14. Végrehajtási szeletek

Minden szelet külön implementation plan-t kap, és csak a saját határán belül zöld kapu
után commitolható.

| Szelet | Tartalom | Fő kapu |
|---|---|---|
| O0 | Raw capture/provenance invariant | storage + worker integrációs teszt, nincs árva artifact/capture |
| O1 | Worker singleton, lease fencing, atomi kvóta | párhuzamos tick/lease/kvóta tesztek |
| O2 | Tranzakcionális audit, actor, idempotencia, Read/Ops history szétválasztás | auth + transaction + contract tesztek |
| O3 | Gyors status read model és integrity snapshot | query count + p95 + fail-closed tesztek |
| O4 | Issue cursor, teljes count, facets, emberi tárgynevek | pagination + contract + SQL plan tesztek |
| O5 | Work-item read model és prioritás | determinisztikus rendezés + scope tesztek |
| O6 | Ops shell, scope, Állapot most, állapotmodellek | Core + app unit + célzott macOS UI |
| O7 | Teendők, inspector, resolve megerősítés | UI + mutation + AX tesztek |
| O8 | Források, Adatminőség, Audit és jegyzetek | UI + partial/stale + AX tesztek |
| O9 | Teljesítmény-, reszponzivitás- és accessibility-zárás | p95, keskeny ablak, Tab, valódi macOS |

A sorrend biztonsági: előbb a tényleges adatbiztonsági invariantok, aztán az olvasó
modellek, végül a teljes console.

## 15. Visszaállítás

- Minden kódszelet `git revert`-el visszavonható.
- A raw capture migrációja nem töröl adatot; visszaállásnál a régi unique constraint
  csak akkor állítható vissza, ha a közben keletkezett ismétlődő capture-k előbb
  archiválhatók vagy egyértelművé tesznek.
- A worker lock/fencing sémája a kód visszaállítása után használatlanul maradhat;
  adatot nem töröl.
- Az ops írások audit eseménnyel és explicit revízióval visszavonhatók; nincs
  csendes adatjavítás migrációval.
- A UI visszaállítása nem érinti az API-t vagy az adatbázist.
- Minden szelet checkpointja rögzíti az érintett fájlokat, futtatott kapukat és
  konkrét visszaállítási lépéseket.

## 16. Kockázatok

- **Túl sok egyszerre:** a szeletek külön tervek és kapuk mentén haladnak.
- **Cursor-konzisztencia:** kurzoros olvasás egy `as_of` pillanatképra van kötve;
  frissítés után új kurzor kell.
- **Művelet-hiány:** ahol nincs backend akció, ott csak diagnosztika és deep link
  jelenik meg, nem hamis gomb.
- **Drága integritás:** a landing soha nem vár teljes újraszámításra; utolsó snapshot
  és stale jelzés jelenik meg.
- **Adatminőség:** az oldal nem javít adatot megjelenítéssel; tényleges javítás
  külön auditált művelet.
- **UI-komplexitás:** a monolit felosztása és közös komponensek nélkül a regresszió
  kockázata magas.

## 17. Rögzített döntések

1. A `triage-first Operations Console` irány választott.
2. Globális és napi scope explicit elkülönítés.
3. A nyers validation adatok nem kerülnek a fő oldalra.
4. AI alapból kikapcsolt.
5. Read scope nem lát belső audit/provenance részleteket.
6. Csak valódi, auditált és jogosultság-védett művelet jelenik meg aktív akcióként.
7. A teljes UI-suite nem rutin kapu; célzott UI-teszt és valódi macOS ellenőrzés szükséges.
8. A VoiceOver tényleges hangja továbbra is emberi, külön ellenőrzés.
