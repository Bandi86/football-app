# Adatmegjelenítés és mélység — FotMob-szerű részletes nézetek (design)

**Dátum:** 2026-11-26
**Státusz:** JÓVÁHAGYOTT (2026-11-26) — a §10 döntései rögzítve:
[`2026-11-26-adatmegjelenites.md`](../../decisions/2026-11-26-adatmegjelenites.md).
A dokumentum önmagában nem tartalmaz kódot; a megvalósítás a
[`2026-11-26-adatmegjelenites-fotmob.md`](../plans/2026-11-26-adatmegjelenites-fotmob.md) terv szerint indul.
**Kérés:** a meccsrészletező és a verseny-/csapat-/játékosoldal legyen «szép és részletes»,
sok információval, szűrőkkel és mindenhez linkkel; az app kezdjen hasonlítani egy FotMob-szerű
részletes metrikás adatoldalhoz.

**Kapcsolódó, már jóváhagyott dokumentumok:**

- [`2026-11-09-football-app-v2-design.md`](2026-11-09-football-app-v2-design.md) — §3.3–3.7: oldaltartalom, adatminőségi felületi szerződés, akadálymentesség, UX-teljesítmény.
- [`2026-11-20-app-design.md`](2026-11-20-app-design.md) — „éjféli stadion” vizuális nyelv, `Scoreboard.swift` komponensei.
- [`2026-11-23-match-insight-design.md`](2026-11-23-match-insight-design.md) — a research-insight szekciók és a bizalmi modell (S113 már implementálva).
- [`2026-11-25-app-oldalak-bovites-design.md`](2026-11-25-app-oldalak-bovites-design.md) + [`2026-11-26-app-oldalak-bovites.md`](../../decisions/2026-11-26-app-oldalak-bovites.md) — a hiányjelek, jegyzetek, szerkesztés és logófeltöltés döntései.

Ez a dokumentum **csak a megjelenítést, az információarchitektúrát és a hozzá szükséges adatelérési réseket** írja le. Nem változtat adaton, nem indít új scrapinget, nem vezet be fogadást, és nem old fel identitásreview-t.

---

## 1. Mi a probléma (tényalap)

A backend az elmúlt hetekben jelentősen gazdagodott, a kliens viszont még mindig csak a kanonikus
minimumot és egy részletesebb, de befejezetlen insight-blokkot mutat. A terv nem „új adatot”
kér elsősorban, hanem **a már meglevő adatot teszi láthatóvá, szűrhetővé és visszakövethetővé**.

| Terület | Ma megvan | Ma nincs / láthatatlan |
|---|---|---|
| Meccsoldal | `ScoreboardHero`, meta-rács, periódusok, eseményidővonal, forma, H2H, tabella-környék, következő meccsek, `StatsDuel`, 3 odds-piac, `MatchInsightView`, verifikációs sáv | felállás, játékosszintű meccsstatisztika, lövéstérkép/xG-térkép, momentum, játékvezetői stáb, edzők, kommentár, közvetítések, kapusstatisztika, támadási zónák, riport szöveg; a dekódolt `seasonStats`, `fanPoll`, teljes `marketsRaw`, `byBookmaker` és preview-részletek nincsenek kirajzolva |
| Versenyoldal | versenyek, meccsek, tabella, csapatok, szezonok, alap aggregátumok | hiányjel-összefoglaló, form/osztott tabella, csapatkattintás, forráslinkek, szűrés/sorrend, jegyzet |
| Csapatoldal | profil, ELO, forma, következő meccs, meccsek versenyenként | keret/játékosok, aliasok, identitásreview, forráslinkek, jegyzet, kattintható formameccsek |
| Játékosoldal | `player`, `spells`, `appearances`, `stats`, `incidents` | linkszerű navigáció meccsre/csapatra, szezon- és versenyszűrés, teljes lista (a lista `prefix(10)`/`prefix(20)`-ra van vágva) |
| Linkek | belső meccs→meccs lánc; forrás szövegként | **nincs egyetlen kattintható link sem**: sem `Link`, sem `openURL`, sem `NSWorkspace`; a research `match.links.source`, `source_urls[]`, `standings.url`, `links.reference`, `odds.by_bookmaker.*.sources` nincs eltárolva kanonikusan és nincs az API-ban |
| Keresés | `⌘F`,.team/competition/match/player találatok | csapat- és versenytalálat csak fülváltást végez, **nem nyitja meg az entitást** |
| Beállítások | `UserDefaults` + helyi időjárás-fájl | nincs beállításoldal (téma, frissítés, AI-kapcsoló) |

Mennyiségi kép egy mintanagról (`2026-11-24`, 187 research detail fájl; a teljes export ~38 869 fájl):
`managers 187`, `odds 187`, `provenance 187`, `links 187`, `match_report_hu 180`, `standings 174`,
`fan_poll 154`, `season_statistics 153`, `weather 139`, `league_table 134`, `head_to_head 104`,
`lineups 100`, `officials 92`, `team_statistics 81`, `player_statistics 70`, `goalkeeping 67`,
`shotmap 42`, `momentum 42`, `attacking_zones 42`, `injuries 37`, `broadcasts 18`, `preview_hu 6`,
`commentary 1`, `prediction 1`. Eseménytípusok: `Substitution 784`, `Goal 445`, `Card 351`,
`Half 305`, `AddedTime 128`, `Comment 18`, `MissedPenalty 7`, `VAR 6`, `PenaltyShootout 5`.

A `fact.match_insight` (migráció `0027`, S113) már **4 563 sort** tartalmaz Novemberben; a Swift DTO
mind a 9 szekciót dekódolja, de csak egy részét rajzolja ki. A `fact.match_lineups`,
`fact.match_player_stats`, `fact.match_shots`, `fact.match_incidents` táblák (`0009`) szintén
vannak és a `footballctl player-import` tölti őket — **csak a `/v1/player/{id}` végponton keresztül
érhetők el**, meccs-szintű végpont nincs rájuk.

---

## 2. Termékcél és sikerfeltételek

1. **Első képernyő:** a meccsoldalon 1 képernyőn belül látszik az eredmény, a státusz, a verseny,
   a kickoff és az adatminőség (spec §3.7).
2. **Mélység:** egy meccsen belül szekciókra bontva érhető el minden rendelkezésre álló tény:
   összefoglaló, események, statisztika, felállás, játékosmetrika, lövések, odds, forma/H2H,
   tabella, sérülések, források, jegyzetek, előzmények.
3. **Visszakövethetőség:** minden megjelenített szakaszon van forrás, rögzítési idő és a
   spec §3.5 szerinti minőségi állapot (`Megfigyelt`, `Számított`, `Hiányzó`, `Elavult`,
   `Konfliktusos`, `Karanténban`) — szövegben is, nem csak színnel.
4. **Nincs rejtett nulla:** hiány esetén a szekció «Hiányzó» jelölést és javítási akciót mutat
   (spec §3.5), nem `0`, nem `0–0`, nem üres cella.
5. **Navigáció:** a napi listáról bármely entitás legfeljebb 3 lépésben elérhető (spec §3.7);
   meccs ↔ csapat ↔ verseny ↔ játékos ↔ másik meccs mindkét irányba kattintható.
6. **Szűrés:** minden «sok soros» nézetnek (meccsoldal odds/játékosstatisztika, verseny meccsei
   és tabellája, csapat meccsei és kerete, játékos statisztikája) van szűrője és rendezése;
   a beállítások `UserDefaults`-ban megőrződnek (spec §3.6).
7. **Linkek:** minden entitásnál elérhető a belső hivatkozás; ahol a forrás URL-t tárolunk,
   ott kattintható külső forráslink is van — engedélyezett, tárolt URL-ből, felhasználói tartalom
   nélkül (AGENTS.md §7).
8. **Akadálymentesség:** WCAG 2.1 AA, teljes billentyűzetes navigáció, VoiceOver-címkék,
   `reduceMotion`, magyar `hu-HU` formátum; minden új interaktív elemnek `accessibilityIdentifier`.
9. **Bizonyíték:** «kész» csak futtatott Rust/Swift/UI kapu + valódi macOS AX-ellenőrzés +
   képernyőkép + magyar szövegek visszaolvasása után (AGENTS.md §8).

---

## 3. Információarchitektúra és navigáció

### 3.1 Felső szint

A négy fül marad: `Főoldal`, `Versenyek`, `Csapatok`, `Üzemeltetés`; a globális keresés `⌘F`.
A játékosoldal továbbra is sheet/stack része, nem új fül — de **elérhető a keresésből és a
csapat/keret/ felállás sorokból is**. Beállítások: opcionális, külön szelet (l. 10. szakasz).

### 3.2 Entitásgráf (belső linkelés)

```text
Napi lista ──meccs──> Meccsrészletező
                         ├── verseny ──> Versenyoldal ──csapat──> Csapatoldal
                         ├── hazai/vendég csapat ──> Csapatoldal ──játékos──> Játékosoldal
                         ├── felállás/játékosstatisztika ──> Játékosoldal
                         ├── esemény/forma/H2H/következő ──> másik Meccsrészletező (vissza veremmel)
                         ├── források ──> külső URL (csak tárolt URL)
                         └── jegyzetek / előzmények (meglévő S1–S3 API)
Keresés ──találat──> a találat **konkrét entitása** (ma csapat/verseny esetén csak fülváltás van)
```

A meccs→meccs lánc (`MatchDetailSheet.sheet(item:)` + „Vissza”) már működik; ezt nem szabad
megtörni. Az új entitáslinkek ugyanazt a verem-modellt követik: minden sheet tartalmazza a
„Vissza” akciót és az `accessibilityIdentifier`-t.

### 3.3 Meccsoldal szekciói (javasolt rend)

Ragadó fejléc (hero + státusz + minőségi jel) alatt **szekciófülekkel** navigálunk, nem egy
hosszú egyetlen ScrollView-ben. A fülek: `Áttekintés`, `Események`, `Statisztika`,
`Felállás`, `Lövések`, `Odds`, `Források`. A `Forma`, `H2H`, `Tabella`, `Következő` és a
jelenlegi insight-tartalom az `Áttekintés`-be kerül összecsukható blokkként; a `Jegyzetek`
és `Előzmények` az Ops felületről is megnyitható panel. A szekciók akkor is megjelennek, ha
adata hiányzik — ilyenkor «Hiányzó» állapottal, nem átugrva (a hiány maga is információ).

| Szekció | Tartalom | Adatforrás | Minőségi jel |
|---|---|---|---|
| Hero | csapatok, címer, eredmény, félidő/ET/pens, kickoff, verseny, stadion, bíró, publish-gate | `GET /v1/match/{id}` | Megfigyelt / gate |
| Áttekintés | magyar preview + `key_points`, predikció (modell + `note`), sérülések, szezonstatisztika, fan-poll, forma, H2H, tabella | `insight.*`, `home_form`, `head_to_head`, `table_slice` | **Számított** (`generated`/`verified` mezők alapján), pillanatkép `snapshot_at`-tal |
| Események | meglévő `MatchTimeline` + kommentár, ha van | `incidents[]`, research `commentary` | Megfigyelt |
| Statisztika | `StatsDuel` + csapat- és játékosszintű tábla (szűrő: oldal, pozíció, minimum perc; rendezés: érték, név) | `fact.match_team_stats`, `fact.match_player_stats` | Megfigyelt + `captured_at` |
| Felállás | formáció, kezdők/cserék pályán, hiányzók és ok, edző, értékelés, kattintható játékos | `fact.match_lineups`, `insight.injuries`, research `managers` | Megfigyelt |
| Lövések | lövéstérkép (x, y, xG, xGoT), momentum, támadási zónák, kapusstatisztika | `fact.match_shots`, research `shotmap`/`momentum`/`goalkeeping` | Megfigyelt |
| Odds | kurált 1X2/BTTS/OU2.5 **+** összes piac szűrővel, irodánkénti bontás, `observed_at` | `odds{}`, `insight.odds_markets` | Megfigyelt, `odds_trust_state` |
| Források | forrás URL-ek, provenance, `captured_at`, hiányok/javítási akció, jegyzetek, előzmények | `links[]` (új), `/notes`, `/history`, `gate_*` | minden tételnél külön jel |

### 3.4 Verseny-, csapat- és játékosoldal

- **Verseny:** meglévő tabella bővítése formmal és hazai/vendég bontással; meccslista szűrővel
  (státusz, csapat, forduló) és rendezéssel; csapatsor kattintható; versenyszintű hiányjel és
  forráslinkek; szezonválasztó megőrzése.
- **Csapat:** keret (as-of `player_team_spell`), aliasok, identitásreview/összevonási előzmény,
  forráslinkek, hiányjelek, szűrhető meccslista; formameccsek kattinthatók (a meccsoldalon
  már azok); logófeltőltés csak későbbi, ops tokenes szeletben.
- **Játékos:** profil (pozíció, születés, állampolgárság), csapattagsági időszakok, meccs- és
  szezonstatisztika verseny/szezon szűrővel, események — **minden sor linkel** a meccsre,
  illetve a csapatra; a `prefix(10)`/`prefix(20)` vágás helyett lapozás/«több megjelenítése».

---

## 4. Szűrés, rendezés, megőrzés

- Mechanizmus: a meglévő `FilterBar` mintájára újrahasználható `FilterChip`/`FilterMenu`
  komponens; az állapot a `UserDefaults`-ba kerül (a liga-összecsukás és -kedvencek mintájára).
- Meccsoldal: odds-piac választó (piac + iroda), játékosstatisztika oldal/pozíció/perc szűrő,
  rendezés név/érték szerint.
- Verseny: státusz (Mind / Élő / Lezárt / Hiányos), csapat, forduló; rendezés dátum/eredmény/pont.
- Csapat: státusz, verseny, szezon; rendezés dátum/eredmény.
- Játékos: szezon, verseny, státusz; rendezés dátum/perc/érték.
- Keresés: meglévő `kind` szűrő megőrzése; találat kiválasztása után entitásra ugrás.
- Minden szűrőnek van alapértelmezése és „törlése”; az eredmények számát a szűrő mellett jelezzük.

---

## 5. Linkek és források kezelése

- **Belső linkek:** SwiftUI `NavigationLink`/`sheet`, az azonosítók a DTO-ból; nem kell új
  backend-Mezeő a meccs→csapat linkhez (`home_team_id`/`away_team_id` viszont **kell**, ma nincs
  a `MatchDetailRow`-ban). A játékoslinkek a felállás/játékosstat rekordokból jönnek.
- **Külső linkek:** kizárólag a forrásból származó, eltárolt URL (`match.links.source`,
  `source_urls`, `standings.url`, `links.reference`, irodai `sources`); új `fact.match_link`
  tábla + `links[]` mező a `GET /v1/match/{id}` válaszban. Swift oldalon `Link`/`NSWorkspace`,
  `http(s)` séma ellenőrzéssel; a felhasználó nem adhat meg URL-t. A link mellett mindig látszik
  a forrás neve és rögzítési ideje.
- **Titok/publikus repó:** az URL-ek nem tartalmazhatnak tokent/cookie-t/személyes adatot; a
  nyers research payload továbbra sem kerül a repóba (AGENTS.md §7). robots/policy és rate limit
  itt is érvényes: a link csak böngészőben nyit, a kliens nem scrape-el újra.
- **Hiány:** ha nincs tárolt URL, a „Forrás” sor «Nincs forráslink» szöveget mutat javítási
  akcióval, nem üres ikont.

---

## 6. Vizuális rendszer (a meglévő „éjféli stadion” kiterjesztése)

A színek, sugarak, tipográfia és rács változatlanok (`Theme.swift`: OLED-fekete, grafit
`#0E0E10`/`#16161A`, borostyán `#FFB020`, élő-zöld `#30D158`, piros `#FF453A`, 4 pt rács,
18/14/9 pt sugarak, `.scoreboard`, `.appText`, `.appLabel`). Csak **új komponensek** jönnek,
egységes fájlban (`apps/FootballApp/DataViews.swift`), a `Scoreboard.swift` mintájára:

| Komponens | Felelősség |
|---|---|
| `SectionTabBar` | szekciófülek + `ScrollViewReader` anchor navigáció, billentyűzetes roving focus |
| `TrustPill` | a 6 minőségi állapot szöveges + ikonos + tooltip jelölése |
| `SourceLine` | forrás név + `captured_at` + opcionális külső `Link` |
| `EntityLinkRow` | csapat/verseny/játékos/meccs sor kattinthatóan, AX-azonosítóval |
| `FilterChip` / `FilterMenu` | közös szűrővezérlők, kiválasztott állapot + törlés |
| `PlayerStatTable` | játékosmetrika tábla, rendezhető oszlopok, hiány «–» |
| `LineupPitch` | formáció és kezdők/cserék pályán, kattintható játékosok |
| `ShotMap` / `MomentumStrip` | lövéspontok xG-vel, momentum vonal, hiányállapot |
| `LinkList` / `NotesPanel` / `HistoryPanel` | források, jegyzetek, előzmények közös megjelenítése |
| `EmptySection` | szakaszszintű hiány + javítási akció (nem üres tér) |

Sűrűségi szintek: **kompakt** (napi lista kártyája — változatlan), **olvasható** (szekciók,
táblák), **térkép** (pálya/lövéstérkép). Tartalomszélesség 860 pt, ragadó hero; `reduceMotion`
minden animációnál; világos/sötét módban is AC-target. A szín önmagában soha nem hordozó
(spec §3.5).

---

## 7. API- és adatigény (a rések, amiket a terv kezel)

| # | Rés | Javasolt megoldás | Új séma? |
|---|---|---|---|
| A | Meccsoldalon nincs csapat ID | `MatchDetailRow` + `home_team_id`, `away_team_id` | nem |
| B | `match_report_hu` nincs betöltve (180/187 mintában) | `fact.match_insight` új `match_report_hu jsonb` oszlop + ingest + API; **Számított** jelöléssel | igen (`0029`) |
| C | Nincs forrás URL a kanonikus API-ban | `fact.match_link(match_id, kind, label, url, source_id, captured_at)` + ingest + `links[]` | igen (`0029`) |
| D | Nincs meccs-szintű játékos-/felállás-/lövésvégpont | `GET /v1/match/{id}/lineups`, `/player-stats`, `/shots` a meglévő `0009` táblákból | nem |
| E | Nincs verseny-/csapatlink és keret | `/v1/competition/{id}` és `/v1/team/{id}` bővítése (`players[]`, `aliases[]`, `links[]`, tabella-form) | nem |
| F | Jegyzetek/előzmények nincsenek a meccsoldalon | a meglévő `/notes` és `/history` végpontok bekötése (S6) | nem |
| G | UI-fixture szerver nem ismeri a verseny/csapat végpontokat | `ui-fixture-server.py` + új fixture-ök | nem |

Az új végpontok mind **read scope**-úak; írni továbbra is csak ops tokennel lehet
(`2026-11-26-app-oldalak-bovites.md` 1. döntése). Új végpontnál mindenhol: magyar 400/401/404,
OpenAPI-szinkron, közös kontrakt-fixture, Rust + Swift kétoldalas teszt.

---

## 8. Szeletek áttekintése (részletes terv: `../plans/2026-11-26-adatmegjelenites-fotmob.md`)

| # | Szelet | Fő érték | Írási hatókör |
|---|---|---|---|
| S0 | Döntések + worktree-egyeztetés (nem kód) | mi mehet be és mi nem | docs |
| S1 | Navigáció és meglévő adatok láthatóvá tétele | azonnali FotMob-szerű élmény, entitás linkek | `apps/`, kis API-mező |
| S2 | Forráslinkek és magyar riport | minden tény visszakövethető, kattintható forrás | `rust/`, `db/`, `apps/` |
| S3 | Meccs-szintű játékosadatok API | felállás/statisztika/lövések elérhetők | `rust/`, `tests/` |
| S4 | Felállás, játékosmetrika, lövéstérkép UI | mélység és szűrők | `apps/` |
| S5 | Verseny- és csapatoldal mélység | teljes entitásnézetek, linkek | `rust/`, `apps/` |
| S6 | Jegyzetek és előzmények a meccsoldalon | ügynök↔felhasználó együttműködés | `apps/` |
| S7 | Keresési entitásugrás, megőrzött szűrők, beállítások (opcionális) | 3 lépéses elérés, testreszabás | `apps/` |
| S8 | Logófeltöltés UI (a régi S5 szelet) | hiányzó címerek javíthatók | `rust/`, `apps/` |
| S9 | AI-asszisztencia (külön döntés, alapból ki) | javaslat-only segítség | `rust/`, `apps/` |

Minden szelet külön tervlépésekkel, kapu-lánccal és checkpoint-bejegyzéssel kerül végrehajtásra;
az írási hatókörök diszjunktak, a kaput független ellenőr futtatja (builder ≠ verifier).

---

## 9. Kockázatok

- **Túl sok egyszerre:** ez a design több nap munka; a szeleteket sorban, de nem egyetlen
  „nagy átállásként” kell vinni, és minden zöld kapu után checkpoint + commit.
- **Már meglevő, nem commitolt UI-work a worktree-kben:** a `feat/match-detail-navigation`
  munkafában szekciónavigáció és dátumformátum-javítás van nem commitolva; a
  `feat/estimated-match-clock`-ban becsült óra; a `feat/match-chain-compact-header`-ben további
  UI- és backend-módosítások. Ezeket **nem szabad felülírni vagy csendben átvenni** — külön
  döntés tárgya (S0).
- **API-kontrakt drift:** új mezőknél a `tests/fixtures/contract/*` kétoldalas teszt nélkül
  a Swift és a Rust csendben elszakadhat.
- **Nem ellenőrzött tartalom:** `preview_hu`, `prediction`, `match_report_hu` és a streakek
  `generated`/`verified` jelzőit a UI-n kötelezően meg kell jeleníteni; tényként való
  megjelenítésük tiltott.
- **Adatlefedettség:** nem minden meccshez van felállás/lövés (mintanag: 100/187, illetve
  42/187) — a szekcióknak hiányállapottal kell működniük; nem lehet „nincs adat” miatt
  üres kártya.
- **Teljesítmény:** a teljes `marketsRaw` és a játékosstatisztika nagy payload; a táblákat
  lapozni/összecsukni kell, az API p95 < 150 ms célját (spec §3.7) mérni kell.
- **Daemon-frissesség:** új API-végpont csak az `install-footballd-agent.sh` után él a kliens
  számára; a telepítés gateway-jóváhagyást igényelhet, ezt nem szabad megkerülni.
- **Licenc:** a research és odds adat csak a lokális API/UI-ban; nyers payload vagy licencelt
  export nem kerül a publikus repóba.

---

## 10. Nyitott döntések (ajánlott alapértelmezéssel)

1. **Worktree-k nem commitolt munkája:** a kis, tesztelt UI-darabok (szekciónavigáció, dátum-
   formátum, becsült óra) átvétele **azonosítás után, külön commitban**, vagy teljesen új
   implementáció `main`-en? *Ajánlás: átvenni a három kis darabot review után, a többit nem.*
2. **Külső forráslinkek:** szabad-e böngészőben nyitni az eltárolt FotMob/web URL-eket?
   *Ajánlás: igen, csak tárolt URL-ből, `http(s)`-re korlátozva.*
3. **`match_report_hu` és forráslinkek betöltése az S2-ben:** *Ajánlás: igen, `0029` migrációval,
   „Számított / nem ellenőrzött” jelöléssel.*
4. **Első kód-szelet:** S1 (client + csapat ID-k), vagy előbb az API-alap (S2+S3)?
   *Ajánlás: S1 — azonnal látható eredmény, minimális backend-kockázat.*
5. **Beállításoldal és AI-gomb:** *Ajánlás: beállítások az S7-ben opcionális; AI-gomb továbbra
   is alapból ki, külön döntés nélkül nem kapcsoljuk be.*
6. **Logófeltöltés:** az S8-at a korábbi döntés (ops token, WebP/PNG) szerint végezzük;
   SVG továbbra is külön biztonsági review tárgya.

---

## 11. Visszaállás

A dokumentum önmagában nem változtat kódot vagy adatot. A későbbi szeletek:
`git revert <szelet-commit>`; a `0029` migráció visszaállítása
`ALTER TABLE fact.match_insight DROP COLUMN match_report_hu; DROP TABLE fact.match_link;`
és a `_sqlx_migrations` sorának törlése — külön, egyeztetett lépés. UI-fixture-ök és
`UserDefaults`-beállások lokálisan visszaállíthatók; éles kanonikus adatot a terv nem ír át.
