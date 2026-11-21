# Adatjavítás és rekonciliáció — részletes szabályok

## Ország-/verseny-mapping javítások

- **A részstring-alapú névtábla-lookup a leghosszabb illeszkedő nevet válassza, ne az első tábla-találatot**:
  az országnevek egymást tartalmazzák (jordania⊃dania, szomalia⊃mali, feheroroszorszag⊃oroszorszag,
  dominikai koztarsasag⊃dominika, romania⊃oman). Az első találat csendben másik országra képez.
  A dekorált címek („Anglia - Premier League") részstring-egyezése megmarad — a leghosszabb találat
  ezt is helyesen adja.
- **Kontinens-zászló detektálás egzakt (foldolt) egyezés legyen**: a „Dél-Afrika"/„Közép-Afrika"
  ország-zászló cím tartalmazza az „afrika" szót, de nem kontinens.
- **A verseny find-or-create ország-szkópolt**: mapping-javítás után az újrafuttatás MÁSodik
  verseny-entitást hozna létre a helyes ország alatt. Előbb tudatos revízió (UPDATE country_id +
  `ops.audit_event` old/new/reason részlettel) — nem migráció —, hogy az újrafuttatás a meglévő
  entitásra landoljon.
- Kár-felmérés javítás előtt: rossz országú verseny-entitások, csapat-entitások, pending review-k
  (lekérdezések a references-ben). A rossz országú megfigyelések történelmi bizonyítékként
  megmaradnak; az újrafuttatás javított megfigyelést ad hozzá, nem ír felül.
- A fail-closed linker a csapat-entitásokat megvédi (review-ban ragadnak), de a **verseny-entitás
  és a megfigyelés azonnal létrejön** rossz országgal — a mapping-hiba kára ott csapódik le először.

## Kereszt-forrás rekonciliáció

- **Fixture-szintű dedupe létezik**: `footballctl dedupe fixtures [--from --to --competition
  --apply --limit]` — kulcs: verseny + szezon + csapat-pár (tükrözés-toleráns) + UTC-nap,
  NEM eszakt kickoff. Auto-merge csak azonos orientáció + egyező (vagy egyik oldalon hiányzó)
  eredménynél; a túlélő a footballdata-provenance-ú sor. A kockázatos párokat (eltérő eredmény,
  tükrözött, dátumon átnyúló) tételesen listázza, nem írja őket.
- **A v1 import és a footballdata backfill eltérő kickoffbal írja ugyanazt a fixture-t**
  (Eliteserien: szisztematikus +1/+3h TZ-eltolás). Az eszakt-kickoff `dedupe matches` ezeket
  nem látja — a fixtures alparancs kell. Csapat-merge után MINDIG fuss le egy `dedupe fixtures`
  kör (a variáns csapatok meccsei akkor válnak azonos-csapat párokká).
- **Csapatnév-variánsok merge-e v1-bizonyítékkal**: `identity merge-team --via-alias "<bizonyíték>"`
  (kötelező szöveg; az áldozat neve reviewed aliasként rögzítődik, method `import`). Az audit:
  v1 `team_aliases` (read-only `psql -d football_app`, OS-user) × v2 csapatnevek foldolt egyezése;
  a kétértelmű variáns (pl. Gimnastic → U19 és senior is) KIMARAD. A country-szűrőben a
  **GB-alrégió kódokat (GB-WLS/GB-SCT/…) normalizálni kell GB-re** — a v1 import Welsh klubokat
  GB-WLS-szel hozott, a footballdata GB-vel; enélkül az audit csendben kihagyja őket (Swansea-tanulság).
- **Verseny-entitás fragmentáció rejti a duplikátumokat**: a fixture-kulcs verseny-szkópolt, ezért
  a v1 verseny-nevek („Scottish League One”) és a footballdata kanonikus nevek („League One”)
  külön entitásai alatt ugyanaz a fixture nem csoportosul. Ez a következő crosswalk-szelet
  (S21 szótár kiterjesztése a v1 nevekre + verseny-merge).
- **A merge után `MATERIAL_SOURCE_CONFLICT` withheld gate-ek keletkezhetnek** (a két forrás
  eltérő kickoffja a provenance-ban megmarad, a verification réteg nem ismeri a túlélő-prioritást).
  A reverify hozza felszínre; feloldás konfliktus-döntéssel (winner observation), külön lépés.
  A withheld őszinte fail-closed állapot, nem hiba — dokumentáld, ne kend el.
- **Verseny-merge (S81)**: `footballctl identity merge-competition --from <beolvadó entitás> --into <túlélő entitás> [--note] [--apply] [--force]`; alapból dry-run. Guard: competition típus, nincs merged_into, fold-olt entitásnév VAGY versenynév egyezik, ország egyezik vagy NULL, túlélő meccsszáma nem kisebb. A merge után a túlélőre alias íródik mindkét névből, így a következő ingest nem hoz létre új versenyt. A find-or-create S81 óta az entitás `canonical_name`-jét is fold-olva egyezteti (alias-hiányos régi sorok). Részletek: `docs/operations/competition-merge.md`. Ha a merge után mégis új verseny keletkezne, előbb a canonical/alias lookupot ellenőrizd, ne a forrást okold.
- A merge-ek visszajátszhatók: `ops.dedupe_archive` (meccs) / `ops.entity_merge_archive` (csapat)
  teljes payloaddal; a note tartalmazza a bizonyítékot és a kickoff-döntést.
- **A kockázatos párok tömeges ellenőrzésekor ne a túlélő forrást hidd alapból.** A `dedupe fixtures` dry-run `KOCKÁZAT [competition|season|home|away|date] … eredmény Some(a)-Some(b) / Some(c)-Some(d)` sorai parse-olhatók. Előbb 2-3 pár webes mintájával állapítsd meg, melyik forrás szisztematikusan hibás — az irány szezononként változhat (egy Eliteserien-blokkban a v1 volt a helyes, a footballdata sorok el voltak csúszva, egy korábbiban fordítva). Ha a v1 a helyes, a **túlélő** (footballdata) sor eredményét írd át auditálva, majd `dedupe fixtures --apply`: így a pár azonossá válik, biztonságosan merge-elődik, és nem marad nyitott `MATERIAL_SOURCE_CONFLICT`. A revízió után ellenőrizd a `fact.conflict` sorokat az érintett meccsekre.
- **Bulk minta ehhez**: a párokat indexelt sorokká alakítva (comp|season|date|home|away|FD|V1) oszd 4-5 párhuzamos kutatónak; mindegyiktől soronkénti ítéletet kérj (`FD`/`V1`/`UNKNOWN`) és két forrás-URL-t. Az `index → match_id` térkép alapján egy tranzakcióban írd át az eredményeket + `ops.audit_event` (`match_result_revision`), majd futtasd a dedupe-ot.

## Konfliktusfeloldás és review-merge

- A `MATERIAL_SOURCE_CONFLICT` blocker minden reverify-kor újra megnyitja a `fact.conflict` sort — a döntést a szabálymotorba kell betáplálni: `VerificationContext.resolved_fields` (`conflict::load_resolved_fields`), CLI: `footballctl conflicts` / `resolve-conflict --match-id --field --winner --reason`. A győztes megfigyelésnek a provenance-ban kell lennie; indoklás kötelező; auditált. A `match` kulcs az egész anyagi összevetést maszkolja.
- **Ne javíts orientációt pusztán a source-ID formátum eltérése miatt**: a footballdata slug (`leeds`) és a v1 `v1t-…` azonosító ugyanarra a kanonikus `core.team`-re képezhet az `identity.source_entity_ref`-ben, de a `home_team_id`/`away_team_id` konfliktust nyit. Előbb oldd fel mindkét ID-t kanonikus csapatra; csak valódi eltérésnél cseréld a csapatokat.
- **A `DUPLICATE_NATURAL_KEY` gate a merge előtti pillanatkép**: ha a fixture-merge már megtörtént, a `dedupe matches --date` nem talál duplikátumot (nincs két egyező kickoffú sor), a gate viszont még hordozza a blockert. Ne a `ops.publish_gate`-et írd kézzel: futtass `reverify --from <nap> --to <nap>` az érintett napokra, és a blocker eltűnik. A `count_duplicate_matches` pontos kickoff-egyezést használ, ezért a +1h eltolt párokat a fixtures-dedupe kezeli, nem ez.
- Eredményeltérésnél előbb webes elsődleges forrás döntsön, és a kanonikus eredményt auditált revízióval javítsd — a maszkolás nem pótolja a hiányzó eredményt (`MATCH_RESULT_REQUIRED`).
- `dedupe fixtures --review-note "<indok>"` csak dokumentált review-val engedi a kockázatos párokat; az indok az archívumba kerül, az azonossági őrök (`same_fixture_identity`, `same_fixture_identity_any_day`) továbbra is kötnek. Minden párra kell bizonyíték (v1 DB + webminta).
- A dátumon-átnyúló detektornál mindkét meccset (`m1`, `m2`) kösd a csapat-párhoz — különben hamis párokat mutat.
- Verseny-fragmentáció gyanúja esetén előbb mérd az átfedő fixture-párokat: a League One és Scottish League One 0 átfedést mutat (külön versenyek!). Az Eliteserien-duplikátumok oka a v1 `II` suffixű csapatvariáns volt (4 `--via-alias` merge + 1089 fixture merge).
- Kétértelmű aliasnál (Gimnastic) a dátumok/ellenfelek v1 DB-vel való egyeztetése dönt, nem a névhasonlóság.
- Top-up pass: mérd a backfill jobokat és a korai szezonok meccsszámát (PL 1993/94: 462 = 22×42), ne requeue-ld reflexből.

## Withheld meccsek lezárása és korai szezonok pótlása

- A `withheld` gate leggyakoribb oka: a v1 import `finished`/`awarded` státuszt hozott eredmény nélkül. Előbb mindig ellenőrizd, hogy a forrás valóban lejátszott-e meccset: a 2026-09-26-i angol meccsek és a KFUM Oslo–HamKam **elhalasztottak** voltak (Sky Sports/ESPN), nem hiányzó eredmény — ilyenkor a helyes javítás `status='postponed'`, nem eredmény kitalálása.
- **Bontsd fel a withheld készletet, mielőtt bármit javítasz**: `SELECT g.blockers->0->>'rule_key', m.status, count(*) FROM ops.publish_gate g JOIN core.match m ON m.id=g.match_id WHERE g.state='withheld' GROUP BY 1,2`. A `MATCH_RESULT_REQUIRED` + `awarded` sorok túlnyomó része visszalépéses hükmen; a `finished` sorok lehetnek elhalasztott vagy el nem játszott meccsek. 25-30 soronként adj párhuzamos kutatóknak explicit listát (index | verseny | szezon | dátum | hazai | vendég | státusz), és csak két forrás + explicit ítélet után írd be az eredményt, vagy állítsd `postponed`/`annulled` státuszra. A visszalépés tényéből **ne** következtess 3-0-ra forrás nélkül.
- Az `awarded` meccsek eredménye sokszor w/o (pl. Reus Deportiu kizárása: az ellenfél 1-0-t kap; Gaziantep visszalépés: 0-3; field invasion: 0-3; Salernitana–Sampdoria rájátszás: 0-3, összesítés 0-5). Mindig két független forrás + `ops.audit_event` (`match_result_revision`, forrásokkal).
- **Hosszabbításos meccs**: a Soccerway néha csak `home_et/away_et`-t közöl FT nélkül. A `domain::final_score()` ET-first; a `MATCH_RESULT_REQUIRED` csak akkor blokkol, ha sem FT, sem ET pár nincs. A tabella/csapat-összesítőkben `COALESCE(et, ft)` kell (nem fordítva), különben az ET-meccs eredménye elveszik.
- Napi integritás: `footballctl integrity [--from --to] [--json]` — duplikátumok, árva provenance, összevont entitásra mutató aktív ref, review/gate/job/breaker; kemény hiba esetén exit 1. Ezt futtasd minden adatművelet után.
- Zajforrások: a `demo-synthetic` policy-t dev DB-ben kapcsold ki (`enabled=false`), a failed jobjait `cancelled`-re állítsd, a breaker sorát zárd le (`state='closed', failure_count=0`, `ops.audit_event('breaker_reset')`) — különben a worker tick örökre `blocked=1`-et ír. A fotmob (robots) és a halott soccerway `/tegnap/` jobjai szintén permanensek: `cancelled`, auditálva.
- **Korai szezonok gap-fill**: a footballdata CSV-k 1993–2005 között hiányosak lehetnek (2. Bundesliga 1996/97: 56 vs v1 306). A `backfill v1-import --tournament <v1 id> --from-year --to-year --apply` lokálisan pótol; az ismeretlen csapatok review-ba kerülnek → `identity approve-create-all`, majd **újra kell futtatni az importot** (az első kör csak a már ismert csapatok meccseit anyagiasítja). **Ha a v1 versenynév eltér a v2-től** (pl. „La Liga 2” → „LaLiga 2”, „First Division (1992–2004)” → „Championship”), add meg a `--target-competition <v2 uuid>`-t: enélkül az import új verseny-entitást hoz létre, és a meccsek nem a meglévő ligába kerülnek. A cél UUID-t a `core.competition`-ból vedd (név + ország egyeztetés). Végül `dedupe fixtures` + reverify + integrity + coverage. A v1 tournament id a v1 `tournaments` táblájából (pl. 44 = 2. Bundesliga).
- **A gap-fill ne álljon meg a korai éveknél**: a footballdata több kisebb ligát (NB I, Ekstraklasa, Eliteserien, Superliga, Persha Liga, osztrák 2. Liga) egyáltalán nem vagy csak részben fed 2006 után. Mérd végig a v1 vs v2 szezon-számokat a teljes idővonalon (v1: `SELECT s.year_start,count(*) FROM matches m JOIN seasons s … GROUP BY 1`, v2: ugyanez `core.match`/`core.season`), és importáld a hiányzó éveket is `--target-competition`-nel, két körben (approve-create-all a körök között). Egy 11 ligás, 2006–2026-os kör ~23 000 meccset hozott az első, ~11 000-et a második passzban; utána kötelező a `dedupe fixtures` (több ezer biztonságos merge, maradék kockázatos párok web-ellenőrzéssel), majd reverify + integrity + `coverage --limit 2000`.

## Csapat- és fixture-duplikátumok v1 import után

- A v1 import **azonos nevű duplikátumokat** hozhat létre, ha a meglévő entitás `canonical_name`-je eltér a `core.team.name`-től (pl. kanonikus „AFC Ajax”, csapatnév „Ajax”), vagy ha csak ékezetben tér el (Fenerbahce/Fenerbahçe, Beşiktaş/Besiktas). A linker ezért a `team_entities_by_normalized_name` most **a `core.team.name`-re is** illeszt, az `approve_create` pedig **új entitás helyett a pontosan egyező meglévő entitást használja** (különben a párhuzamos review-sorok duplikált entitást hoznak létre).
- Az `identity merge-team` név-ellenőrzése `fold_name`-re váltott (ékezet-független, török `ş/ğ/ı/İ` is): azonos fold-név + kontextus mellett az ékezet-variáns merge engedélyezett; a „Res.”/„II”/U19 csapatok továbbra is külön entitások (a fold-név eltér).
- Csapat-merge után **kötelező** a `dedupe fixtures --apply`: a merge ugyanarra a csapatpárra hozhat két, eltérő kickoffú/sorrendű meccset. A biztonságos merge-ek után maradó kockázatos pároknál (fordított hazai/vendég, eltérő eredmény) először **webes forrással** állítsd helyre a kanonikus meccset (csapat-sorrend + eredmény, `ops.audit_event`), majd `dedupe fixtures --apply` — így a pár azonossá válik és biztonságosan merge-elődik. Ne hagyd a kockázatos párokat merge nélkül: az `integrity` ilyenkor hard fail.
- **A kézi eredmény-/sorrend-revízió önmagában nem csendesíti a gate-et**: a provenance-ban maradt eltérő megfigyelés `MATERIAL_SOURCE_CONFLICT`-ot ad, és a meccs withheld marad. A revízió után `resolve-conflict --match-id … --field match --winner <observation-id> --reason …` kell (a győztes observation a `fact.provenance`-ból választandó), majd reverify. A `match` mező az egész anyagi összevetést maszkolja; dokumentált indoklás kötelező.
- **Hükmen/technikai eredménynél a félidőeredményt ne találd ki**: `home_ht/away_ht = NULL`, különben a `HT_GREATER_THAN_FT` szabály blokkol. A `+:-` típusú v1/RSSSF jelölés műszaki győzelem, a szabály szerinti eredmény 3-0 — ezt írd a `reason`-be forrással.
- **Meglévő duplikált csapatok felderítése**: fold-old a `core.team` sorokat `(fold_name, country, gender, age_group, branch)` kulcsra; a >1 elemű csoportokat a legtöbb meccses entitásba merge-eld. A kész script: `scripts/team_dup_groups.py` (dry-run alapból, `--apply` a merge-hez); a fold a török `ş/ğ/ı/İ` betűket is kezeli, és **kihagyja azokat a párokat, amelyek egymás ellen játszottak** (azok azonos nevű, de külön klubok). A „Res.”/„II”/U19 csapatok külön entitások maradnak, mert a fold-név eltér.
