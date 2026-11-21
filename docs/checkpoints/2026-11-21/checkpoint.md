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

## Következő döntési pont

- Az 5 forrás-ellentmondás withheld kézi forrás-ellenőrzése; a 38 XX-duplikátum
  és 9 „Al-" név review-alapú rendezése; az 185 research review; az NB I
  UI-fixture hibája; a `tick.rs`-ben lévő nem commitolt auto-dedupe változás
  külön szelet (builder ≠ verifier).
