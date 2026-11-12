# S1 záró review — felhasználói döntések (2026-11-12)

**Helyzet:** az S1 forráskutatás (W1–W4, Task 1–15) 2026-11-11-én lezárult;
a katalógus (`docs/research/source-catalog.md`) `## Döntési pontok` fejezete
16 pontot tett a felhasználói review-kapu elé. A felhasználó 2026-11-12-én
mind a 16 pontról egyesével döntött. Ez a dokumentum a döntések hivatalos
rögzítése (spec §3.1); a katalógus státuszai ez szerint frissülnek.

**Jelölés:** minden pontnál a döntés, az indoklás és az újrabírálás feltétele
(ha van). A spike-bizonyítékok a katalógus bejegyzéseiben és a task-reportokban
élnek; új hálózati kérés a review során NEM történt.

## 1. TheSportsDB — ELVETETT

A „3” kulcs 403 (auth-fal), a free „123” kulcs `eventsseason` limitje
15 esemény — gerinc-szerepre elégtelen. A gerincet a footballdata fedi,
a stadion/referee domain FotMob-ból igazoltan jön; nincs mért rés, amit
a TheSportsDB töltene be. Prémium előfizetés a demo-körhöz nem indokolt.
**Újrabírálás:** ha az adapter-szeletek stadion/referee rést mérnek.

## 2. Livescore MCP — ELVETETT

A livescoremcp.com robots.txt explicit tiltja a `/mcp`, `/sse`, `/message`,
`/health` útvonalakat, miközben a szolgáltató AI-ügynök MCP-végpontként
hirdeti az oldalt. A spike a robots-kapun StopSpike-ot kapott; megkerülés
NEM történt (a v1 `robots_override` anti-patternje nem kerül át). A gerinc-
és mélységi domainek a FotMob/ESPN/OpenFoot hármasból fedettek.
**Újrabírálás:** ha a szolgáltató dokumentált feltételekkel egyértelműen
engedélyezi az API-használatot (kapcsolatfelvétel most nem indul).

## 3. Logók/zászlók — A V1 RÉTEGES MEGOLDÁS AUDITÁLT ÁTVÉTELE

A v1 logó-megoldásának átvétele, a dokumentált hibák átnézésével:

1. **Zászlók:** lipis/flag-icons MIT pack (272 SVG, LICENSE megvan) —
   tiszta licenc, átvehető.
2. **Csapatlogók:** a v1 helyi asset-bázis (Sofascore-eredetű alaplogók +
   FM27 MegaPack kézi/review-olt párosításai) és manifestjeinek átvétele
   **lokális, nem publikus tárolóba** (a repó publikus — scrape-elt és
   FM-es kép nem kerülhet bele), sha256-provenance megőrzésével.
   A hozzárendelések **auditja kötelező** a v1 tanulságai szerint:
   egyleveles rövidítés-ütközés (A. Madrid / R. Madrid), városnév-alapú
   téves párosítás (Newcastle → Manchester United), token-audit ~50%-os
   tévedése — nincs néma automatizmus, bizonytalan pár review-sorba kerül
   (a v2 „automatikus fuzzy entitás-összevonás tilos” szabálya szerint).
3. **Liga-logók:** FotMob id-link út (a FotMob igazolt forrás).
4. **Új forráskutatás csak mért résekre** (pl. NB I/NB II, packből kimaradt
   klubok), és ott lefedettség-ellenőrzés kötelező.
5. Az FM27 MegaPack **csak helyben** marad, továbbterjesztés tilos.

A GitHub football-logos (license: null) és a Wikimedia imageinfo
(elvetett, robots) útvonalak ezzel kiváltásra kerülnek; a zászló-rés
a MIT packkal zárul. **Megvalósítás:** önálló logó/asset szeletben
(S2/S3 után), nem az S2 hatóköre.

## 4. Wikidata — NEM KELL MOST

A `www.wikidata.org/w/` robots-tiltott (spike-szintű elvetés). A crosswalk
részben fedett: reep strong rung + a v1 review-olt alias-táblák.
**Újrabírálás:** csak mért identitás-rés esetén (adapter-szeletek), akkor
a `query.wikidata.org` SPARQL (saját robots-ítélet) vagy a hivatalos CC0
dumpok alternatívája probe-olható.

## 5. OpenFoot — READ-ONLY FOLYTATÁS, KÖZÖS KULCS

A v1-gyel közös kulcs (Keychain: `openfoot-api-key`) read-only marad.
A server-side remaining az irányadó (élő mérés: starter plan, 5000/hó,
used 326 / remaining 4674; a v1 fix-log 459-es eltérése nem ellenőrzött).
A havi számlálást az **S2 worker-ledger** veszi át. Lineups/xG Developer
plan-gate (403) — upgrade NEM indokolt, a domain FotMob-ból jön.
**Kanonikus írás csak külön felhasználói döntéssel** (v1-incidens +
fail-closed identitás elve).

## 6. ClubElo — SAJÁT ELO-SZÁMÍTÁS

A tükör licencet nem deklarál, a CSV >5 MB (nem mért), a meccs-tábla
football-data származék (nem független keresztforrás). Az Elo a
predikciós fázisban **saját számítású** lesz a footballdata-eredményekből
(bootstrap nélkül, as-of szabályokkal); a provenance-címke (saját számítás,
bemenet forrása) kötelező. A tükör nem adoptálható. A séma-rés lista
18. tétele ennek megfelelően módosul (származtatott feature, nem
tükör-pillanatkép).

## 7. FM27 játékmentés-adat — KIHAGYÁS MOST

A mag-pipeline nem függ tőle; a játékos-mélység a FotMob/Transfermarkt
útvonalakon jön. A MegaPack logók helyi, display-only használata a
3. pont szerint külön él. **Újrabírálás:** ha a játékos-mélységnél mért
rés jelentkezik (helyi jelenlét ellenőrzése + `derived_opinion` címke
csak akkor, külön szeletben).

## 8. Guardian — ToS-REVIEW AZ ADOPTÁCIÓ ELŐTT

Technikailag próba-szinten igazolt (41 gu-island props eredmény, erősített
őrrel), de a ToS scrape-politikája nem ellenőrzött (BBC-precedens).
A státusz `halasztott` marad; a read-only ToS-review az adoptációs szelet
kapuja. **Tiltás esetén elvetés**; csak eredmény-keresztforrás szerepben
kerülhet szóba.

## 9. API-Football — ELVETETT

Host-szintű 403 (robots + web + docs) erről a kliensről; a spike
StopSpike, endpoint-bizonyíték nincs. A felhasználó döntése: elérhetőségi
próba és regisztráció NEM indul, a forrás elvetett. A demo-kör fedettsége
más forrásokból megvan (footballdata + OpenFoot + ESPN).

## 10. reep — TUDOMÁSULVÉTEL, STAMP-GUARD KÖVETELMÉNY

A `latest.json` stamp-regressziója (2026-10-19.1 < a v1 által mért
2026-10-31.1) nem elvetési ok. **S2 követelmény:** az adapter stamp-alapú,
idempotens kezelést kap — régebbi stamp nem ír felül újabbat, a regresszió
naplózott, a `matches.csv.gz` letöltését kiadói sha256 ellenőrzi; a >5 MB-os
cut streaminggel töltendő (adapter-szelet, a harness-plafon tudatos
kezelésével). Licenc: CC0, strong rung only.

## 11. worldfootball — KUPA-GERINC ÁTADVA A HÁRMASNAK

A spike-szintű 403-elvetés megmarad; a kupa-gerinc szerepét **végleg
a Sky/ESPN/thefa hármas** veszi át (13. pont). **Újrabírálás:** engedélyezett
szállítási újra-probe csak akkor, ha az UEFA-kupa archívum-mélység
(1955/56 óta) ténylegesen indokolttá válik.

## 12. Soccerway — TARTALÉK (OBSCURA-FÜGGŐ)

A `halasztott` státusz megmarad. Az obscurás útvonal igazolt (118 FA Cup-sor,
58 eventRowLink/mid, crosswalk exact-match-csel megvalósítható — fuzzy tilos).
**Aktiválás feltételei:** a v2 Obscura-integráció megépülte + a Sky/ESPN/thefa
hármas mért rése (korai selejtezőkörök) + az év/TZ/ToS/mid-stabilitás
kérdések tisztázása. Költség: ~13 s/oldal.

## 13. FA Kupa — SKY ELSŐDLEGES + ESPN KERESZT + THEFA KIEGÉSZÍTŐ

Mért fedettség: Sky 40 meccs (AET/pens explicit, ESPN↔Sky 4/4 kereszt),
thefa 20 jövőbeli fixture (results Opta-decoy → halasztott), ESPN 4 esemény.
**Felállás:** Sky elsődleges, ESPN keresztforrás, thefa hivatalos
fixture/round-dates kiegészítő. **90 perces konvenció-őr mindenhol
kötelező** (AET/pens skip; ET/pens csak explicit forrásból, külön státusz).
**Magyar Kupa:** FotMob (liga-ID 213) + Tipplap követendő.
A Sky/ESPN ToS-reviewk (15. pont, illetve a katalógus Sky-jegyzete)
külön kapuk maradnak.

## 14. football-data.org / The Odds API — REGISZTRÁCIÓ NEM KELL MOST

Kulcs nélküli spike: football-data.org 403, The Odds API 401; regisztráció
nem történt, kulcs nem készült. A kétforrásos odds-szabályt a footballdata
closing + Tipplap closing fedezi; az eredmény-gerinc igazolt.
**Újrabírálás:** csak mért keresztforrás-rés esetén, célzottan
(a kulcsos spike-hoz a Keychain-írást a felhasználó saját üzenetében
kell engedélyezni).

## 15. ESPN — ToS-REVIEW AZ ADOPTÁCIÓ ELŐTT

Technikailag igazolt (PL 5/5 eredmény-egyezés, linkelhetőség mérve:
4 alias-szal 100% offline plafon; AET/pen rés explicit őrrel kezelendő).
A ToS nincs ellenőrizve — a read-only ToS-review a 4–5. javasolt
adapter-szelet kapuja (Guardian-minta). **Tiltás esetén** a keresztforrás-
szerepet át kell adni, és a FA Kupa-hármas felállása újratárgyalandó.

## 16. Bizalmi mátrix + adoptációs sorrend — JÓVÁHAGYVA, S2 INDUL

A katalógus „Bizalmi mátrix” és „Adoptációs sorrend” fejezete a felhasználó
által jóváhagyva, az alábbi átvezetésekkel:

- **Elvetett:** TheSportsDB (1.), Livescore MCP (2.), API-Football (9.) —
  kikerülnek a feltételes ágról; worldfootball (11.) elvetett marad,
  kupa-szerep nélkül.
- **Kiváltva:** logos-github / logos-wikimedia → a v1 logó-megoldás
  auditált átvétele (3.); ClubElo tükör → saját Elo-számítás (6.).
- **Kihagyva/halasztva:** FM27 játékmentés-adat (7.), Wikidata (4.),
  football-data.org / The Odds API regisztráció (14.) — mért rés esetén
  újra elővehetők.
- **Kapuk:** Guardian + ESPN ToS-review (8., 15.); Soccerway tartalék-
  aktiválás feltételei (12.); OpenFoot read-only + worker-ledger (5.);
  reep stamp-guard (10.).
- **FA Kupa felállás:** Sky + ESPN + thefa (13.), 90 perces skip-őrrel.

Az **S2 spec ciklus (brainstorming → spec → terv) elindul** a jóváhagyott
mátrixszal és a 18 tételes séma-rés listával mint bemenettel. Hatókör:
ingest keretrendszer — Obscura-integráció, worker kvóta/breaker/ledger,
bizalmi szintes gate (a meglévő gate backward kompatibilis bővítése),
fail-closed identitás. A séma-rés lista 18. tétele a 6. pont szerint
módosul (saját Elo-feature).

## Következő lépések

1. A katalógus státuszai e dokumentum szerint frissülnek (élő dokumentum).
2. S2 brainstorming → spec (`docs/superpowers/specs/`) → terv
   (`docs/superpowers/plans/`), külön ciklusban.
3. A logó/asset átvétel (3. pont) önálló szeletet kap az S2/S3 után.
4. Commit/push csak kifejezett felhasználói kérésre.
