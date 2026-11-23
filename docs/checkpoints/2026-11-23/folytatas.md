# Folytatási jegyzet — 2026-11-23 záró állapot

Ez a fájl a holnapi (vagy későbbi) folytatás egyetlen belépőpontja. A napi események a `checkpoint.md`-ben vannak; itt csak az összegzés, a hézagok és a teendők.

## Hol tartunk

- **S113 — meccsrészletező gazdagítása research adatokkal: KÉSZ.** Commit `acfa435`; spec `docs/superpowers/specs/2026-11-23-match-insight-design.md`, terv `docs/superpowers/plans/2026-11-23-match-insight.md`. Teljes kapu-lánc zöld (581 Rust teszt, 125 Core teszt, UI TEST SUCCEEDED, daily-check 0).
- **S114 — historikus backfill research-szel: FOLYAMATBAN.** Pilot (11-12) + 4 teljes nap (11-11, 11-10, 11-09, 11-08) végigment a v2-be. A **11-07 backfill a zárás után magától befejeződött, zöld**: 680 meccs / 680 detail, verify + deep_verify 0 hiba / 0 figyelmeztetés, odds 69 (ebből **66 football-data a v1-ből**). Csak a research könyvtárba írt — **v2-be még nem került be**.
- Döntés: **v1 teljes import NEM kell** — a v1 `match_odds` tábláját (3,82 M football-data sor, 2004-től) a `tools/odds.py` közvetlenül olvassa. A v1 marad kereszt-ellenőrzésre / hiánypótlásra.
- Döntés: **nem készül Sofascore adapter** — a teljes ~818 meccses lista amatőr/ifjúsági farka alacsony értékű és túlterhelést jelentene. A teljes lista referenciaként elmentve: `~/Library/Application Support/FootballResearch/docs/sofascore-snapshots/2026-11-12.txt` + `2026-11-12-comparison.md`.

## Mit sikerült elérni — adatmennyiség naponta

| Nap | Research meccs | V2 meccs | Insight sor | Játékos | Stat sor | Odds (ebből v1 football-data) | Integrity |
|---|---:|---:|---:|---:|---:|---:|---|
| 11-12 | 122 | 116 | 59 | 419 | 25 821 | 2 (0) | OK |
| 11-11 | 77 | 94 | 46 | 470 | 11 910 | 22 (0) | OK |
| 11-10 | 92 | 91 | 71 | 688 | 23 669 | 0 (0) | OK |
| 11-09 | 87 | 84 | 63 | — | — | 5 (2) | OK |
| 11-08 | 572 | 557 | 482 | 4 330 | 227 215 | 64 (50) | OK |
| 11-07 | 680 | — | — | — | — | 69 (66) | backfill zöld, v2-lánc hátra |
| **Össz.** | **1 630** | **892** | **721** | **~6 000** | **~290 k** | **162 (118)** | **5/5 OK + 1 backfill zöld** |

Minden napon: `backfill` → verify + deep_verify **0 hiba / 0 figyelmeztetés**, `api-export`, discovery job (worker, 1 kísérlet, `last_error` üres), `match-insight-import --apply`, `player-import --apply`, `integrity` verdict **OK**, 0 failed job, `coverage-day`.

A napi kapu pontos parancsai (egy napra, `DATE=2026-11-08` példa):

```bash
R="$HOME/Library/Application Support/FootballResearch"
cd "$R" && python3 tools/football.py backfill --from $DATE --to $DATE --order desc --limit 1
cd "$R" && python3 docs/api-export.py $DATE
cd "$REPO" && ./scripts/research-sync.sh $DATE --enqueue-only   # worker ~1-3 perc, tick 60 mp
cd "$REPO" && set -a && source .env && set +a
rust/target/debug/footballctl match-insight-import --from $DATE --to $DATE --apply
rust/target/debug/footballctl player-import --from $DATE --to $DATE --apply
rust/target/debug/footballctl integrity --from $DATE --to $DATE
rust/target/debug/footballctl coverage-day --from $DATE --to $DATE
```

## Mi a hézag — mit kell kivizsgálni tesztekkel

1. **November 9 — a fő kivizsgálnivaló.** A backfill 87 meccset talált, de csak **84 került** a DB-be, és **63 insight sor** lett (a 46–71-es napi tartomány alatt). Az import két meccset kihagyott ezzel az üzenettel: `nincs kanonikus meccs-ref` — `fm:1000014221` és `fm:1000023069`. Ez azt jelenti, hogy a research dokumentumban szereplő `fm:` source-ref nem kötődik kanonikus meccshez. **Teszttel kell reprodukálni**: olvasd be a két `api/v1/matches/2026/11/09/*_fm*.json` dokumentumot, ellenőrizd a `match.source_ids` / `uid` mezőket, és a `canonicalize` ref-feloldási útját. Kérdés: hiányzik a ref a forrásból, vagy a canonicalizer nem ismeri fel? A javítás csak teszt zöldje után.
2. **Feloldatlan csapatok:** 11-12-n 6, 11-08-on **15 meccs** maradt ki a DB-ből `feloldatlan csapat` miatt (l. `footballd.log`). Megnézni, hogy `create-on-miss` vagy team-alias kérdése-e; ezek arányát a coverage mutatja (557/572).
3. **Odds-lefedettség dátumfüggő:** 11-10-en 0, 11-08-on 64 (50 a v1-ből). Ez **adatelérhetőség**, nem pipeline-hiba, de mérni kell naponta, és a `coverage-day` odds oszlopát figyelni. A v1 oddst már most használjuk — nem kell import.
4. **`preview_hu` / `prediction`:** csak `scheduled` meccsekre készül (`api-export.py:1376,1380`), ezért a lejátszott napokon 0. Helyette `match_report_hu` van 121/122-n, de **nincs** a Rust `SECTIONS` listában (`research_insight.rs:15-25`) és a Swift UI-ban sem. Kis kód-szelet: hozzáadni az insighthoz „Számított / nem ellenőrzött" jelöléssel. (A user még nem kérte kifejezetten — nyitott döntés.)
5. **Sofascore fedettségi rés:** 11-12-re 360 verseny/818 meccs vs research 40/122; kimaradt 344 verseny/729 meccs. A 11-08-hoz hasonló napokon a research sokkal közelebb jár (572 meccs) — a rés **dátumfüggő**, nem strukturális. Ha pontos mérés kell, új Sofascore snapshot + comparison a `docs/sofascore-snapshots/` alá (l. meglévő script-mintát a checkpointban).
6. **Integrity `függő review`:** 11-08-on 26 (11-12-n 5). Valószínűleg a nagyobb meccsszám arányos következménye, de érdemes megnézni, van-e köztük a backfillből származó.
7. **Lemez:** 2,3 GiB szabad a záráskor. Nagy napok után `rust/target/debug/incremental` törlése (generált cache, adat soha).

## Folytatási lépések sorban

1. **11-07**: a backfill már zöld (l. fent) — csak a v2-lánc van hátra: `api-export 2026-11-07` → `research-sync.sh 2026-11-07 --enqueue-only` → `match-insight-import` + `player-import --apply` → `integrity` + `coverage-day`.
2. **11-06, 11-05, …** sorban, naponta egy kapuval. Nagy napoknál (FotMob 150+ liga) a worker 2-3 percet is igénybe vehet — a `junction` tick 60 mp.
3. **Nov 9-i kivizsgálás** tesztekkel (l. fent 1.), külön szeletként, a napi backfill mellé nem keverve.
4. Csak ezután: `match_report_hu` szelet, ha a user kéri.

## Visszaállás

- V2 oldal: a betöltött sorok `ops.source` = `research` + `captured_at` / `ops.job.time_window=<nap>` alapján törölhetők; `git revert` a docs commithoz.
- Research oldal: a `backfill-state.json` `done` listájából a nap kivétele + az adott `2026/11/NN` és `api/v1/**/<nap>` újragenerálható. A Sofascore snapshotok git-en kívüli referenciák, törlésük nem befolyásolja a pipeline-t.
- A 0027 migráció (S113) visszaállítása: `DROP TABLE fact.match_insight;` + a migrációs sor törlése.
