# 2026-11-28 — Meccsrészletező mélység — megvalósítási terv

**Spec:** [`2026-11-28-meccs-elonezet-melyseg-design.md`](../specs/2026-11-28-meccs-elonezet-melyseg-design.md)
**Kiindulás:** `feat/homepage-preview` @ `c0fd3f2` (S220: jobb panel kész).
**Státusz:** JÓVÁHAGYÁSRA VÁR — a felhasználó kérésére először csak terv, kód nélkül.
**Branch-stratégia:** minden S-szelet külön `feat/...` branch + külön commit;
zöld kapu után merge `feat/homepage-preview`-be. A felső fejléc, a szűrők és
a bal lista erre a tervre **nem** írhatók át.

---

## 0. Közös kapu és írási szabály

Minden szelet végén:

```bash
# Rust érintése esetén
cd rust && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
# Swift
cd apps && xcodegen generate && xcodebuild test -scheme FootballApp -only-testing:FootballAppTests
# Valódi gép
./scripts/install-football-app.sh && open -a FootballApp
swift scripts/ax-probe.swift FootballApp 'match.detail.block'
./scripts/app-window-shot.sh /tmp/fb-<szelet>.png
```

A teljes `run-ui-tests.sh` **nem** fut rutinban (15 perc, blokkolja a gépet).
Minden UI-szöveget magyarul visszaolvasunk képről/AX-ből. Új adatot nem
találunk ki: minden mező mögött konkrét forrásmező vagy számolt képlet áll,
`generated_at`/`captured_at` és hiányjelöléssel.

---

## 1. S221 — Adatszerződés: a meglévő adatok elérhetővé tétele

**Cél:** a kliens megkapja mindazokat a mezőket, amik a DB-ben már megvannak,
de a DTO-kban vagy a Swift dekódolásban hiányoznak. **UI még nem változik.**

**Írási hatókör:** `rust/crates/api/**`, `rust/crates/storage/**`,
`apps/FootballAppCore/**`, `tests/fixtures/contract/**`, checkpoint.

- [ ] **Season stat teljes dekódolása:** a Swift `InsightSeasonStat`
      (`apps/FootballAppCore` / `MatchInsightModels.swift` ~194–216) vegye fel
      a `clean_sheets`, `assists`, `possession_pct`, `average_rating`,
      `goal_difference`, `points`, `rank_label`, `source` mezőket — mind
      `Optional`, alapérték `nil`. A JSONB forrás már tartalmazza őket
      (`fact.match_insight.season_stats`); a hiány maradjon `nil`.
- [ ] **Következő meccsek logói:** `NextMatchEntry`
      (`storage/repositories/match_detail.rs:860-866`) bővülése
      `home_logo`/`away_logo` mezőkkel ugyanazzal az `ops.asset` lekéréssel,
      mint a `MatchDetailRow` `home_logo`/`away_logo` mezője; hiány `null`.
      Swift DTO + kontrakt-fixture szinkron.
- [ ] **H2H kiegészítő statisztika:** `HeadToHeadEntry`
      (`match_detail.rs:609-617`) opcionális `home_corners/away_corners`,
      `home_yellow/away_yellow`, `home_red/away_red`, `home_xg/away_xg`
      mezőkkel; a lekérés `LEFT JOIN fact.match_stats`-ből, a H2H `match_id`
      alapján. A `fetch_head_to_head` adja vissza a `coverage` számot is
      (hány H2H meccshez volt statisztika).
- [ ] **Számolt H2H összesítő (backend, read-only):** `h2h_summary` a
      `MatchDetailRow`-ban: `matches` (darabszám), `goals_for/against`,
      `btts_pct`, `over25_pct`, `corners_avg`, `cards_avg`, `with_stats`
      — minden érték `null`, ha nincs elég alap (`with_stats = 0`). Képletek:
      BTTS és over/under a kanonikus gólokból, szöglet/lap a csatlakozó
      `fact.match_stats`-ból. **Nincs új tábla, nincs írás.**
- [ ] **Kontrakt-tesztek:** `GET /v1/match/{id}` kulcsszáma régi kulcsok
      megőrzésével nő; Rust round-trip + Swift `DetailContractTests` zöld;
      OpenAPI séma automatikusan követi a `ToSchema`-t.
- [ ] **Mérés:** read-only SQL a fedésről (`fact.match_stats` 21 473/55 041
      meccs; a Leeds–Coventry H2H mintában 2/2) — számok a checkpointbe.

**Kapu:** Rust workspace zöld, kontrakt zöld, `integrity` OK, daemon
újratelepítve. UI kép nem kell.

---

## 2. S222 — Bevezető, sérülések, kulcsjátékosok

**Cél:** a pre-match legemberközelebbi részei a panel tetején, mindkét
csapatra külön oszlopban.

**Írási hatókör:** `apps/FootballApp/**`, `apps/FootballAppTests/**`.

- [ ] **0. szakasz — `IntroBlock`:** `insight.preview_hu.title`,
      `key_points`, `sections[].heading/text`. Ha nincs preview: ténysor
      (verseny · forduló · kezdés · státusz) és egy mondat:
      „A forrás nem adott szöveges bevezetőt." A generált jelölés
      (`generated`, `source`, `generated_at`) **mindig** látszik
      („Számított szöveg — nem ellenőrzött tény").
- [ ] **A Research szakaszból kikerül** a duplikált preview; ott csak a
      `match_report_hu` és a forráslinkek maradnak (ugyanazzal a jelzéssel).
- [ ] **2. szakasz — `AvailabilityBlock`:** két oszlop, csapatlogóval.
      `insight.injuries[].name/type/expected_return` — **nem** csak 6 sor,
      hanem az összes; ha 6-nál több, „Többi sérült" kinyitó gomb.
      Hiány: „Nincs rögzített sérülés a forrásban" (ez nem azt jelenti,
      hogy nobody is sérült).
- [ ] **Kulcsjátékosok:** `/lineups` és `/player-stats` alapján csapatonként
      a 3 legmagasabb értékelésű játékos (mező: `rating` / `FotMob rating`),
      szereppel (`role`: kezdő/csere) és a rögzített gólok számával.
      Címke: „Forrás szerinti szerep — nem megerősített kezdő".
      **Nincs szezonális góllövőlista** — ha a felhasználó ezt kéri, a
      kártyán rövid magyarázat: „A forrás csak meccs-szintű gólokat ad."
- [ ] **Üres/hiba/betöltés állapot** minden blokkhoz; AX:
      `match.detail.block.intro`, `.availability`, magyar `accessibilityLabel`.
- [ ] **Teszt:** 3 app unit (preview jelölés, sérülések darabszáma, szerep-
      háromállapot) + valódi macOS kép egy preview-val és egy preview nélküli
      meccsről.

---

## 3. S223 — Szezonösszehasonlítás

**Cél:** a két csapat szezonmutatói egymás mellett, pontosan a felhasználó
által felsorolt mezőkkel — de **csak akkor**, ha a forrás tényleg adta őket.

- [ ] `SeasonComparisonBlock`: kétoszlopos tábla (`home`/`away`), sorok:
      pozíció/rank (`rank_label`, pl. „8/20"), lejátszott meccs, győzelem/
      döntetlen/vereség, GF/GA, gól/meccs (`goals_per_game`), kapott
      gól/meccs, pont. További sorok **csak nem-null értékekkel**:
      clean sheets, asszisztok, labdabirtoklás, átlagértékelés.
- [ ] Az átlagértékelés címkéje a `source` mezőből jön (pl. „fotmob_table
      alapján"); **sehol nem írjuk Sofascore-nak** — tiltott forrás, és a
      mérés szerint 22/27 035 a kitöltöttség.
- [ ] Ha a mező hiányzik: „—" és tooltip „Nincs adat a forrásban" — **nem 0**.
- [ ] A `table_slice` pozíciója és az insight `standing` pillanatképe
      közötti eltérést nem simítjuk át: a blokk láblécében szerepel a
      `generated_at` / `snapshot_at`.
- [ ] Teszt: minden mezőre két eset (érték / null); képkocka két csapatról.

---

## 4. S224 — H2H és eddigi meccsek szétválasztása, számolt metrikák

**Cél:** „külön vizsgalva az egymas elleni meccseket, kulon az eddigieket".

- [ ] `HeadToHeadBlock` (4a): meccsenkénti sor — dátum, verseny, két logó,
      eredmény, H/V; kattintás → láncolt meccsrészlet. Felett összesítő
      csíkok: összes gól, BTTS%, over/under 2.5, szögletátlag, lapátlag,
      **mindegyik mellett `with_stats / matches` coverage** (pl. „2/5 meccs
      alapján"). A backend `h2h_summary` mezőjét rajzolja, ha `null`, akkor
      „Nincs elég rögzített adat az összesítéshez".
- [ ] `RecentFormBlock` (4b): `home_form` és `away_form` **külön**, nem a H2H
      részeként; eredmény, ellenfél, verseny, H/V, logó (ha van).
- [ ] A research `h2h_streaks` két számlálóját (`no_win`, `no_clean_sheet`)
      külön kis kártyaként, „streak — számított" jelöléssel.
- [ ] Magyarázó szöveg a panelen: „A szöglet- és lapmutatók csak azokból az
      egymás elleni meccsekből számolnak, amelyekhez van rögzített
      meccsstatisztika."
- [ ] Teszt: BTTS/OU számolás 3 gólkombinációra, coverage 0 esetén `null`,
      H2H és forma nem keveredik (AX-fa alapján).

---

## 5. S225 — Következő meccsek logóval, tabella

- [ ] `NextMatchesBlock`: `home_next`/`away_next`, két csoport, **mindkét
      csapat logójával** (S221 DTO), H/V, dátum, verseny; kattintás → láncolt
      meccs. Régi szöveges megjelenítés kikerül.
- [ ] `StandingsSlice`: meglévő tabella marad; kis javítás: a kijelölt
      csapat sora kiemelt, a `table_slice` `generated_at`-je látható.
- [ ] Teszt: logó URL hiányzik → placeholder, nem törött kép; AX-szöveg
      „nincs logó" nem „ismeretlen csapat".

---

## 6. S226 — Odds és a saját predikció

- [ ] `OddsBlock` átdolgozása: kurált 1X2/BTTS/OU kártya + a `/markets`
      részletes piacai 10 csoporttal és „Többi piac" gombbal, bookmaker és
      `captured_at` oszloppal, `odds_trust_state` jelzéssel. A régi
      `MarketOddsList` komponens újrahasználható, nem kell új API.
- [ ] `PredictionBlock` kiemelten (az odds alatt, saját kártya):
      `/prediction` sorai (`model_version`, `probability`, `fair_odds`,
      `edge`, `quality`, `as_of`) + research `insight.prediction`
      (`expected_goals`, `win_probability_pct`, `most_likely_score`,
      `inputs`, `note`). A kettő forrás **nem** keveredik jelzés nélkül:
      külön sor „Shadow modell" és „Research predikció (generated)".
- [ ] A „**NEM ELLENŐRZÖTT modell**" jelölés a kártya keretén és az
      AX-címkében is megjelenik; nincs fogadási call-to-action, nincs
      „biztos tipp" szöveg.
- [ ] A `PredictionCard` meglévő AX-azonosítói (`match.prediction`) maradnak.

---

## 7. S227 — Integráció, magyar szövegek, kapu

- [ ] A blokkok rögzített sorrendje a spec 3. táblázata szerint; minden blokk
      `match.detail.block.<name>` AX-azonosítót kap; a régi szekciófülek
      továbbra is ugrani tudnak rájuk (`ScrollViewReader` anchor).
- [ ] Magyar szögek visszaolvasása képről: minden címke, hiányüzenet,
      jelölés; CJK/replacement karakter 0.
- [ ] `reduceMotion`, világos/sötét mód, 25/75 arány és fejléc érintetlen.
- [ ] Teljes körű ellenőrzés a §0 kapu szerint; végén teljes, magyar
      zárójelentés a SZELET/VÁLTOZOTT/FUTOTT/NEM ELLENŐRZÖTT/KOCKÁZAT/
      VISSZAÁLLÁS formátumban.

---

## 8. Kockázatok és visszaállás

- **Alacsony fedés:** preview 88/27 035, sérülés 2 881/27 035, average
      rating 22/27 035 — a legtöbb meccsen a panel üres blokkokat fog
      mutatni. Ez erény: a hiány látható, nem hamis adat. Ha a felhasználó
      ezt nem akarja, új forrás bekötése külön döntés (Sofascore **nem**).
- **Teljesítmény:** a `/markets` és `/prediction` külön hívások; a panel
      `.task(id:)`-ben párhuzamosan tölt, egyik hiba sem blokkolja a
      többit. Nagy piacoknál a 10 csoportos korlát marad.
- **Duplikáció:** a preview és a sérülések jelenleg a Research szakaszban
      vannak — az S222 áthelyezi, nem duplázza.
- **Visszaállás:** minden szelet `git revert`-tel; a DTO-bővítések opcionális
      mezők, a régi mezők változatlanok. Adatot a terv nem ír.
