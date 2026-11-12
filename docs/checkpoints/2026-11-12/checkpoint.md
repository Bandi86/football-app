# 2026-11-12 — Napi checkpoint

**Állapot:** review-kapu lezárva (16/16 döntés), rögzítés kész; S2 brainstorming indul
**Előző nap:** 2026-11-11
**Kapcsolódó dokumentumok:** [S1 döntések](../../decisions/2026-11-12-s1-review-dontesek.md),
[forráskatalógus](../../research/source-catalog.md),
[S1 spec](../../superpowers/specs/2026-11-11-real-data-sources-research-design.md)
**Branch:** `feat/source-research` (a review előtt tiszta, commitolva: 10e4a11)

## Napi cél

Az S1 záró review-kapu lezárása: a katalógus 16 döntési pontjának
egyesével történő megválaszolása a felhasználóval, a döntések rögzítése
`docs/decisions/`-be, a katalógus státuszainak átvezetése, majd az
S2 spec ciklus indítása (a 16. pont jóváhagyása szerint).

## Kiinduló állapot

- [x] S1 (W1–W4, Task 1–15) lezárva, commitolva + pusholva (10e4a11);
      a munkafa tiszta volt (`git status --short --branch`).
- [x] A review-kapu nyitott: bizalmi mátrix + adoptációs sorrend
      jóváhagyásra, 16 döntési pont megválaszolatlan.
- [x] Kötelező olvasási sorrend felvéve: AGENTS.md, README, S1 spec/terv,
      2026-11-11 checkpoint, git állapot.

## Elvégzett szelet: 16 döntési pont + rögzítés

- [x] Mind a 16 pont egyesével végigjárva a felhasználóval (helyzet +
      opciók + javaslat minden pontnál; a felhasználó döntött).
      Eredmények röviden:
      1. TheSportsDB elvetett; 2. Livescore MCP elvetett;
      3. logók: a v1 réteges megoldás auditált átvétele (MIT zászlók,
      v1 asset-bázis lokális tárolóba + audit, FotMob liga-logók,
      új forrás csak mért résekre); 4. Wikidata nem kell most;
      5. OpenFoot read-only folytatás (worker-ledger az S2-ben);
      6. ClubElo helyett saját Elo-számítás; 7. FM27 adat kihagyás;
      8. Guardian ToS-review kapu; 9. API-Football elvetett;
      10. reep stamp-guard követelmény; 11. worldfootball kupa-szerep
      átadva a Sky/ESPN/thefa hármasnak; 12. Soccerway tartalék;
      13. FA Kupa: Sky + ESPN + thefa, 90 perces skip-őr;
      14. football-data.org / The Odds API regisztráció nem kell most;
      15. ESPN ToS-review kapu; 16. mátrix + sorrend jóváhagyva, S2 indul.
- [x] A 3. ponthoz a v1 logó-megoldás read-only átnézése
      (`../football-app`: logo-audit, migrációs riport, manifestek,
      flag-icons MIT LICENSE) — a döntés ezt támasztja alá.
- [x] Döntési dokumentum: `docs/decisions/2026-11-12-s1-review-dontesek.md`
      (16 pont, indoklások, újrabírálási feltételek).
- [x] Katalógus-frissítés: 5 bejegyzés státusz-sora frissítve
      (thesportsdb, logos-github, livescore-mcp, clubelo, api-football →
      `elvetett`, döntés-hivatkozással) + új „Felhasználói döntések
      (2026-11-12, S1 záró review)” fejezet a 16 pont átvezetésével.
      A „Döntési pontok” fejezet történeti előkészítés maradt (nem szerkesztett).

## Futtatott ellenőrzések

- [x] Magyar visszaolvasás (Read tool, szó szintű): döntési dokumentum
      teljes egészében + a katalógus új fejezete — ékezetek épek,
      hiányzó szóköz / idegen szó nem fordul elő.
- [x] Idegen-glyph scan (python3, nem-ASCII karakterek): döntési
      dokumentum 0 találat; katalógus 6 találata mind korábbi, szándékos
      jelölés (Køge, CÍMKE−1, ©, ≈, 90’, ≠) — az új szakaszban 0.
- [x] Záró újsor: döntési dokumentum (pótolva) és katalógus OK.
- [x] `git status --short --branch` a rögzítés után: új fájl
      `docs/decisions/2026-11-12-s1-review-dontesek.md`, módosult
      `docs/research/source-catalog.md` + ez a checkpoint.

## Ellenőrizetlen / nem történt

- Új hálózati kérés NEM történt (a review a meglévő spike-bizonyítékokra épült).
- DB-írás NEM történt; kód NEM módosult; tesztek nem futottak (csak dokumentum).
- Commit NEM készült (felhasználói engedélyhez kötött).
- A ToS-reviewk (Guardian, ESPN) és a logó/asset átvétel NEM indult el —
      külön szeletek kapui.

## Következő döntési pont

1. **S2 brainstorming → spec → terv** (a 16. pont szerint jóváhagyott
   indítás): ingest keretrendszer — Obscura-integráció, worker
   kvóta/breaker/ledger, bizalmi szintes gate, fail-closed identitás;
   bemenet: jóváhagyott mátrix + adoptációs sorrend + 18 tételes
   séma-rés lista (a 18. tétel saját Elo-feature-re módosul).
2. Commit engedélyezése a mai dokumentum-változásokra (külön kérésre).

## Visszaállítási út

- Új fájl: `docs/decisions/2026-11-12-s1-review-dontesek.md` — törlése
  csak kifejezett engedéllyel.
- Módosult: `docs/research/source-catalog.md` (5 státusz-sor + új fejezet) —
  a review előtti állapot a git munkafa-diffből visszaállítható
  (`git restore` csak kifejezett engedéllyel).
- DB és /tmp/s1 érintetlen.

## S2a — spec + megvalósítási terv (2026-11-12, brainstorming → writing-plans)

- [x] S2 dekompozíció jóváhagyva: S2a (keretrendszer), S2b (bizalmi gate +
      identitás), S2c (Obscura), S2d (séma-rések) — mindegyik saját ciklus.
- [x] S2a brainstorming: 3 tisztázó kérdés (tiszta keretrendszer; worker a
      footballd-ben; teljes §7.3 eseménymodell) + 3 megközelítés (A: trait +
      worker crate — jóváhagyva) + 5 design szekció egyenként jóváhagyva.
- [x] Spec: `docs/superpowers/specs/2026-11-12-s2a-ingest-framework-design.md`
      — felhasználói review: „Spec jó". Önreview: 2 javítás (ops.job
      time_window, async-trait függőség).
- [x] Terv: `docs/superpowers/plans/2026-11-12-s2a-ingest-framework.md`
      — 12 task, TDD-lépések, valódi kód, commit nélkül (AGENTS §9).
      Önreview-javítások: DemoAdapter TZ-kezelés, `runner` modulnév
      (a `loop` foglalt szó), reserve-elszámolás egyértelműsítése,
      `url` függőség, fixture-útvonalak a CARGO_MANIFEST_DIR konvencióra.
- [x] Magyar visszaolvasás + glyph-scan: spec és terv tiszta (a terv
      szkennjén 0 idegen glyph; a diagram-karakterek szándékosak),
      záró újsor mindkettőn megvan.
- [ ] Végrehajtás: NEM indult — a felhasználó a végrehajtási módról
      (subagent-driven vs inline) és a `feat/s2a-ingest-framework`
      branch létrehozásáról még nem döntött.

Következő döntési pont: végrehajtási mód + branch-engedély; utána Task 1
(domain típusok) indul, taskonkénti STOP-pontokkal. Commit nem készült.
