# Adatmegjelenítési döntések (2026-11-26)

**Helyzet:** a `2026-11-26-adatmegjelenites-fotmob-design.md` §10-es nyitott pontjaira
a felhasználó 2026-11-26-on mind az ajánlott alapértelmezést választotta. A döntések
az S0-tól érvényesek; a megvalósítás a `2026-11-26-adatmegjelenites-fotmob.md` terv szerint,
szeletenként és zöld kapu utáni commitkal történik.

## Döntések

1. **Worktree-k nem commitolt munkája:** a kis, tesztelt UI-darabok — a
   `feat/match-detail-navigation` szekciónavigációja + `mm`→`MM` dátumjavítása +
   `marketsRaw`-védelem, és a `feat/estimated-match-clock` becsült órája —
   **review után, külön commitban átkerülnek `main`-re**. A
   `feat/match-chain-compact-header` további UI- és backend-módosításait **nem** vesszük át
   (az S162-es döntés változatlanul él). A worktree-ket nem írjuk, nem töröljük.
2. **Első kód-szelet:** **S1** — navigáció és a meglévő adatok láthatóvá tétele
   (szekciófülek, megbízhatósági jelölés, dekódolt de ki nem rajzolt insight-részek,
   entitáslinkek, `home_team_id`/`away_team_id` az API-ban). Az API-alap (S2+S3) utána jön.
3. **Külső forráslinkek:** **engedélyezett**, de kizárólag a forrásból eltárolt `http(s)`
   URL-ből; a kliens nem scrape-el újra, a felhasználó nem ad meg URL-t, token/cookie nem
   kerül a linkbe (AGENTS.md §7).
4. **`match_report_hu` és forráslinkek:** az **S2** mindkettőt betölti az új `0029` migrációval
   (`fact.match_insight.match_report_hu` + `fact.match_link`); a riport „Számított / nem
   ellenőrzött” jelöléssel jelenik meg.
5. **Beállítások és AI:** a beállításoldal opcionális S7; az AI-gomb továbbra is **alapból ki**,
   bekapcsolása külön döntés.
6. **Logófeltöltés:** az S8-ban, a korábbi döntés szerint ops tokennel és csak WebP/PNG-
   formátumban; SVG továbbra is külön biztonsági review tárgya.

## Következmények

- A spec státusza VÁZLAT → JÓVÁHAGYOTT; a terv S0 lépései lezárhatók.
- Az S1 írási hatóköre: `apps/**`, `rust/crates/{api,storage}/**`, `tests/fixtures/**`,
  dokumentáció. Az átvett worktree-darabok Review + külön commit, és a teljes kapu-lánc
  újrafuttatása rájuk is vonatkozik.
- A döntések önmagukban nem változtatnak kódot, sémát vagy éles adatot.
