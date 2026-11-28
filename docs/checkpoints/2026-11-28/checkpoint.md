# 2026-11-28 — checkpoint

## Induló állapot és tervezett szelet

- `main`: `cece562`, preview: `cdf57a8`; a futó app a preview-ból készült.
- Tervezett apró szelet: reggeli gyors ellenőrzés (tegnapi nap + integritás +
  lemez), majd a kijelölt meccs teljes részletezésének beágyazása a főoldal
  jobb oldali paneljébe (S220). A kijelölés nélküli panel tartalma majd később
  jön — most csak egyértelmű üres állapot.

## Záró eredmény

### Reggeli gyors csekk

- [x] `daily-check.sh 2026-11-27`: **EXIT=0 / ZÖLD**; detektált hiányjelek
  362 (missing_logo 182, open_past 131, missing_odds 49), rögzítve audit nélkül
  rendezve. Tipplap 238 linkelt esemény, markets 59 done / 0 failed.
- [x] `ops.job`: failed **0** (a tegnap nyitott kérdés — a `daily-check` csak
  az előző napot nézte — most reads: minden napon 0 failed).
- [x] `integrity --from 2026-11-27 --to 2026-11-28`: `verdict: OK`,
  duplikátum 0, failed job 0, nyitott breaker 0.
- [x] Lemez: reggel 1,4 GB szabad volt a párhuzamos worktree-k `rust/target`
      könyvtárai miatt; a felesleges build-artifactok törlése után **25 GB**.
      Forrás, adat vagy konfiguráció nem lett törölve.
- [x] Daemon `/v1/health`: OK.

### S220 — jobb oldali meccsrészletező panel

- [x] **Kijelölés → panel:** a bal oldali kártyára kattintva a teljes
      meccsrészletező a jobb panelben jelenik meg; nem nyílik új ablak/sheet.
      A régi modalis sheet továbbra is él keskeny ablaknál (< 1200 pt),
      ahol nincs jobb panel.
- [x] **Egyetlen forrás:** a `MatchDetailSheet` tartalma átkerült a közös
      `MatchDetailPane`-be; a sheet és a beágyazott panel is ezt rajzolja.
      Így a betöltés, a hibaállapot, a szekciók és a kapcsolódó lapok
      (szerkesztés, láncolt meccs, csapat, verseny, játékos) egy helyen vannak.
      AX-azonosítók (`match.detail`, `match.detail.sections`, …) megmaradtak.
- [x] **Térkihasználás:** a panelek számára a tartalom szélessége nem az
      860 pt-es sheet-korlát (a panel `contentMaxWidth: nil`), így a részlet
      kitölti a ~1200 pt-es jobb oldalt. A sheet változatlan maradt.
- [x] **Kijelölés nélkül:** ideiglenes, egyértelmű üres állapot
      (`home.detail.empty`): „Válassz meccset…". A későbbi tartalom egy
      helyen, a `MatchDetailPanel.emptySelection` részben cserélhető.
- [x] **Mérés — valódi macOS ellenőrzés:** az app újratelepítve, újraindítva.
      Üres panel: `/tmp/fb-panel-empty.png`; kattintás a Leeds–Coventry
      kártyára → teljes részlet a panelben (`match.detail.section.*` AX-elemek
      jelen vannak): `/tmp/fb-panel-detail2.png`; másik meccsre kattintva
      azonnal vált (Manchester United–Brentford): `/tmp/fb-panel-second.png`;
      „Bezárás" → üres állapot: `/tmp/fb-panel-closed2.png`; széles tartalom:
      `/tmp/fb-panel-wide2.png`.
- [x] **Kapuk:** app build 0, app unit **TEST SUCCEEDED**; a teljes 15 perces
      XCUITest-harness **nem futott** (felhasználói döntés). Branch
      `feat/home-detail-sidepanel` pusholva, preview `c0fd3f2`-re merge-elve
      és pusholva.

### Visszaállás (S220)

- `git revert` a `feat/home-detail-sidepanel` commitjára, illetve a preview
  merge-jére; a kijelölés és az üres állapot kódja külön is visszaállítható.
  Adatot a szelet nem ír.

## S221–S227 — Részletes pre-match terv (dokumentáció, kód nélkül)

- [x] A felhasználó kérésére **először terv készült**, kód nem változott:
      `docs/superpowers/specs/2026-11-28-meccs-elonezet-melyseg-design.md` és
      `docs/superpowers/plans/2026-11-28-meccs-elonezet-melyseg.md`.
      A fejléc, a szűrők, a bal lista és a 25/75 arány változatlan marad.
- [x] **Adatfelmérés (read-only, konkrét számokkal):** a research/DB alapján
      `preview_hu` **88/27 035**, sérülések **2 881/27 035**, `season_stats`
      24 048/27 035 — de `average_rating` csak **22**, `possession_pct` **22**,
      `assists` **0**, `clean_sheets` **149** sor; `rank_label`/`goals_per_game`
      ~1 040/1 914 oldalon teljes. H2H gólok + `match_id` rendelkezésre áll,
      `fact.match_stats` 21 473/55 041 meccsre tartalmaz szöglet/lap/xG-t —
      ezért a H2H-összesítés számolható, de `coverage` jelzéssel.
- [x] **Kifejezett hiányok rögzítve:** nincs Sofascore-adapter/oszlop és
      megbízható szezonális góllövőlista; a „kezdhet-e" csak forrás szerinti
      szerep (kezdő/kispad/sérült/ismeretlen), nem megerősített kezdő.
      Ezeket a terv **nem ígéri**, nem tölti ki nullával vagy becsléssel.
- [x] A terv 7 szeletre bont (S221 adatszerződés → S227 integráció), minden
      szelet külön branch + commit + magyar AX/képi ellenőrzés; teljes
      XCUITest-harness továbbra sem része a kapunak.
- [x] Magyar szöveg ellenőrizve: CJK/replacement karakter **0** a két új
      dokumentumban.

## S221 — Adatszerződés a pre-match panelhez (jóváhagyva)

- [x] **Döntés:** a felhasználó az S221-gyel indított, UI még nem változott.
- [x] **Rust/storage:** `HeadToHeadEntry` 8 opcionális mutatóval bővült
      (szöglet, sárga/piros lap, xG) a `fact.match_stats` legfrissebb sorából;
      új `H2HSummary` (matches, with_stats, total_goals, btts_pct, over25_pct,
      corners_avg, cards_avg) a meglévő H2H-listából számolva — ott `null`, ahol
      nincs alapadat. `NextMatchEntry` `home_logo`/`away_logo` mezőkkel bővült.
      A régi mezők változatlanok; a kontrakt-fixture 70 → 71 kulcs.
- [x] **Swift/Core:** `MatchDetail.h2hSummary`, `HeadToHeadEntry` új mezői,
      `NextMatchEntry` logói, `InsightSeasonStat` 8 korábban dekódolatlan mező
      (clean_sheets, assists, possession_pct, average_rating, points,
      goal_difference…). Mind opcionális, régi payload-ek dekódolhatók maradnak.
- [x] **Tesztek:** Rust workspace **0 failed**, clippy 0, fmt 0; új egységtesztek
      a H2H-számolásra (BTTS 33,3%, over 33,3%, szögletátlag 10,0, lapátlag 3,0)
      és a hiány-esetre (`with_stats=0` → minden átlag `null`). Core **167/167**,
      app unit **TEST SUCCEEDED**. Teljes XCUITest-harness nem futott.
- [x] **Élő ellenőrzés:** daemon újraépítve/telepítve, `/v1/health` OK;
      Leeds–Coventry meccsen `/v1/match` 71 kulcs, `h2h_summary`:
      **5 meccs, 5 stattal, 13 gól, BTTS 60%, over2.5 60%, szögletátlag 9,8,
      lapátlag 4,8**; az első H2H-sor 3–8 szöglettel és 2–1 sárga lappal;
      a következő meccs mindkét logóval. Az adatot a szelet nem írta.
- [x] Branch `feat/match-preview-contract` @ `5dfa456` pusholva, preview
      `ba98a83`-re merge-elve. Az app binárisa nem lett cserélve (nincs UI-változás).

### Visszaállás (S221)

- `git revert 5dfa456`; a DTO-mezők opcionálisak, a régi fixture és a régi app
      továbbra is működik. Adatot a szelet nem írt.

## Következő döntési pont

- A felhasználónak jóvá kell hagyni az S221–S227 tervet, illetve döntenie kell
  arról, hogy a ritka mezők (preview, sérülés, rating) hiánya esetén a
  „nincs adat" üzenet elfogadható-e, vagy új forrás kell (Sofascore **nem**).
- Mi jelenjen meg a jobb panelben, ha nincs kijelölt meccs (napi összefoglaló,
  kedvencek, valami más)? A felhasználó ezt később dönti el.
- Nyitva marad: 2876 függő review, a `daily-check` napközni failed-job
  ellenőrzésének kiterjesztése, predikció shadowban, S9 AI-gomb. VoiceOver skip.
