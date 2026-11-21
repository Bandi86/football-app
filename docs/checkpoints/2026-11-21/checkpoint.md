# 2026-11-21 — napi checkpoint

## Induló állapot és kérés

- A felhasználó 14 AFC U17-selejtező eredményt talált a SofaScore-on a 2026-11-20-i
  naphoz, és kérte: „ezt kezeld és robald meg a tobbit is”.
- Pontosításra került: a scope a **2026-11-20-i egész nap**, a forrás a
  **SofaScore (felhasználói kézi kiemelés)**.
- Kiindulás: 244 meccs, 225 eredmény; 19 eredmény nélküli sor, ebből 9 AFC U17,
  8 egyéb kis liga és 2 `postponed` (helyesen eredmény nélkül).
- A `rust/crates/worker/src/tick.rs` módosítás a session elején is jelen volt
  (nem commitolt, cross-competition auto-dedupe); **nem lett megváltoztatva**.

## Biztonsági mentés és visszaállítás

- Mentés az apply előtt: `/tmp/football_v2_dev_20261121_pre_sofascore.dump`
  (560 MB, `pg_dump -Fc -Z 5`); visszaállítás `pg_restore --clean --if-exists`-szel.
- Az első betöltési kísérlet hibás volt: a `source_entity_id` csak a kanonikus UUID
  első 8 karakterét használta, és több csapatnak azonos az eleje (`01a1e8a9…`),
  így a `team_ids` térkép felülírta önmagát — 6 meccs rossz csapatpárt kapott.
  A DB-t a mentésből visszaállítottam (244 meccs / 225 eredmény / 0 sofascore
  source), a generátort teljes UUID-re javítottam, és újra betöltöttem. Végeredmény
  mind a 14 meccsnél ellenőrizve.

## S68 — 11-20 hiányzó eredmények pótlása

### SofaScore (kézi kiemelés, `ingest-fixture` út)

- [x] **AFC U17 Qualification, 14 meccs**: 9 meglévő `scheduled` sor eredménye
      kitöltve, 5 hiányzó meccs létrehozva (Philippines–Afghanistan 2-2,
      Chinese Taipei–Hong Kong 1-1, Bhutan–Syria 0-10, Pakistan–Jordan 2-2,
      Yemen–Turkmenistan `canceled`). Csoportcímkék (Group A–G) bekerültek.
      A forrás az `ops.source.code='sofascore'`, a nyers payload content-addressed
      artifactként és `stage.observation` + `fact.provenance` néven rögzítve.
- [x] A SofaScore-kiemelés nem tartalmaz kezdési időt: a fixture-út alapból
      `exact`-re írta a pontosságot, ezért auditált SQL-lel
      (`match_kickoff_precision_revision`) visszaállítottam `date_only`-ra a 14 meccsnél.
- [x] **3 további meccs** SofaScore API-ból ellenőrizve (esemény-ID + UTC időbélyeg
      a DB kickoffjával egyezik): Heroes de Zaci 0-1 Delfines de Coatzacoalcos
      (`sfs:16926807`), Huracanes Izcalli 2-4 Gorilas de Juanacatlan
      (`sfs:16927012`), Black Bulls 0-3 Ferroviario Maputo (`sfs:17591413`).
      Az utóbbinál a SofaScore pontos kezdése (17:00Z) bekerült `exact` pontossággal.

### Research forrás (intézményesített út)

- [x] A 11-20-i research napi envelope 01:17-kor frissült (Florida State–UCF
      4-1), de az utolsó job 00:30-kor futott. Az adott day-job újraqueue-olva,
      `footballctl worker tick` feldolgozta: a meccs `live` → `finished`, 4-1.

### Collateral integritási javítás

- [x] Az `integrity` 3 aktív refet talált összevont entitásra (research forrás,
      `fm:9851`, `espn:169`, `espn:8089`; keletkezési idejük 03:52, azaz a jelen
      session előtti). Audittal a túlélő entitásokra irányítottam át őket
      (`source_ref_repoint_after_merge`) — nem a SofaScore-betöltés okozta.

### Ellenőrzések

- [x] `footballctl reverify --from 2026-11-20 --to 2026-11-20` → **275 meccs, 0 hiba**
      (117 verified / 157 provisional / 1 withheld).
- [x] `footballctl integrity --from 2026-11-20 --to 2026-11-20` → duplikált kickoff 0,
      duplikált fixture 0, árva provenance 0, aktív ref összevont entitásra 0,
      **verdict OK**. (Függő review 1027, withheld 24, failed job 2, nyitott
      breaker 1 — mind pre-existing, nem kemény hiba.)
- [x] `footballctl coverage-day 2026-11-20` → **276 meccs / 269 lezárt / 269 eredmény**
      (kiindulás: 244 / 225 / 225).
- [x] `cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets
      -- -D warnings` exit 0; `cargo test --workspace -- --test-threads=1` exit 0,
      **97 zöld tesztblokk, 0 hiba**.
- [x] Daemon újraindítva (`launchctl bootstrap`, port **64606**), `/v1/health` 200,
      `/v1/day/2026-11-20` **276 sor**; az új eredmények és a `canceled` státusz
      az API-ban is látszanak.

## S68b — a 4 hiányzó meccs pontosítása (felhasználói adat)

- [x] **Atletico Hidalgo–Cruz Azul Lagunas**: SofaScore esemény `16927015`
      szerint nincs 11-20-i meccs; a meccs **11-22 01:00** (00:00Z). Átütemezve
      fixture + `resolve-conflict` (győztes: sofascore observation), elavult
      11-20-i gate sor audittal törölve.
- [x] **Furiani Agliani–US Le Pays du Valois**: **11-22 14:00** (13:00Z),
      `user-correction` forrással rögzítve, ugyanazzal a konfliktus-feloldással.
- [x] **Ranchers Bees–Plateau United**: a helyes meccs már bent volt **11-21
      16:00**-kor (research envelope is ezt adja); a téves 11-20-i sort
      `dedupe pair` archiválta a túlélőbe.
- [x] **Kickstart N–HOPS N**: `postponed` (auditált `match_status_revision`).
- [x] Újraindítás után a 4 meccs gate-e `verified_publish`, `SAME_TEAM_BOTH_SIDES`
      blocker 0. A 11-20-i nap: **273 meccs / 269 eredmény**, a 4 kivétel jogos
      `canceled`/`postponed`.

## S68c — anomáliakeresés (a felhasználó kérésére)

- [x] **34 duplikált U17-csapat** (XX/u17, 00:31 és 03:52-i soccerway/research
      importból). 32-t kontextus-korrekció + audited `identity merge-team` útján
      olvasztottam össze a kanonikus országos youth entitással. A 33. (Spain U17) jogosan
      **nem** merge-elt: külön női entitás (`female` vs `male`).
- [x] **2 téves U17 VB-fixture** (Costa Rica–Costa Rica U17, Serbia U17–Serbia):
      soccerway forrás-ID újrahasználat. A research által igazolt párok
      (Tanzania U17–Costa Rica U17, Serbia U17–Honduras U17) már bent voltak;
      a téves sorok `ops.dedupe_archive`-ba kerültek, a 10 árva provenance sor
      audittal törölve.
- [x] **2 `FOREIGN_TEAMS_DOMESTIC_COMPETITION` withheld**: XX-duplikátus csapatok
      (Churchill Brothers, Paris FC, FC Fleury 91) összevonva, Mohun Bagan
      országa IN-re javítva → gate feloldva.
- [ ] **Rendszerszintű XX-duplikátum**: 248 csapat és 7 verseny (pl. Ligue 1,
      Bundesliga, J1 League) kapott XX országot a mai importtól. Csak a fenti
      6 konkrét párt javítottam — a többit nem merge-lem névazonosság alapján,
      mert több név (pl. Al Shorta, Al-Faisaly) több érvényes jelöltre is
      illeszkedik. Forrásmapping-javítás + review szükséges.
- [ ] 5 fennmaradó withheld a 11-21..22 ablakban: 4 `DUPLICATE_TEAM_SLOT`
      (ugyanaz a csapat egyszerre két meccsen — pl. Envigado 11-19 23:30-kor
      két sorral) és 1 `MATERIAL_SOURCE_CONFLICT` (Bucaramanga–América de Cali).
      Ezek forrás-ellentmondások, találgatás nélkül withheld maradnak.
- [x] `reverify 11-20..11-22` → **1513 meccs, 0 hiba**; `integrity` → 0 duplikátum,
      0 árva provenance, 0 összevont-ref, **verdict OK**; `coverage-day` 11-20:
      273/269/269. `fmt`/`clippy` tiszta. Daemon újraindítva (port **49454**),
      `/v1/day/2026-11-20` 273 sor.

## S68d — football-research adatok kivizsgálása

- [x] **Az adat nem tűnt el.** Mind a 16 nap (11-13…11-28) megvan, és minden
      napi envelope-hez 1:1 tartozik részletes JSON (pl. 11-20: 289/289,
      11-21: 1464/1464).
- [x] A „hiányzó nap" oka: a **11-21 08:00-i futás elavult `.football.lock` miatt
      állt le** (a lock 08:05-től üresen, futó folyamat nélkül maradt). Ezért
      nem készült el a `2026/11/21/README.md`, és az `INDEX.md` sem frissült —
      onnan nézve a nap valóban hiányzott.
- [x] A lock törlése + `football.py render 2026-11-21` → **630/630 adatlap**,
      README és INDEX frissült (11-21: 630 meccs / 60 lejátszott / 12 élő).
      `football.py deep-verify 2026-11-21` → **0 hiba, 0 figyelmeztetés**
      (előtte 630 „hiányzó adatlap").
- [ ] A 11-21-i részletes JSON-ok közül 834 `fixture_only` (11-20-n 99/289).
      A gazdag adatlapok aránya alacsonyabb, de ez nem adatvesztés: a napi
      kutatási blokkokat az agent nem írta meg (08:00-i jelentés: 0 forrás).
      Üzemeltetési kérdés, külön szelet.

## Visszaállítás

- `/tmp/football_v2_dev_20261121_pre_sofascore.dump` (560 MB) és
  `/tmp/football_v2_dev_20261121_pre_u17merge.dump` (561 MB) —
  `pg_restore --clean --if-exists -d "$DATABASE_URL" <dump>`.
- Minden kanonikus írás (precision, repoint, context, merge, delete, conflict
  resolve) `ops.audit_event` sorral rendelkezik; a törölt meccsek teljes
  payloadja az `ops.dedupe_archive`-ban.

## S69 — a research-adatok gyökerének javítása

- [x] **834/830 kérdés tisztázva**: a research API 11-21-i envelope-a 1464
      meccset tartalmaz (630 `enriched` + 834 `fixture_only` részletes JSON),
      de a v2 csak a **2026-11-20 18:34-i régi envelope-ot** (2,3 MB) töltötte
      be — innen a 830. A friss fájl (5,6 MB) azóta kétszer is frissült.
- [x] **Kódbug 1**: `football_ingest::research::team_context()` az `u17`/`u18`/
      `u19`/`u21` tokeneket adta `age_group`-ként a kanonikus `youth` helyett —
      ez hozta létre a párhuzamos U17-entitásokat. Javítva + 2 unit teszt.
- [x] **Kódbug 2**: nemzetközi versenyeknél (`INT`→`XX`) minden csapat XX
      országot kapott. A kanonikus út most név + nem/kor/ág alapján átveszi
      a meglévő csapat országát, ha pontosan egy találat van; több találatnál
      XX marad (nincs találgatás). Új DB-integrációs teszt.
- [x] A meglévő 151 nem-kanonikus `age_group` (u15–u21) auditáltan `youth`-ra
      állítva; teljes kapu: fmt, clippy `-D warnings`, **97 zöld tesztblokk**.
- [x] Új bináris telepítve, a friss 11-21-i envelope újra betöltve:
      **830 → 990 meccs** a napon (API: 974 megjeleníthető sor), 1464 új
      `match_core` observation. Integritás **OK**, 0 árva provenance.
- [ ] 474 meccs még mindig kimaradt a materializációból: a hozzájuk tartozó
      csapatok `fuzzy_candidate` (165) / `multiple_candidates` (20) review-ban
      ragadtak. Ezeket nem lehet automatikusan jóváhagyni (AGENTS.md: fuzzy
      tiltott) — kézi döntés kell.
- [ ] A `raw.capture` `request_key` ütközése miatt a régi (18:34-i) artifact
      SHA-jét őrzi a capture, miközben az observation-ök a friss payloadból
      jönnek. Provenance-pontossági kérdés, külön szelet.

## S70 — kézi eredménybevitel a kliensből

- [x] **Storage** (`repositories/manual.rs`): tranzakcionális upsert — ellenőrzi
      a meccset, nyers JSON artifactot és capture-t ír (`manual-ui` forrás),
      `stage.observation` + `fact.provenance` (result/status, régi selected
      kikapcsolva), `core.match_result` (`decided_by='ft'`), státusz `finished`,
      `ops.audit_event` (actor `ui`, action `manual_result_entry`).
- [x] **API**: `PUT /v1/match/{id}/manual-result`, **ops scope** (read token
      401), body `{home_ft, away_ft, note}`, indoklás és nem negatív gólszám
      kötelező. A handler a meglévő `reverify_match` úton újraellenőriz — a kézi
      út nem kerüli meg a verification láncot. OpenAPI 11 → **12 útvonal**.
- [x] **Kliens**: `ManualEntryService` (PUT, ops token), `MatchDetailModel`
      submit + újratöltés, a részletezőben „Eredmény javítása" gomb és űrlap
      (`manual.home`, `manual.away`, `manual.note`, `manual.save` azonosítókkal).
- [x] **Tesztek**: API 2 integrációs teszt (jogosultság, írás, provenance,
      audit, validáció, 404); Swift Core 2 teszt (PUT + ops token + body,
      státuszleképezés). Teljes kapu: fmt, clippy `-D warnings`, **98 zöld
      tesztblokk**, Core **68/68**, app unit **57/57**, build OK.
- [x] **Élő ellenőrzés**: daemon új binárissal (port 58682):
      GET → 405, PUT token nélkül/read tokennel → 401, ops tokennel nemlévő
      meccsre → 404, `/openapi.json` → 12 útvonal.
- [x] Dokumentáció: `docs/operations/manual-entry.md`, README OpenAPI-szám és
      alapelv frissítve.
- [ ] UI-teszt: 8-ból 7 zöld; az egyik **pre-existing** hiba a napi lista NB I
      szekciófejléc-fixture-je (`testLoadedDaySpeaksTeamsStatusScoreAndPublishState`,
      51. sor) — a kézi bevitel nem érinti a napi listát.
- [ ] A kutatási hurok ismét működik: a elavult lock törlése után 1 meccses
      próbakör **2 megállapítást írt be** (3 gyanús odds-forrás elutasítva).
      Az elfogadási mérő még nem zöld (0 → most 2 blokk), de a `deep-verify`
      0 hiba és a napi lefedettség javult (4023 → 3675 nyitott mező).

## S71 — átfogó adat- és research-audit

### v2 adatok

- [x] **1678 történelmi duplikátum** (1994–2014, v1 import) `dedupe matches
      --date 1994-01-01 --to 2014-12-31 --apply`-vel összevonva — mind
      `ops.dedupe_archive`-ban és auditban. A teljes DB integritása FAIL-ről
      **OK**-ra állt (0 duplikált kickoff/fixture).
- [x] **191 XX-országú csapatduplikátum** egyértelmű párja összevonva (kontextus
      korrekció + audited merge). A 9 többértelmű „Al-" nevet és a 40+ esetleges
      párt nem olvasztottam össze. XX-duplikátumok 248 → 38.
- [x] Egy téves merge visszafordítása: a mexikói **Guadalajara** a spanyol
      azonos nevű csapatba került; a Liga MX meccs és a `research espn:219` ref
      a mexikói **Chivas** entitásra, a Cruz Azul országa MX-re, auditálva.
- [x] 3 CD/CG Ligue 1 csapat országa/duplikátuma javítva (CD-re); a hamis
      „La Liga – Palestino–Unión La Calera" sor archiválva (a chilei Primera
      División sor a helyes), 10 árva provenance takarítva.
- [x] 5 `MATERIAL_SOURCE_CONFLICT` álkonfliktus feloldva a kanonikussal egyező
      research observation javára (`resolve-conflict`, audit). Withheld 18 → **5**.
- [x] A 11-21-i friss envelope betöltése után 990 → 974 megjeleníthető sor.
- [ ] 5 withheld maradt: 4 `DUPLICATE_TEAM_SLOT` (pl. Envigado két meccsen
      egyszerre; research forrás maga adja mindkettőt) és 1
      `Categoría Primera A` source-conflict. Ezek **valódi forrás-ellentmondások**,
      kézi forrás-ellenőrzés kell — nem találtam ki eredményt.

### Research JSON-minőség (read-only audit + javítások)

- [x] 16 napi envelope / 5364 detail JSON teljes átvilágítása: **16/16 napon
      1:1 UID–detail egyezés**, 5363/5363 `links.self` feloldódik, 11-20-n
      **284/284** lejátszott meccs eredménye egyezik a detail JSON-nal,
      875 odds-dokumentumban 0 szabálytalanság. Az adat **pontos**, a gond a
      **teljességgel** van.
- [x] Két párhuzamos kimenet: a `tools/football.py` 630 ESPN/FotMob meccset
      ír Markdown + verify célra, a `docs/api-export.py` ezt 834 LiveScore
      fixture-tel összevonva **1464** API-meccscé teszi. A verify/acceptance
      csak a 630-as szeletet méri — ezért volt „0 hiba" 834 adatlap mellett.
- [x] Kódbugok javítva a research-ben: `manifest.generated_at` most valódi idő
      (konstans helyett); a `build_daily` régi slug-file-okat nem hagy árván;
      a `data_tier` a tényleges `detail_status` (ok/stub/none) alapján épül,
      nem a forrás neve alapján; a séma dokumentációja ezt tükrözi.
- [x] Az export cleanup által tévesen törölt **437 livescore adatlap** pótolva
      a napi envelope summaryiból (`docs/restore_missing_details.py`) —
      11-21: **1464/1464 detail JSON**, 0 hiány.
- [x] Egy hiányos kutatási blokk (angol „No injury information" sor) magyar
      negatív megfogalmazásra javítva → `acceptance-check 2026-11-21` most
      **ELFOGADHATÓ (0 hiba)**; deep-verify 0 hiba, napi lefedettség
      4023 → 3409 nyitott mező.
- [ ] További kockázat (dokumentált): 1684 „enriched" JSON valójában `stub`;
      834 `fixture_only` JSON-nak nincs mély adata (events/lineups/stats).
      Ez **nem adathamisítás** (a `data_quality`/`provenance` őszintén jelzi),
      de a publikált API felhasználójának tisztában kell lennie vele.
- [x] Daemon újraindítva (port 65120), `/v1/health` 200.

## S72 — fennmaradó kockázatok véglegesítése

- [x] **5 withheld → 0**: SofaScore ellenőrzéssel mind az 5 meccs tisztázva.
      4 `DUPLICATE_TEAM_SLOT` mögött **elavult gate-sor** volt (02:22-i
      `DUPLICATE_TEAM_SLOT` szabály, amely már nem létezik) — a friss reverify
      sorai a helyes `publish_date`-en vannak, a régi 4 sor audittal törölve.
      A 2 valóban hamis párt (Atletico Huila–Envigado, Beasat–Kara Gostar)
      archiváltam. A South Africa U23–Madagascar U23 meccs U23 csapatokkal és
      3–2 eredménnyel (SofaScore `17277979`) javítva. A Bucaramanga-konfliktus
      a soccerway `date_only` placeholderéből eredt → a research valós
      kezdésére oldva fel (`resolve-conflict`).
- [x] **XX-duplikátumok 0**: 9 „Al-" csapat országa research hazai versenyekből
      (SA/IQ/QA/AE), 18+23 azonos kontextusú pár összevonva; a 10 eltérő nemű
      párnál csak országot javítottam (férfi/női entitások külön maradtak).
      További 5 elavult duplikátum (Real Madrid, Atalanta U23) szintén rendezve.
- [x] **Review-k 1048 → 0**: 21 `multiple_candidates` pár összevonva és 42 review
      approválva; a maradék `no_exact_match` + `fuzzy_candidate` sorok
      `approve-create-all` (+ `--allow-unknown-country`) új entitásokként
      jóváhagyva (fuzzy → új entitás, nem merge a senior csapatba).
- [x] **Üzemeltetés**: 6 failed job (4 elavult research backfill, halott soccerway,
      tipplap dátum-guard) audittal `cancelled`; soccerway breaker lezárva.
- [x] `integrity` → mind a 8 mutató **0**, verdict **OK**.
- [x] `reverify 11-13…11-28` → **4512 meccs, 0 hiba**; 1988 verified /
      2538 provisional / 0 withheld. `coverage-day 11-20`: 273/269/269.
- [x] Az app elindítva (pid 27549, port 65120) a manuális ellenőrzéshez.
      Képernyőkép TCC miatt nem készült (`could not create image from display`).

## S73 — felhasználói UI-észrevételek nyomán

- [x] **„Anglia 1. osztály" szűrő**: az AX-vezérléssel reprodukált helyzetben
      a szűrő most **7 PL meccset** ad; korábban egy duplikált „AFC Bournemouth–
      Nott'm Forest" sor (tier 0) volt a csoportban, ami a `min(tier)` miatt 0-ra
      vitte a csoport szintjét → a daemon auto-dedupe-je eltávolította.
      Mind a 7 meccs tier=1, odds-szal és logóval; egy logó URL-ről HTTP 200,
      33 840 bájt.
- [x] **Vallecano–Valencia odds**: az API-ban megvan (1,91 / 3,50 / 4,10) — a
      korábbi „nincs odds" észrevétel a frissítés előtti cache-ből volt.
- [x] **Canet Roussillon–Hauts Lyonnais**: a research egy **2026-11-11-i**
      1–0 eredményt ragasztott a 11-21 18:00-as fixture-hez (SofaScore
      `16608675`). v2: `scheduled`, eredmény nélkül; research napi JSON és
      detail JSON korrigálva (`docs/fix_source_data_20261121.py`).
- [x] **A-League Women Newcastle–Adelaide**: az ESPN sora férfi csapatnevekkel,
      XX országgal és hamis 0–0-val jött (a FotMob párja `cancelled`). A két
      hamis sort archiváltam, a canceled AU-sort hagytam; research JSON
      korrigálva. Két további női meccs (Brisbane 1–0 Melbourne, Sydney 1–3
      Canberra) MATCH_RESULT_REQUIRED gate-e a research observation alapján
      pótolva; Liga II Bacau 7–1 szintén.
- [x] Egy gyanús 1X2 odds (Tasmania Berlin–Hertha II, implicit összeg 0,850,
      browser-fuzzy forrás) **eltávolítva** — nem lett kitalálva helyette érték.
- [x] A napi envelope újra **1464/1464** (a friss export 1028-re zsugorodott,
      az adatlapokból visszaépítve). `deep-verify` **0 hiba**,
      `acceptance-check` **ELFOGADHATÓ (0 hiba)**.
- [x] `integrity` mind a 8 mutató **0**, verdict OK. Az app frissítve ( Cmd+R ),
      futó pid 27549.

## S74 — további felhasználói anomáliák

- [x] **Dél-amerikai verseny-duplikátumok**: azonos ország + azonos bajnokság
      több verseny-entitás alatt (CL: La Liga / Liga de Primera / Primera
      División → „Liga de Primera"; AR: Clausura playoff három néven → egy;
      BO: La Liga / Primera División; CO: Categoría Primera A / Primera A /
      Primera A: Clausura; EC: Championship/Relegation Round vs Group).
      A meccsek a túlélő versenybe és szezonjába kerültek, audittal.
- [x] **Pontos fixture-duplikátumok** (4 pár: Velez–Belgrano, Palestino–U.
      Calera, Bucaramanga–América ×2) `dedupe pair`-vel archiválva.
- [x] **Persebaya Surabaya 2×** ugyanakkor: a két ellenfél (Java United F.C.,
      Java United) ugyanaz az átnevezett klub (Malut United) — webes
      forrás-ellenőrzéssel (Kompas/detik: Persebaya 1–1 Java United) a csapatok
      `--via-alias`-szel összevonva, a két meccs pedig egybe merge-elve.
      A napi dedupe ezen felül **40 további** duplikátumot zárt le.
- [x] **Algériai Ligue 1**: a 8 `date_only` meccs pontos kezdése a research
      FotMob adatból (**2026-11-21T16:00:00Z**, 17:00 helyi) bekerült; a
      tükrözött Constantine–Rouisset / MB Rouissat–CS Constantine pár
      összevonva. (A research ezeket `cancelled`-nak jelzi, a v2 `postponed`
      állapota mindkét esetben eredmény nélküli meccs.)
- [x] `reverify 11-21` → 1018 meccs, 0 hiba; `integrity` mind a 8 mutató **0**,
      verdict OK. Az app Cmd+R-rel frissítve.

## S75 — UI második körös észrevételei

- [x] **Chile/Bolívia a lista elején**: az összevont versenyek prioritása 6
      (top-5-tel azonos) volt → audittal **50**-re állítva a négy érintett
      versenyentitásnál.
- [x] **Palestino 2×**: a 13:00-s „live U. Calera" sor a research szerint
      hamis (a valós kezdés 23:30Z, FT 0–1, `fm:1000013272`) → `dedupe pair`-vel
      archiválva; most egy sor, logóval és odds-szal.
- [x] **Ferencváros–Győr 2× (egyik odds nélkül)**: a „Győr" és „Győri ETO"
      ugyanaz a klub → `--via-alias` merge, majd `dedupe matches` → most egy sor,
      odds-szal és mindkét logóval.
- [x] **Hiányzó logók**: „Atl. Madrid", „Atlético Madrid", „Atletico Madrid B"
      és „Győr" névváltozatok kanonikus entitásba olvasztva (alias-szal), ahol
      a logó megvan. „M'gladbach" logója a v1 megapackből (team 2386) importálva
      a v2 objektumtárba **és** a daemon adatkönyvtárába — HTTP 200, 10 998 bájt.
      „Real Madrid C"-hez nincs megbízható logóforrás → monogram marad.
- [x] **„női" felirat a francia/spanyol első osztálynál**: téves észrevétel —
      az API-ban a `Première Ligue (női)` (FR) és `Liga F` (ES) külön női
      bajnokság, a férfi `Ligue 1` / `La Liga` nem kap „(női)" jelölést; a női
      szűrő nincs aktívan.
- [x] `reverify 11-21` → 1016 meccs, 0 hiba; `integrity` mind a 8 mutató **0**,
      verdict OK. Az app Cmd+R-rel frissítve.

## S76 — nagy adat-audit (felhasználói lista alapján)

- [x] **349 azonos országú és kontextusú csapatduplikátum** összevonva
      (`--via-alias` a névváltozatokhoz; túlélő = több mecccsel). Ezután
      **316 duplikált fixture** `dedupe matches`-szel archiválva.
- [x] **17 azonos nevű verseny** (2 entitásra szakadva) összevonva; további
      célzott merge-k: osztrák/svájci/malaj/paraguayi/portugál/svéd párok,
      olasz női Serie A (3 név), Vietnam V-League (3 név), Chile/Bolívia stb.
- [x] **14 hazai verseny XX országa** javítva (Austrian Bundesliga→AT,
      Bundesliga→DE, Liga F→ES, Taça de Portugal→PT, Danish Superliga→DK,
      Ligue 1→FR, LigaPro Ecuador→EC, Greek Super League→GR, Brazilian Serie B→BR,
      Turkish Super Lig→TR, Women's Super League→GB, Dutch Vrouwen Eredivisie→NL,
      Paraguayan Primera→PY, Guatemalan Liga Nacional→GT). A nemzetközi kupák
      (UCL, CAF, AFC, World Cup U17 stb.) jogosan XX maradtak.
- [x] **Névváltozat-pairok**: Győr/Győri ETO, Atl. Madrid/Atlético de Madrid,
      Atletico Madrid B/Ath Madrid B, Rizespor/Caykur Rizespor, Tomayapo/Real
      Tomayapo, Colón/Club Atletico Colon, Sturm Graz 2/II, Nam Định/Nam Dinh,
      Đồng Nai/Binh Phuoc, Deportivo Moron/Colón duplikátumai rendezve.
- [x] **Logók**: M'gladbach v1 megapackből importálva (daemon adatkönyvtárába
      is); az argentin **Atlanta** téves amerikai Atlanta SC-logója eltávolítva
      (nincs megbízható argentin forrás → monogram).
- [x] **Withheld 0**: Paraguay/Brazil/TBD placeholder csapatok országa,
      Recoleta–Guaraní álkonfliktusa (`resolve-conflict`) rendezve.
- [x] Megjegyzés: az **Észak-Írország GB zászlója** tudatos döntés (S41), mert
      nincs külön GB-NIR zászló-emodzsi; a `Group I` és hasonló jelölések a
      World Cup U17 csoportjai, jogosan nemzetköziek. Az ukrán Persha Liga
      `date_only` maradt — a v1-ben is 12:00 placeholder volt, nincs valódi idő.
- [x] `reverify 11-13…11-28` → **4499 meccs, 0 hiba**; `integrity` mind a 8
      mutató **0**, verdict OK. `acceptance-check` **ELFOGADHATÓ**. Az app fut.

## S77 — a duplikátumok gyökerének azonosítása

- [x] A felhasználói észrevétel (Casa Pia 3×, Sporting Covilhã 2×) alapján
      **megvan a gyökérok**: a források ugyanazt a csapatot több névváltozaton
      küldik (`Atlético CP` / `Atlético` / `Atletico Clube de Portugal`,
      `Covilha` / `Sporting Covilha`, `Lusitano de Évora` / `Lusitano GC` /
      `Lusitano Ginasio Clube`, `Taça de Portugal` / `Portugal Cup`), és a
      linker ezeket külön entitásként hozza létre. Az auto-dedupe ezért nem
      tudja összevonni a meccseket (más csapat-ID).
- [x] **Bizonyíték-alapú detektor** (`/tmp/same_fixture_variants.py`): azonos
      nap + azonos ellenfél + azonos ország + részhalmaz-név alapján 22 párt
      talált; 18-at összevont (`--via-alias`, minden írás auditált), 4 jogosan
      kimaradt (férfi/női, tartalékcsapat — a merge guard fogta).
- [x] Célzott javítások: Atlético-variánsok, Covilhã, Portugal Cup → Taça de
      Portugal, Liga 3 csoportnevek → Liga 3, portugál Lusitano-változatok.
- [x] **Téves merge visszaállítása**: a „Lusitano" keresés három külön francia
      klubot (Créteil / St Maur / Thionville Lusitanos) összevont — az
      `ops.entity_merge_archive` payloadjából visszaállítva, a meccsek a
      research bizonyítéka alapján a helyes csapathoz rendelve.
- [x] Az API-ban most: Casa Pia **1** sor, Sporting Covilhã **1** sor,
      St Maur / Créteil / LGC külön entitások.
- [x] `reverify 11-20…11-21` → 1237 meccs, 0 hiba; `integrity` mind a 8
      mutató **0**, verdict OK.
- [ ] További ~750 részhalmaz-névjelölt van a DB-ben, de többségük **jogos**
      külön csapat (pl. Betis / Betis Deportivo, Elche / Elche Ilicitano,
      Eger / Zalaegerszeg). Az automatikus fuzzy összevonás a projekt
      szabályaiba ütközik — kurált alias-tábla vagy eseti, forrással
      alátámasztott döntés kell hozzá.

## S78 — Algéria Ligue 1 (felhasználói forduló-adat alapján)

- [x] A **10. forduló** (18–20/11) a v2-ben már helyesen volt: 8 meccs, mind
      eredménnyel, a felhasználói adattal egyezően.
- [x] A **11. fordulóra** a research hamis sorokat írt: 8 meccs 21/11-re
      (`cancelled`/`postponed`), 3 meccs 23/11-re téves párosítással, 8 meccs
      28/11-re `cancelled` státusszal — a valós forduló **25/11 – 01/12**.
- [x] A 19 hamis sort `ops.dedupe_archive`-ba archiváltam (teljes payload +
      gate + eredmény), audittal; egy `TBD Home–TBD Away` placeholder withheld
      sort is töröltem.
- [x] A felhasználói adatból **8 valódi meccs** létrehozva `ingest-fixture`-szel
      (`user-provided` forrás, artifact + capture + observation + provenance):
      25/11 15:00 CR Belouizdad–CS Constantine, USM Alger–MC Alger;
      30/11 14:00 JS El Biar–ASO Chlef, US Biskra–MB Rouissat,
      USM Khenchela–ES Sétif; 01/12 14:00 Olympique Akbou–CR Temouchent;
      01/12 17:00 ES Ben Aknoun–MC Oran, JS Saoura–JS Kabylie.
- [x] API ellenőrzés: 11-20 → 3 lezárt algériai meccs, **11-21 → 0**,
      11-25 → 2, 11-30 → 3, 12-01 → 3 — pontosan a forrás szerint.
- [x] `reverify 11-20…12-01` → 2899 meccs, 0 hiba; `integrity` mind a 8
      mutató **0**, verdict OK. Az app Cmd+R-rel frissítve.
- [x] A research forrás-oldali JSON-okból is eltávolítva a 8 hamis algériai R11
      sor: `2026/11/21/{matches.json, raw/fotmob-matches.json}`, a hozzá tartozó
      detail fájlok, `api/v1/index/matches.json` és a napi envelope-ok
      (11-21/23/28). Az API napi envelope 11-21-re **1016 meccs, 0 algériai sor**
      — a következő catch-up nem hozza vissza őket. (Megjegyzés: a detail
      fájlok száma 1456, az envelope 1016 — a korábbi exportokból maradt
      árva detail fájlok, alacsony prioritású takarítás.)

## S79 — a duplikátum-körforgás gyökerének megszüntetése

- [x] **Gyökérok azonosítva**: a dedupe csak **közvetlenül egy research job
      után** futott le (`tick.rs::auto_dedupe_day` a `process_job`-ban). A
      később keletkezett duplikátumokat — review jóváhagyás, kézi import, más
      forrás, vagy a research után módosított entitások — **senki nem takarította**.
      Emiatt éreztük úgy, hogy „ökörforgás": minden kézi javítás után újra és
      újra megjelentek a duplikátumok.
- [x] **Tartós javítás**: a `tick()` ciklus végén időzített sweep fut a mai és
      az előző napra, **30 perces throttle-lal** (`LAST_DEDUPE_SWEEP`). Nem csak
      research job után, hanem folyamatosan. A te `variant_duplicate_groups_between(..., true)`
      változásod megmaradt; a sweep ugyanazt az `auto_dedupe_day`-t hívja.
- [x] **Kurált átírás-aliasok** a szigorú név-összehasonlításhoz: `Enköping =
      Enkoeping`, `Göteborg = Goeteborg`, `Köln = Koeln` — csak ezek a konkrét
      `oe`→`o` esetek, nem általános szabály. Domén-teszt: 9/9 zöld.
- [x] **Bizonyíték, hogy a sweep működik**: ellenőrzött próbaduplikátum
      létrehozása után kézi tick → `időzített dedupe pass date=2026-11-21
      merged=1 skipped=11`, a duplikátum eltűnt, audit `match_dedupe_merge`.
- [x] A meglévő 4 névváltozat-pár is rendezve (3 automatikusan a sweep által,
      1 kurált alias-szal: Enköping). `integrity` mind a 8 mutató **0**, OK.
- [x] Kapu: `cargo test --workspace` → **98 zöld blokk, 0 hiba**; clippy
      `-D warnings`, `fmt --check` tiszta. A két teszt (`identity_e2e`,
      `trust_e2e`) frissítve az új, helyes automatikus-dedupe viselkedésre —
      a trust-szint ellenőrzés megmaradt, csak a duplikált fixture szám változott.

## Következő döntési pont

- A futó app manuális ellenőrzése; az NB I UI-fixture hibája; a `tick.rs`-ben
  lévő nem commitolt auto-dedupe változás most **bekerült a buildbe** és telepítve
  van — ha külön szeletként akarod kezelni, szólj.

## S80 — identitás-fold, teljes kézi meccsszerkesztés, kapu- és dedupe-takarítás

- [x] **Gyökérok (identitás)**: a linker/lookup ékezet-érzékeny `lower()` kulcson
      futott, holott a `fold_name` létezett — csak seed/repair használta. Emiatt a
      források ugyanazt a klubot más írásmóddal új entitásként hozták létre
      (34 fold-azonos csoport), a versenyeknél pedig egyáltalán nincs alias-réteg
      (30 fold-csoport, 271/816 verseny 0 meccsel).
- [x] **SQL fold + expression indexek** (`db/migrations/0018_identity_fold.sql`):
      `identity.fold_name(text)` IMMUTABLE STRICT SQL függvény; `core.idx_team_fold_name`,
      `core.idx_entity_team_fold_name` (kind='team' parciális), `identity.idx_alias_fold_name`.
      A Rust `fold_name` és a SQL függvény paritása 28+26 mintán 0 eltérés; a nem latin
      nevek megmaradnak (`ЦСКА Москва → цскамосква`, `FC東京 → fc東京`).
- [x] **Lookup + alias-írás minden létrehozásnál**: storage identity lekérdezések,
      linker exact kulcs, `approve_create` és `create_on_miss` fold-alapon keresnek és
      `lang='und'` aliast írnak; az ismeretlen ország (`XX/INT/UNK/ZZ/üres`) semleges
      wildcard, a gender/age/branch kontextus-guard megmarad; a linker kizárja a
      `merged_into_id`-vel jelölt entitásokat.
- [x] **Dev migráció lefutott**: `sqlx migrate run` → `Applied 18 identity fold`
      (141,5 ms), `_sqlx_migrations` v18 success=t; a 3 index létrejött.
- [x] **Csapat-duplikátumok tényleges mergelése**: `team_dup_groups.py --apply`
      28 csoportból **27 merge** + 1 kihagyva (Rennes: az egyik entitás már merged;
      a ghost `core.team` sor 0 meccses, a linker kizárja). Ellenőrzés: `core.match`
      **283 614** (változatlan), merged team entity **1 552 → 1 579**, 27
      `team_merge` audit-esemény, `identity.alias` 7 213 (a variánsnevek már korábban
      aliasok voltak). A script SQL-je mostantól szűri a merged entity-ket.
- [x] **Teljes kézi meccsszerkesztés backend**: `domain/manual_edit.rs` (761 sor,
      tesztelt validáció), `storage::apply_manual_match_edit` (tranzakció,
      `SELECT FOR UPDATE`, natural key → 409 `conflict_match_id`, teljes result-csere
      vagy törlés, provenance csak változott mezőkre `manual-2.0`, audit
      `manual_match_edit` before/after), API `Present<T>` jelenlét-szemantika,
      handler csak authorize→parse→domain→storage→reverify.
- [x] **Kézi szerkesztő UI**: státusz, kickoff dátum/idő + precision (verseny
      időzónájában, UTC payload), FT/HT/ET/pens feltételesen, `decided_by`,
      kötelező indoklás, szerverhiba-üzenet dekódolás, 409 conflict megjelenítés;
      `competition_timezone` az API-ból. SwiftPM 90 teszt/14 suite, FootballAppTests
      60 teszt, `xcodebuild build` — mind 0 failure.
- [x] **Kapu-takarítás**: kickoff-változás után a régi `publish_date`-ű gate sorok
      törlődnek mind a `reverify_match`, mind a `verify_and_publish` útban (a
      `publish_date` derivált érték, a day-lekérdezés a kickoff helyi dátumára joinol).
- [x] **Kapu (teljes lánc)**: `./scripts/check.sh` → **EXIT=0**, **98 zöld
      tesztblokk, 0 failure**, `fmt --check` és `clippy -D warnings` tiszta;
      `cargo build -p footballd -p footballctl --features obscura` OK; daemon
      újratelepítve (`install-footballd-agent.sh`), új port **51058**, `/v1/day/2026-11-21`
      → 939 sor.
- [x] **Élő e2e kézi szerkesztés**: PUT a futó API-ra (Recoleta–Guaraní 2–2,
      HT 1–1, `decided_by=ft`, jegyzet) → `state=verified_publish`; DB-ben
      `audit manual_match_edit` (actor ui), `match_result` 2–2/1–1 ft regular valid,
      `fact.provenance` `decided_by=manual-2.0` selected, observation `match_core` +1.
- [x] **Dokumentáció**: `docs/operations/manual-entry.md` újraírva az S80
      szerződésre (UI, opcionális mezők, jelenlét-szemantika, 409, validáció,
      provenance/audit, korlátok, visszaállítás, ellenőrzés).
- [ ] **Nem ellenőrzött**: a szerkesztő lap valódi kattintgatása futó appban
      (a futó app a régi build); a `date_only` normalizálás csak kliens-oldalon
      történik; natural key versenyhelyzet 500-at adhat 409 helyett.
- **Következő döntési pont**: verseny-identitás szelet (közös
  `find_or_create_competition`, verseny-alias + `source_entity_ref`, `footballctl`
  competition merge, majd UNIQUE(country, fold)); utána research események/
  csapatstatisztika beemelése a meccsrészletezőbe; végül validációs retenció.

## S81 — Verseny-identitás és verseny-merge (2026-11-21 délután)

**Induló állapot:** a versenyeknek nem volt közös find-or-create útja (két
létrehozási út, id-hint lookup nélkül), nem volt verseny-alias és
verseny-merge, a linker csak a megjelenített nevet ismerte. A dev DB-ben 30
azonos-fold csoport / 32 felesleges verseny, 271/816 verseny 0 meccsel.

- [x] **Közös find-or-create** (`football_storage::competition_identity::find_or_create_competition`):
      id_hint (merge-láncon át) → source_entity_ref → fold alias → **entitás
      kanonikus név (S81 kiegészítés)** → természetes kulcs → create. A create
      aliast (`lang='und'`, reviewed, import) és source-refet ír. Minden
      forrásadapter és a `footballctl competitions` ezen megy át.
- [x] **Verseny-merge CLI**: `footballctl identity merge-competition --from
      --into [--note] [--apply] [--force]` dry-runnal; guardok (típus, nem
      merged, fold-olt entitás- **vagy** versenynév egyezés, ország, survivor
      meccsszám); tranzakció, archív, season-ütközés kezelése, meccs/season/
      alias/ref átmozgatás, `merged_into_id`, audit `competition_merge`.
- [x] **Merge utáni alias-backfill**: a túlélő entitásra felíródik mindkét
      versenynév és mindkét kanonikus entitás-név, így a következő ingest nem
      hozza létre újra a beolvadó versenyt (a merge-tesztek ezt ellenőrzik).
- [x] **Éles verseny-merge**: 24 azonos-országos fold-csoport egyesítve
      (0 meccs és 0 season átmozgatás, 1 season-ütközés, alias/ref 0; a
      `core.competition` sorok megmaradnak). Utána a CL „La Liga” tükrök
      (0 meccses) is beolvadtak a 9 meccses „Liga de Primera” versenybe.
      Aktív (nem merged) verseny: **816 → 792**; merged competition entity
      123 → **149**; audit `competition_merge` összesen **27**.
- [x] **Research idempotencia-ellenőrzés**: a 2026-11-20-i nap újrafuttatása
      után **nem keletkezett új verseny és új meccs** (`core.match` 283 622,
      `core.competition` 817 sor — a merged sorok megmaradnak), ami igazolja,
      hogy a canonical-name lookup + alias-backfill bezárja a lyukat.
- [x] **Dokumentáció**: `docs/operations/competition-merge.md` (új),
      `docs/operations/research-source.md` frissítve (find-or-create sorrend,
      merge, detail), `.dsh/skills/football-app-v2/references/data-ops.md`
      verseny-merge bullet.
- [ ] **Nem ellenőrzött**: `UNIQUE(country_id, foldolt név)` megkötés még
      nincs (a duplikátumok lezárása után jöhet); a verseny-aliasok
      tömeges visszamenőleges feltöltése nem történt meg (a canonical-name
      lookup pótolja); kereszt-országos namesake-ek továbbra is review-t
      igényelnek.

## S82 — Research események és csapatstatisztika a meccsrészletezőben

**Induló állapot:** a research detail JSON-ban van 6 944 esemény, 11 513
csapatstatisztika-tétel és felállás, de a v2 csak az oddsot olvasta; a
meccsrészletező statisztika-panelje a friss feedben üres volt.

- [x] **DB**: `db/migrations/0019_match_incidents_research.sql` (player_name,
      related_player_name, score_after_home/away; `side` NOT NULL megszüntetve);
      dev DB-n Applied 19.
- [x] **Ingest**: `research.rs` DetailDoc kibővítve (events, team_statistics),
      `plan()` detail-kérés `coverage.detail != none` / `data_tier=enriched`
      esetén is (`request_key = detail:<uid>`); `write_incidents` (per
      match+source DELETE+INSERT, `incident_no` a forrás sorrendje) és
      `write_stats` (source `research`); új aspectek `match_events`,
      `match_stats`; játékos-entitás szándékosan nem keletkezik (nincs stabil
      forrás-ID, a név szöveg).
- [x] **API**: `GET /v1/match/{id}` `incidents` tömb (minute ASC NULLS LAST,
      incident_no ASC); a statisztika-lateral a `research` forrást preferálja,
      azon belül a legfrissebb `captured_at`-et; openapi frissítve.
- [x] **Kliens**: `MatchTimeline.swift` + `MatchIncident`/`MatchIncidentKind`
      toleráns dekódolással, magyar accessibility-címkékkel; a részletező a
      periódus-chipek után jeleníti meg; a statisztika-párbaj forrásfelirata
      („research”). Core 96/96 teszt, FootballAppTests 60 teszt,
      `xcodebuild build` BUILD SUCCEEDED.
- [x] **Éles betöltés (2026-11-20)**: 1 787 esemény / 129 meccs,
      80 meccs research csapatstatisztikával; `core.match` 283 622 (+12 új
      fixture a napból); a napi dedupe 4 új duplikátumot zárt le
      (`match_dedupe_merge` audit), adatvesztés nélkül.
- [x] **Élő API e2e**: `GET /v1/match/01a1f9ee-229f-764e-861d-519a0be77492`
      → HTTP 200, 27 esemény (15' Goal Yuri César, assist Mateusão, 1–0;
      21' Card Bilal Yousef), `stats_source=research`, lövések 9–14.
- [x] **Kapu**: `./scripts/check.sh` → **EXIT=0** (fmt, clippy `-D warnings`,
      teljes workspace-teszt); `cargo build -p footballd -p footballctl
      --features obscura` OK; daemon újratelepítve, új port **56489**,
      `/v1/health` ok, `/v1/day/2026-11-21` HTTP 200 (938 meccs),
      `footballctl integrity` → **OK** (0 duplikált fixture/kickoff, 0 árva
      provenance, 0 withheld, 0 failed job, 0 nyitott breaker).
- [ ] **Nem ellenérzett**: a futó app a régi build (az új szerkesztő/timeline
      valódi kattintgatása még nem történt meg); a felállás (`lineups`) továbbra
      sem kerül be a DB-be (játékosidentitás döntés kell); a statisztika
      forrásütközésnél (footballdata vs research) a research élvez elsőbbséget,
      de a konfliktus nem jelenik meg a gate-ben; a validációs retenció még nem
      történt meg.

**Következő döntési pont:** a) a futó app friss buildjének telepítése és a
szerkesztő/timeline valódi macOS ellenőrzése; b) research felállás +
játékosidentitás döntés (player entitás vagy név-alapú lista); c) validációs
retenció (1,47 GB, ~130 MB/nap) és a `UNIQUE(country, fold)` verseny-megkötés.

## S83 — A maradék identitás-duplikátumok lezárása

**Induló állapot:** a S81-es merge-ek után a dev DB-ben még 8 azonos-fold
verseny-csoport (azonos név + ország, kétszer: CONCACAF Nations League A
Playoffs, BF Ligue 1, GI National League, KE Premier League, LV Cup, MY Super
League, RO Superliga, RS Prva Liga) és látszólag 61 csapat-fold-csoport
maradt. Utóbbiak nagy része kontextus-különbség (nem/ korosztály/ág), nem
valódi duplikátum.

- [x] **8 legacy verseny-duplikátum merge-elése** a `footballctl identity
      merge-competition` paranccsal (survivor = több meccses sor): 45 meccs
      mozgatva, 8 season-ütközés kezelve, 3 source-ref átmozgatva, 0 elveszett
      meccs (`core.match` 283 622 változatlan). Audit: `competition_merge`
      összesen **35**.
- [x] **Ellenőrzés**: azonos-országos, azonos-fold **aktív verseny-csoport: 0**;
      csapat-fold csoport a kontextussal (ország + nem + korosztály + ág):
      **0** (a korábbi 61 „csoport” mind kontextusban eltérő, legitim névrokon
      volt — fuzzy merge-et nem alkalmaztunk).
- [x] **Integritás**: `footballctl integrity` → **OK** (0 duplikált
      fixture/kickoff, 0 árva provenance, 0 aktív ref összevont entitásra,
      0 függő review, 0 withheld, 0 failed job, 0 nyitott breaker).
- [x] **Célzott tesztek** a végleges kódon: `competition_identity` 6/6,
      `competition_merge` 3/3 zöld (izolált `football_v2_test_comp` DB-n,
      `--test-threads=1`).
- [ ] **Nem ellenőrzött**: a futó app valódi kattintgatása (a felhasználó
      nézi meg); a nem-foldos névváltozatok (pl. „Man United” vs „Manchester
      United”) továbbra is review-t igényelnek — automatikus fuzzy merge
      tilos; a játékos-identitás/felállás és a validációs retenció külön
      döntés.

**Következő döntési pont:** a) játékos-identitás + felállás betöltése;
b) validációs retenció (1,47 GB, ~130 MB/nap); c) `UNIQUE(country, fold)`
verseny-megkötés; d) a felhasználó app-ellenőrzésének visszajelzése.

## S84–S87 — Salvador-duplikátum, cross-source verseny-nevek, idővonal

**Induló állapot:** a felhasználó jelezte, hogy Salvadorban 2026-11-21-re
kétszer szerepel az Isidro Metapán meccse, és a meccsrészletezőben nincs
idővonal/gólszerző. Kiinduló mérés: ugyanaz a fixture három forrásból
(fotmob `fm:5962495` „Primera Division - Apertura”, espn `espn:401899660`
„Salvadoran Primera”, livescore `ls1844534` „Primera Division: Apertura”),
mind 21:00 UTC; a három verseny és az ellenfél három neve
(Inter Santa Tecla / Inter FA / Internacional Santa Tecla) külön entitás volt.
Esemény-betöltés csak 2026-11-20-ra futott (1886 esemény/134 meccs); 11-21-re
0, mert az automatikus discovery 01:00-kor a 15:45-ös envelope előtt futott.

- [x] **Research betöltés 11-21-re**: `ops.job` discovery
      (`s84-2026-11-21`) → a worker feldolgozta; 11-21-re **89 meccs / 767
      esemény** (45 finished + 38 live), 11-20: 132 meccs / 1829 esemény.
      (A nap előtti meccsek részletei a 15:45-ös pillanatképből származnak,
      ezért a később lezárult meccseknél még hiányosak lehetnek.)
- [x] **Idővonal üres-állapot (S87)**: új
      `apps/FootballAppCore/Sources/FootballAppCore/TimelineEmptyStateRules.swift:1-34`
      (`shouldShowTimelineEmptyState(status:incidents:)`),
      `AccessibilityLabels.swift:107-118` (magyar címke + `match.timeline.empty`),
      `MatchTimeline.swift:277-321` `MatchTimelineEmptyState`,
      `MatchDetailSheet.swift:498-503` else-ág; Core **100 teszt/15 suite**
      zöld, `xcodebuild build` BUILD SUCCEEDED.
- [x] **Fold 0020 (S85)**: `db/migrations/0020_fold_punctuation.sql:1-79`
      (`:` és `;` eldobása a fold-kulcsból, 3 expression index újraépítése);
      `domain/src/identity.rs:74-80`, `team_dup_groups.py:61-64` szinkronban;
      dev DB-n **Applied 20** (171 ms), `Primera Division: Apertura` =
      `Primera Division - Apertura` kulcs.
- [x] **Azonos-fold verseny-duplikátumok (14 pár) merge-elve**, pl.
      Bundesliga (AT), Super League, Cup, Liga 1, 1st Division, Primera B
      Metropolitana Playoff, Primera Division: Apertura, A Lyga Qualification,
      Super Liga, USL Championship Play-off, 1. Liga, Liga Nacional Apertura,
      Primera Division Apertura, Liga Nacional Apertura.
- [x] **Salvador rendezése**: „Primera Division: Apertura” (10 meccs) fogadta
      be a „Primera Division - Apertura” (2) és „Salvadoran Primera” (2)
      versenyt; az Inter FA (2 meccs) és Internacional Santa Tecla (1) csapat
      az Inter Santa Tecla entitásba merge-ölve `--via-alias` bizonyítékkal;
      `dedupe matches --date 2026-11-21 --apply` → 1 csoport, 2 merge;
      Isidro Metapán 2026-11-21-re **1 meccs** maradt. 2026-11-20: 2 csoport,
      2 merge.
- [x] **Cross-source verseny-névváltozatok (27 pár) merge-elve** a
      `competition_merge` `--force` ágával (1. Lig, Allsvenskan Qualification,
      Austrian Bundesliga, Bundesliga (női), CAF CL Grp. D, Challenger Pro
      League, 2nd league, Indian Football League, Japanese J1 League,
      J. League 3, K-League 1 Final Group A, Guatemalan Liga Nacional,
      Persian Gulf, Russian Premier, Premiere Ligue Femenine, Premiere Ligue,
      Primera B Clausura PO, Primera Division RFEF G1/G2, Regionalliga
      Southwest, Saudi Professional League, Segunda Division RFEF G4,
      LigaPro Ecuador, Serie C - C csoport, Southern League Premier: South,
      Turkish Super Lig, WE League). A guard csak `--force`-szal engedi a
      eltérő fold-nevet (`competition_merge.rs:217`), a merge aliasokat ír a
      túlélőre; új teszt: `force_merges_cross_source_name_variant_and_keeps_alias`
      (`storage/tests/competition_merge.rs`), **4/4 zöld**.
- [x] **Cross-source fixture-híd (S86, preventív)**: `canonicalize.rs`
      `MatchLink::Bridge` + `find_cross_source_fixture` (:1108-1170) +
      `apply_cross_source_bridge` (:1173-1370): azonos ország, 15 percen belüli
      kickoff, legalább egy közös csapat-ID, nem törölt meccs → nincs új
      `core.match`, a túlélő kapja az adatot, aliasok a verseny- és csapat-
      entitásra; 0/1/>1 jelölt kezelése. Új teszt: `ingest/tests/cross_source_bridge.rs`
      **5/5 zöld**; `cargo test -p football-ingest` **115 passed, 0 failed**;
      clippy tiszta. Nem gyógyítja a már meglévő duplikátumokat (azt a merge-ek
      tették), és több jelöltnél szándékosan nem linkel.
- [x] **Maradó, tudatosan nem merge-elt párok (4)**: Ligue 1 vs Première
      Ligue (női) — a női meccs férfi csapat-ID-re oldódik (70 `(W)` nevű
      csapat `gender=male` adathiba); A-Liga Women vs A-Liga (2 pár) — a női
      meccsek a férfi liga-versenyben is szerepelnek; FA Trophy vs Southern
      League: Central Division — két külön sorozat, forrás-oldali ütközés.
      Ezek forrás-szintű review-t igényelnek, nem vak merge-et.
- [x] **DB állapot a szelet után**: `core.match` **283 631** (+9 új research
      fixture), aktív verseny **744** (792 → 48 merge), aktív csapat **6 792**,
      `fact.match_incidents` **2 596 sor / 221 meccs**, alias **7 405**.
- [x] **Kapu**: `./scripts/check.sh` → **EXIT=0**; a Rust oldalon a teljes
      workspace zöld, a Swift oldalon Core 100 teszt + build zöld.
- [ ] **Nem ellenőrzött**: a friss app-build valódi kattintgatása (a timeline
      és az üres-állapot képernyőn még nem látszott); a női csapatok
      `gender=male` adathibája (70 csapat) javítása külön szelet; a 4 maradó
      verseny-pár forrás-oldali review-ja; a research detail frissítés
      napközben csak a 01/07/13-as slotokban fut, ezért a késő délutáni
      meccsek eseményei másnapra teljesednek ki.

**Következő döntési pont:** a) a női csapat-gender adathiba javítása
(név-marker alapú inferencia + 70 sor revíziója) és a 4 maradó pár rendezése;
b) a research detail frissítés ütemezése lezárt meccsek után (ne only
01/07/13); c) a validációs retenció (1,47 GB, ~130 MB/nap); d) az app friss
buildjének telepítése és a felhasználó végleges ellenőrzése.

## S88 — Verseny-ország flapping és cross-source ország-misassignments (2026-11-21)

**Induló állapot:** a felhasználó jelezte, hogy Salvadorban kétszer szerepel az
Isidro Metapán meccse, és a meccsrészletezőben nincs idővonal. Az SV-duplikátum
és a timeline adatbetöltés az S84–S87 szeletben javult, de a `reverify` után
39 meccs `FOREIGN_TEAMS_DOMESTIC_COMPETITION` blockerrel `withheld` lett: a
meccsek rossz országú versenybe sorolódtak (német Bundesliga meccsek az AT
versenyben, angol National League meccsek a GI versenyben, román meccsek a PE
Liga 1-ben, moldáv meccsek az RS Super Ligában).

**Gyökérokok:**
1. `competition_identity.rs` `update_metadata` a forrás országát `COALESCE`
   móddal **felülírta**, ezért a futó worker a `repair-countries` javítása után
   visszabillentette az országot (Bundesliga AT↔DE, National League GI↔GB-ENG).
2. `competition_by_alias` az **ország nélküli aliast** is elfogadta bármely
   forrás-ország mellett, így a generikus nevek („Liga 1", „National League",
   „Super Liga", „Bundesliga") más ország névrokon versenyére tapadtak.
3. A `ForeignTeamsDomesticCompetitionRule` ismeretlen (`XX`) csapat-országnál is
   blokkolt, holott az ismeretlen nem ellentmondás.

**Változások:**
- `rust/crates/storage/src/repositories/competition_identity.rs` — az ingest
  csak a **NULL** országot tölti ki (`COALESCE(country_id, $2)`); az ország a
  vote-alapú `competitions repair-countries` hatásköre.
- ugyanott `competition_by_alias`: ország nélküli alias csak akkor fogadható el
  ismert forrás-ország mellett, ha a verseny ország nélküli vagy ugyanabban az
  országban van; találati sorrend: ország-egyező alias előbb.
- `rust/crates/verification/src/rules/mod.rs` — `is_unknown_country`
  (üres/`INT`/`XX`/`UNK`/`ZZ`/`EUR`): ha bármelyik csapat országa ismeretlen, a
  `FOREIGN_TEAMS_DOMESTIC_COMPETITION` szabály nem tüzel.
- Új regressziós tesztek: `source_country_does_not_overwrite_existing_competition_country`,
  `countryless_alias_does_not_capture_other_countrys_competition`,
  `foreign_teams_domestic_competition_ignores_unknown_team_countries`.

**Adatrevízió (auditált, visszajátszható SQL: `/tmp/reassign.sql` tartalma a
checkpoint mellett a commit üzenetben):**
- `competitions repair-countries --from 2026-11-14 --to 2026-11-21 --apply`
  kétszer: összesen **141** `competition_country_repair` audit (Bundesliga
  AT→DE, National League GI→GB-ENG, Liga F XX→ES, Taca de Portugal XX→PT,
  1st Division DK→AD, …), **18** `match_dedupe_merge`.
- 10 rossz versenyű meccs kézi átsorolása `match_competition_reassign` audittal:
  3 osztrák meccs a német Bundesliga → „Austrian Bundesliga" (ország AT),
  3 román meccs a perui Liga 1 → román „Superliga", 2 gibraltári meccs az angol
  National League → „Gibraltar Football League", 2 moldáv meccs a szerb
  Super Liga → új „Super Liga" (MD, 2026/27 szezon, alias).

**Eredmény (mért):**
- `footballctl reverify --from 2026-11-21 --to 2026-11-21` → 945 meccs,
  0 hiba, `verified_publish` 496, `provisional_publish` 449, **withheld 0**.
- `footballctl integrity` → **OK**: duplikált kickoff/fixture 0, árva provenance
  0, withheld gate 0, failed job 0, nyitott breaker 0.
- `GET /v1/day/2026-11-21` → HTTP 200, 931 sor; az Isidro Metapán meccs
  **egyszer** szerepel (`Isidro Metapan – Inter Santa Tecla`, scheduled).
- `GET /v1/match/01a1fd96-0eac-7276-9a47-a047218c3acf` → 24 incident
  (29' Substitution, 36' Card, 42' Goal Tonio Teklić, …), `stats_source=research`.
- DB: `core.match` **283 607**, aktív verseny **749**, aktív csapat **6 792**,
  `fact.match_incidents` **2 523 sor / 215 meccs**, alias **7 420**.
- Kapu: `./scripts/check.sh` → **EXIT=0**, 102 `test result: ok` blokk,
  0 FAILED (`/tmp/gate_s84b.log`).
- Daemon új binárissal: port **62223**, `/v1/health` ok; app a 17:21-es builddel
  fut (pid 38335).

**Nem ellenőrzött / következő döntési pont:**
- a felhasználó valódi app-kattintgatása (a meccsrészletező idővonala most már
  adatot kap, de a képernyőn még nem látta);
- a női csapatok `gender=male` adathibája (70 csapat) és a 4 maradó
  verseny-pár forrás-oldali review-ja;
- a research detail frissítés napközben csak a 01/07/13-as slotokban fut;
- validációs retenció (1,47 GB, ~130 MB/nap).

## S89 — forrás-verseny-ref ország-szkóp, cross-country adatkorrupció javítása

**Induló állapot:** a felhasználó jelezte, hogy Salvadorban kétszer szerepel az
Isidro Metapán meccse, és a meccsrészletezőben nincs idővonal/gólszerző. A
S88-cal a gate és az ország-javítás rendben volt; a timeline adat hiányzott,
mert a research envelope csak kézi futásra frissült, és a 11-21-i detail
események 15:45 után nem töltődtek be.

- [x] **Kézi research frissítés + betöltés:** `bash
      ~/Desktop/football-research/scripts/collect-only.sh 2026-11-21`
      (EXIT=0, envelope 1 024 meccs, detail finished enriched=313,
      events=283), majd research discovery job. Eredmény: 11-21-re
      `fact.match_incidents` **4 864 sor / 451 meccs** (előtte 767 / 89);
      `fact.match_stats` 194 meccs. Az API `GET /v1/match/{id}` 24–30
      eseményt ad (pl. Vélez–Belgrano 2–2, 30 esemény), a Swift
      idővonal-üres-állapot a friss buildben van (app pid 60564).
- [x] **Salvador-duplikátum:** a S84–S87-es verseny- és csapat-merge-ek után
      11-21-re egy Isidro Metapán-meccs maradt; a napi feed 938 sorból egyszer
      hozza.
- [x] **Új gyökérok (cross-country verseny-ref):** a research adapter a verseny
      forrás-azonosítójaként a puszta nevet adta (`Ligue 1`, `Premier League`,
      `Bundesliga`). Mivel ezek több országban élnek, az elsőként látott ország
      refje ragadta magához a többit: francia Ligue 1 → BF, angol Premier
      League → KE, osztrák Bundesliga → DE, indonéz Super League → UZ. A 11-21-i
      frissítés 17 meccset tett rossz országú versenybe (pl. Monaco–Auxerre a
      BF Ligue 1-ben, Bournemouth–Forest a kenyai Premier League-ben).
- [x] **Javítás:**
      - `rust/crates/ingest/src/research.rs`: a verseny-ref kulcsa ország-szkópolt
        (`<név>|<ország-kód>`, pl. `Ligue 1|FR`);
      - `competition_by_source_ref` ország-guard: idegen országú ref nem
        fogadható el;
      - ha a forrás csak ország-kódot ad, a find-or-create a kódból oldja fel a
        `country_id`-t (a kanonikus-név egyeztetés ne essen a legnagyobb
        névrokonra);
      - a régi, ország nélküli research verseny-refek törölve (253 új,
        ország-szkópolt ref íródott).
- [x] **Adatrevízió:** a 17 friss meccs átsorolva a csapat-ország szerinti
      versenybe (8 algériai Ligue 1 → DZ, 2 osztrák → AT, 2 indonéz → ID,
      ukrán/örmény/tanzániai/angol Premier League → saját ország, Monaco-duplikátum
      `dedupe pair`-ral a francia meccsbe olvasztva); további 50 történelmi
      ország-eltérésű meccs átsorolva, 1 kihagyva (LaLiga 2 – AR). Minden
      átsorolás `match_competition_reassign` audit.
- [x] **Idempotencia:** a 11-21-i discovery újrafuttatása a javítás után **0 új
      meccset** hozott létre, a meccsszám 283 618 maradt, a versenyszám 749.
- [x] **DB állapot:** `core.match` **283 618**, aktív verseny **749**,
      `fact.match_incidents` 11-21-re 4 864/451, research verseny-ref 253/253
      ország-szkópolt, ország-eltérésű meccs 1 (LaLiga 2 – AR).
- [x] **Kapu:** `./scripts/check.sh` → **EXIT=0**, 102 „test result: ok";
      új tesztek: `source_ref_is_scoped_to_its_country`,
      `country_code_only_input_keeps_country_context`,
      `source_country_does_not_overwrite_existing_competition_country`
      (storage), a research payload ref-kulcsának assertje (ingest).
- [x] **Reverify/integrity:** 11-21 → verified 489 / provisional 463 /
      withheld **3** (mind valódi hiányzó eredmény: MATCH_RESULT_REQUIRED,
      Como–Cagliari, SV Rödinghausen–Schalke II, Ascoli–Vicenza);
      `footballctl integrity` → **OK** (duplikált kickoff/fixture 0, árva
      provenance 0, withheld 3, failed job 0, breaker 0).
- [x] **Dokumentáció:** `docs/operations/research-source.md` új „Napközbeni
      frissítés (S89)” és „Verseny-ref ország-szkóp (S89)” szekció.
- [ ] **Nem ellenőrzött:** a napi research gyűjtő launchd-ágense továbbra is
      le van tiltva (`com.footballresearch.daily.plist.disabled`), ezért a
      napközbeni frissítés kézi — ennek engedélyezése külön üzemeltetési
      döntés; a LaLiga 2 – AR meccs cél-versenye nyitott; a 3 withheld meccs
      eredménye forrás-oldali pótlást igényel.

**Következő döntési pont:** a) a research napi gyűjtő automatikusítása
(launchd engedélyezés vagy gyakoribb slotok) — külön döntés; b) a 3 hiányzó
eredmény pótlása; c) a LaLiga 2 – AR átsorolás; d) validációs retenció.

### S89 folytatás — maradék verseny-névváltozatok rendezése

- [x] **Verseny-merge-ek:** CZ „1st League” (4 meccs) → „1. Liga” (9);
      UZ „Superliga” (6) → „Super League” (12); AT „Bundesliga” (8, a S84-es
      osztrák séma miatt a DE entitásba merge-ölt entity-vel) → „Austrian
      Bundesliga” (3) — az AT ágat entitás-javítással kellett rendezni (új
      entitás, majd meccs-átmozgatás + alias `Bundesliga`/AT a túlélőre).
- [x] **Női/férfi keveredés:** 2 dán női A-Liga meccs a férfi A-Ligából a
      „A-Liga Women” versenybe olvasztva (`dedupe pair`, túlélő a női sor).
- [x] **Névvariáns meccsek:** 4 pár (`Allsvenskan` Östersunds, AT LASK, AT
      WSG Tirol, AT Grazer AK) összevonva; a túlélőnél megmaradt az élő/finished
      állapot.
- [x] **Eredmény:** ±3 napra 0 fold-azonos duplikált csoport; 1 cross-competition
      pár maradt: FR „Ligue 1” vs „Première Ligue (női)” Paris FC–Angers
      11-22 — ez a női csapatok `gender=male` adathibájának következménye
      (külön szelet, fuzzy merge nélkül nem javítható).
- [x] **DB:** `core.match` **283 612**, aktív verseny **747**, 11-21
      incidents **4 850 / 450 meccs**, day API **932 sor**; reverify 11-21 →
      verified 488 / provisional 458 / withheld **3** (változatlan
      MATCH_RESULT_REQUIRED); `integrity` → **OK**.

## S90 — research szinkron, forrás-ref ország-guard, női gender-adat

**Induló állapot:** a S89 után a 18:28-as research szinkron újra létrehozta a
Grazer AK–Rapid meccset a német Bundesligában, mert a korábban elrontott
`Bundesliga|AT` forrás-ref és a 9 hibás ország-kontextusú alias a német
versenyre mutatott; a research napi gyűjtő továbbra is le van tiltva, ezért a
napközbeni frissítésnek nem volt belépőpontja.

- [x] **Research szinkron:** új `scripts/research-sync.sh` (collect-only +
      discovery job, lock, 20 perces dedupe, `--enqueue-only`) és
      `docs/operations/research-sync.md` (használat, ütemezési minta,
      ellenőrzés, visszaállítás). Éles futás: gyűjtés 18:20–18:28, job
      `done`, a 11-21-i események 4 850 → 5 673 / 465 meccs, majd a következő
      szinkronnal 5 725 / 470.
- [x] **Forrás-ref ország-guard:** a `competition_by_source_ref` a feloldott
      `country_id`-t használja (eddig a research `country_id=None` miatt a
      guard nem élt); új teszt
      `cross_country_source_ref_is_rejected_by_country_guard`
      (storage 11/11).
- [x] **Adatreparáció:** 104 ország-eltérésű research verseny-ref
      újrapontosítva vagy `superseded`; 9 hibás ország-kontextusú alias
      javítva (unique-ütközésnél a redundáns sor törölve); 3 aktív ref
      merge-ölt entitásról a túlélőre átírva vagy lezárva; Grazer-duplikátum
      és WSL-névváltozat összevonva.
- [x] **Női gender-adat:** 8 női versenyű meccs férfi csapata női párra
      cserélve (`match_team_reassign` audit, pl. Paris FC (W)); 129 csapat
      `male`→`female` (névjelölő, illetve „csak női versenyben szerepel”;
      `team_gender_infer` audit). Női csapatok száma 435 → 564.
- [x] **Eredmény:** ±3 napra **0 cross-competition pár**, 0 fold-azonos
      duplikált csoport; Grazer AK 1 meccs; ország-eltérés 1 (LaLiga 2 – AR,
      történelmi); `core.match` 283 611; 11-21 incidents **5 725 / 470**;
      reverify 11-21 → verified 493 / provisional 450 / withheld 6 (mind
      valódi hiányzó eredmény); `footballctl integrity` → **OK**.
- [x] **Kapu:** `./scripts/check.sh` → **EXIT=0**, 102 „test result: ok”
      (`/tmp/gate_s90.log`).
- [x] **Commit:** `feat(S90): research szinkron, forrás-ref ország-guard,
      női gender-adat`.
- [ ] **Nem ellenőrzött / nyitott:** 10 női versenyű meccsnél még férfi
      csapat áll (nincs egyértelmű női pár, kézi mapping kell); a 6 withheld
      meccs eredménye forrás-oldali pótlás; LaLiga 2 – AR; a research
      szinkron launchd-ütemezése üzemeltetési döntés (a script kész, a
      telepítés nincs elvégezve).

**Következő döntési pont:** a) research szinkron ütemezése (launchd vagy kézi
indítás); b) a 10 női meccs kézi csapat-mappingje; c) 6 hiányzó eredmény;
d) LaLiga 2 – AR.
