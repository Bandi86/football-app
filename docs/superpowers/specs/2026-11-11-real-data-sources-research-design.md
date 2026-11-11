# Valós adatforrások kutatása és katalógusa — design (S1)

**Dátum:** 2026-11-11
**Státusz:** design szekciók egyeztetve (1–4 + bővítések); spec review előtt
**Branch:** `feat/real-data-wiring` (a kutatás új branchen folytatódik, külön engedéllyel)
**Kapcsolódó:** [Valós adat bekötése terv](../plans/2026-11-11-real-data-wiring.md),
[rendszertervezési specifikáció](2026-11-09-football-app-v2-design.md)

## 1. Keret és hatókör

A v2 főoldala már valós API-adatból dolgozik (0003 migráció, `/v1/day` bővítés, demo fixture-pár).
A következő fázis a **valós adatforrások hibrid bekötése**: ami scraping, ami API. A rendszer
túl nagy egy specnek, ezért önálló alszeletekre bomlik; ez a spec az **S1 (forráskutatás és
katalógus)** terve, a teljes architektúra-váz csak keretként szerepel benne.

### 1.1 Architektúra-váz (a későbbi szeletek építik)

```
fetcher réteg                     kanonikus Rust pipeline (meglévő)
┌────────────────────────┐
│ Rust adapter (nyílt:   │──┐
│ CSV/JSON/server HTML)  │  │   raw.artifact + raw.capture
├────────────────────────┤  ├──▶ (tömörített objektumtár)
│ elszigetelt browser-   │  │        │
│ fetcher (Obscura első  │──┘        ▼
│ jelölt; csak spike-    │     stage.observation
│ bizonyítékkal)         │        │
└────────────────────────┘        ▼
                            canonicalize (idempotens)
                                  │
                                  ▼
                            verification + bizalmi mátrix
                                  │
                                  ▼
                            publish gate ──▶ /v1/day ──▶ kliens
```

- A fetcher **kizárólag nyers payloadot** állít elő; kanonikus entitást soha nem ír.
- A v1 motor-policy-k (napi kvóta, delay+jitter, robots-ellenőrzés, circuit breaker,
  coverage-ledger, raw-retention) a `worker` crate-be portolódnak — **S2 feladata**.
- Az identitás fail-closed: csak explicit crosswalk/alias vagy egzakt egyezés; minden más
  review-sorba kerül. Automatikus fuzzy összevonás tilos.

### 1.2 Alszeletek (mindegyik saját spec → terv → implementáció ciklus)

1. **S1 — forráskutatás és katalógus** (ez a spec).
2. **S2 — ingest keretrendszer:** adapter-szerződés, worker ütemező kvótákkal/breakerrel/ledgerrel,
   bizalmi szintes publish gate, fail-closed identitás-alapok, séma-rések lezárása.
3. **S3–S7 — adapterek sorban** (a katalógus adoptációs rangja szerint), szeletenkénti tesztekkel.
4. **S8 — kliens:** batch-frissítés jelzése, provenance/bizalmi szint megjelenítése.

### 1.3 S1 hatóköre

- Forrásonként élő spike-próba (eldobható, `/tmp`), hozzáférési mátrix, mezőszintű katalógus
  a v2 sémára képezve, bizalmi javaslat domainenként, robots/rate-limit igazolás, lefedettség
  a demo körre (top-5 liga, NB I/NB II, kupák).
- Kimenet: élő forráskatalógus-dokumentum + elfogadott adoptációs sorrend + bizalmi mátrix,
  ami az S2 spec bemenete.
- **Nem része S1-nek:** production adapter, séma-migráció, scheduler, kliensváltozás.
- A v1 anti-patternjei tiltólistásak: fabrikált eredmény hamis provenance-szal
  (eredmenyek.com), `robots_override`, stealth/blokk-megkerülés.

## 2. Kutatási módszer és spike-protokoll

Forrásonkénti lépéssorrend, minden lépés bizonyítékkal:

1. **Asztali elemzés:** v1 extractor + dokumentumok (mezőnevek, selectorok, quirkek),
   hivatalos docs/robots.
2. **Robots és feltételek ellenőrzése először:** robots.txt + ToS-jegyzet. Amit a robots tilt,
   azt nem kerüljük meg; tiltott → `elvetett` indoklással. Dokumentált, programozott használatra
   szánt API-felület (pl. Livescore MCP JSON-RPC) intended-use; ha a robots az API útvonalat is
   tiltaná, az felhasználói döntési pont, nem csendes megkerülés.
3. **Hozzáférési próba:** hitelesítés nélküli GET a helyi gépről (kulcsos forrásnál a meglévő
   kulccsal, read-only); státusz, latency, TLS-viselkedés. Ez dönti el a szállítást:
   plain HTTP (Rust) vagy browser/TLS-impersonáció — csak spike-bizonyítékkal.
4. **Minta payload:** domainenként 1–3 reprezentatív kérés; kutatási budget forrásonként
   ≤ 10 kérés/nap, a v1 delay-eknél nem kisebb késleltetéssel.
5. **Séma-ellenőrzés:** parse, mezőlista típusokkal/nullázhatósággal, időzóna- és ID-stabilitás;
   képezés a v2 sémára (`core.match`, `core.match_result`, `fact.odds_snapshot`, `core.standing`);
   ami nem fér bele → **séma-rés** jelölés (S2/S3 bemenet), nem improvisálás.
6. **Lefedettség a demo körre:** ligák/országok, magyar nevek/aliasok, dátumkezelés
   (Tipplap múlt-napi oldalának ma-listát szolgáló hibája → fail-closed guard kötelező).
7. **Bizalmi javaslat domainenként** indoklással (3.2 mátrix).
8. **Katalógus-státusz:** `javasolt` → `igazolt` (spike zöld) → `elvetett` (indoklással)
   vagy `regisztráció szükséges`. Csak igazolt forrás kaphat adapter-szeletet.

### 2.1 Biztonsági és fegyelmi szabályok

- Kizárólag read-only GET/HEAD (illetve dokumentált read-only API-hívás); nincs login,
  cookie-megkerülés, CAPTCHA-fejtés, rate-limit-feszítés.
- 403/429 esetén azonnali stop, nincs retry-kalapálás; a breaker a kutatásban is él.
- Spike-kód és nyers payload **csak `/tmp`-ben**; repóba nem kerül (méretkorlát 5 MB,
  timeout, séma-validálás elfogadás előtt).
- Spike-ok **nem írnak** dev/teszt DB-be; egyetlen kimenet a katalógus-dokumentum.
- Regisztrációhoz kötött forrásnál a spike megáll az auth-határnál; kulcsot nem hozunk
  létre engedély nélkül.
- Bizonyíték: próba-log (státusz, latency), mezőtábla, payload sha256 — nyers adat nélkül.
- OpenFoot: a v1 `.env`-ben lévő Bearer kulcs újrahasználata read-only, az 5000 kérés/hó
  kereten belül; a kulcs Keychainbe kerül (AGENTS 7. pont). A felhasználó bármikor
  visszavonhatja a kulcs-újrahasználatot; akkor a spike regisztrációs falnál megáll.

## 3. Katalógus-séma, bizalmi mátrix, Obscura

### 3.1 Katalógus-bejegyzés sémája (forrásonként egy blokk)

- `slug`, név, base URL, típus (hivatalos / archívum / aggregator / open-data / API);
- **hozzáférés:** none / ingyenes regisztráció / fizetős; auth mechanizmus, titok tárolása Keychainben;
- **szállítás:** `rust-http` | `obscura` | `chrome-fallback` — spike-döntés, v1 előzménymérés hivatkozással;
- **domainek + mezőtérkép:** forrásmező → v2 `tábla.oszlop`, típus, nullázhatóság, időzóna; séma-rések jelölve;
- **lefedettség:** demo-kör mátrix (megvan / hiányzik / alias kell);
- **identitás:** külső ID stabilitás, crosswalk-lefedettség (reep, Wikidata), alias-igény (HU↔EN, gender, ország);
- **kvóta/robots:** robots-ítélet + engedett útvonalak, napi budget, delay, breaker-paraméterek;
- **frissesség-javaslat:** batch-frekvencia, odds closing-ablak;
- **bizalom domainenként** (3.2);
- **v1-érettség + kötelező őrök** (dátum-guard, decoy-200 tartalmi őr, 90-perces konvenció);
- **kockázatok + mitigáció**;
- **státusz + bizonyíték** (spike-log dátum, payload sha256, v1-doc hivatkozás);
- **adoptációs rang**.

**Elhelyezés:** a katalógus élő dokumentumként `docs/research/source-catalog.md`-ben él.
Új mappa indoklása: szeleteken átívelő, bizonyíték-frissítésű katalógus — sem a spec, sem a
terv, sem a checkpoint, sem az operations nem erre való. Az adoptációs döntések
`docs/decisions/`-be kerülnek; a spike-jegyzőkönyvek a napi checkpointba.

### 3.2 Bizalmi mátrix (domain × forrásosztály)

| Domain | Magas bizalom (egy forrásból is verified) | Közepes (keresztellenőrzés vagy provisional) |
|---|---|---|
| Fixture/kickoff | hivatalos ligaoldal, football-data, TheSportsDB | aggregator scrape (FotMob, Livescore, OpenFoot) |
| Eredmény FT/HT | hivatalos, football-data, worldfootball | aggregator; ET/pens csak explicit forrásból |
| Odds | — (mindig kétforrásos egyezés, pl. Tipplap + football-data closing) | egy forrás = provisional + kliens-jelzés |
| Tabella | hivatalos, football-data számított | FotMob-sor keresztellenőrzéssel |
| Kezdő/hiányzó/xG/stats | — | közepes; display-only, nem gate-tényező |
| Identitás-híd | reep „strong rung” (name-dob), Wikidata egzakt | minden más → review-sor |

A mátrix az S1 kilépésekor felhasználói jóváhagyást kap; a verification szabálymotor
S2-ben kapja a bizalmi szinteket (a meglévő gate backward kompatibilis bővítése).

### 3.3 Obscura szerepe

- Az [Obscura](https://www.obscura.sh/) (Rust headless engine, V8, CDP-kompatibilis,
  self-host Docker/bináris) a browser-fetcher réteg **első jelöltje**; a
  [GitHub-repo](https://github.com/h4ckf0r0day/obscura) és a v1 mérései a kiindulópont.
- A v1 már beépítette opt-in szállításként (helyi bináris + CDP 127.0.0.1:9222, Playwright
  kliens; a kvóta/breaker/robots változatlanul érvényes rá; stealth alapból KI).
- **Nem általános csere:** default `rust-http`; Obscura csak ahol a spike igazolja a tartalmi
  többletet (JS-renderelt oldal, pl. Soccerway). A v1 2026-11-09-i mátrixa szerint a
  Cloudflare/IP-blokk nem nyílt meg (nem is próbáltuk), az SSR oldalak olcsóbban jönnek HTTP-vel.
- **Decoy-200 szabály:** minden adapterben parser-szintű tartalmi őr kötelező (HTTP 200
  önmagában nem bizonyíték; a v1 Understat-esete: üres decoy oldal 200-cal).
- Stealth/blokk-megkerülés tilos; a v1 `STEALTH=1` kapcsolója nem kerül át.

## 4. Forrás-univerzum

### 4.1 Adatigény → forrásjelöltek

| Adatigény (app + roadmap) | Elsődleges jelöltek | Másodlagos / rés-pótló |
|---|---|---|
| Napi fixture/kickoff | football-data, TheSportsDB, Tipplap | ESPN, hivatalos ligaoldalak, API-Football (reg.) |
| Eredmény FT/HT/ET/pens | football-data, worldfootball | ESPN (90’ konvenció = rés), hivatalos, Livescore MCP |
| Odds 1X2/BTTS/O-U + piacok | Tipplap, football-data (B365/PS/Avg, nyitó+záró) | The Odds API / API-Football (reg., keresztforrás) |
| Tabella | football-data számított, FotMob | hivatalos, API-Football (reg.) |
| Kezdők/hiányzók | FotMob, Transfermarkt | PL-hivatalos, API-Football (reg.); SofaScore elvetett |
| Meccs-stats (lövés/szöglet/lap) | football-data, FotMob | StatsBomb |
| xG/események | FotMob (PL-t is ad), StatsBomb (no PL) | Understat elvetett (decoy-200) |
| Játékos-stats/góllövők | Transfermarkt, FotMob | FM27 helyi save (licencelt, opcionális) |
| Logó/zászló/stadion/referee | GitHub football-logos, Wikimedia Commons, TheSportsDB | FotMob CDN, football-data (referee) |
| Identitás-crosswalk | reep (CC0, strong rung) | Wikidata/SPARQL (CC0), v1 alias táblák |
| Elo/rating (predikcióhoz) | ClubElo tükör | — |
| Történeti kalibráció | football-data archívum 1993→, StatsBomb | ClubElo |
| FA Kupa / hazai kupák (**rés**) | worldfootball csak UEFA-kupák | thefa.com, Sky Sports, Soccerway, ESPN, Tipplap — spike dönt |

### 4.2 v1-leltár státuszai (kiindulás, spike-frissítéssel)

**Működőként igazolt v1-ben:** Tipplap (odds + settlement, HU nevek, 0,3 s delay,
dátum-eltérés guard), football-data.co.uk (CSV: FTHG/FTAG/HTHG/HTAG, HS/AS/HST/AST/HC/AC/
HF/AF/HY/AY/HR/AR, Referee, B365/PS/Max/Avg nyitó+záró; 200 req/nap, 4+3 s; robots engedett;
aliased archív kódok tiltva), FotMob (`__NEXT_DATA__`: lineups, missing, xG, stats, incidents,
shotmap, 29 season metrics, standings; 200/nap, 6+6 s; `/api/*` robots-tiltott → HTML only),
Transfermarkt (hiányzók, góllövők; 40/nap, 8+8 s; season-id quirkek), worldfootball.net
(UEFA-kupák gerince, `pso` observed), StatsBomb open-data (események/xG; attribution,
non-commercial; no PL), TheSportsDB (ingyenes „3” kulcs, szezon-gerinc; 40/nap, 2+1 s),
reep heti CC0 cut (crosswalk, strong rung only), ClubElo tükör (bootstrap + self-computed).

**Részben/blokkolt v1-ben:** SofaScore (IP-ban, nincs day feed → elvetett), ESPN (letiltva,
11% linkelhetőség, AET/pen skip → újraértékelés), OpenFoot (read-only az incidens után),
Livescore MCP (v1 `robots_override` anti-pattern → becsületes robots-újraellenőrzés).

**Kifejezetten a felhasználó által forrásként megnevezve (2026-11-11):**
- **OpenFoot** (`openfootapi.com/v1/matches`): **5000 kérés/hó** (≈166/nap) keret; Bearer kulcs
  a v1 `.env`-ben (Keychainbe migrálandó); domain: fixture + lezárt eredmény; lineups/xG a
  starter plan gate-je miatt 403 → séma-rés; read-only fegyelem az incidens miatt.
- **Livescore MCP** (`livescoremcp.com/mcp`): JSON-RPC POST `tools/call`; domain: napi
  fixture/eredmény, lineups, incidents, csapat- és játékos-stats; v1 kvóta 100/nap, 6 s delay;
  intended-use API-felület, robots-ellenőrzés az API útvonalon; override nem kerül át.

**Tiltólista (elvetett, indoklással a katalógusban):** eredmenyek.com (fabrikált eredmény),
SofaScore (IP-ban), WhoScored (Cloudflare-tiltás), UEFA.com (hálózati blokk), Understat
(decoy-200), FlashScore + kicker (robots tilt), BBC Sport (ToS), FBref (ToS + Cloudflare).

**Új jelöltek (v1-ben nem szerepeltek):** API-Football, football-data.org, The Odds API
(ingyenes regisztrációs csomagok — hozzáférési mátrix), thefa.com (FA Kupa-rés),
Wikidata/SPARQL (crosswalk), valamint a v1 Obscura-mátrixában mért, adapter nélkül maradt
SSR oldalak: Bundesliga-hivatalos, Sky Sports, The Guardian (MLSsoccer a demo körön kívül →
katalógus-jegyzet).

**Meta-források:** logók/zászlók (GitHub `luukhopman/football-logos`, Wikimedia Commons API,
FotMob CDN, helyi FM MegaPack — licenc-ellenőrzéssel), FM27 helyi save (offline, licencelt,
opcionális), premierleague.com hivatalos hiányzók (PL only, 30/nap, 20+25 s).

## 5. Kutatási hullámok és sorrend

- **W1 — gerinc (teljes igazolás):** football-data → TheSportsDB → Tipplap → FotMob →
  Transfermarkt → worldfootball.
- **W2 — meta/display + API-források + újraértékelés (teljes igazolás):** logók/zászlók
  (GitHub + Wikimedia), stadion/referee (TheSportsDB/football-data), PL-hivatalos hiányzók,
  **OpenFoot**, **Livescore MCP**, ESPN újraértékelés (linkelhetőség-méréssel),
  FA Kupa-rés probe (thefa.com / Sky Sports / Soccerway / Tipplap).
- **W3 — mélység (próba-szintű igazolás; teljes igazolás a saját szeletben):** StatsBomb,
  reep, ClubElo, Wikidata crosswalk, FM27 (opcionális, licenc-ellenőrzéssel).
- **W4 — hozzáférési mátrix:** API-Football, football-data.org, The Odds API
  (regisztrációs falnál stop + kérdés); Bundesliga-hivatalos / Sky Sports / The Guardian
  SSR-probe; MLSsoccer katalógus-jegyzet.

Az adoptációs rang a W1–W2 sorrendjét követi; a W3/W4 források rangja a spike-eredmény
és a bizalmi mátrix függvénye, felhasználói jóváhagyással.

## 6. Kilépési kritériumok

### 6.1 Forrásonként (`igazolt` státusz)

- robots-ítélet + csak engedett útvonalak; hozzáférési próba zöld, budget-loggal (≤ 10 kérés/nap);
- domainenként ≥ 1 minta payload parse-olva, teljes mezőtérkép a v2 sémára, rések jelölve;
- demo-kör lefedettségi sor kitöltve (megvan / hiányzik / alias kell);
- tartalmi őr definiálva és **decoy-teszttel igazolva** (üres/decoy payload fail-closed bukik);
- bizalmi javaslat domainenként indoklással; szállítási döntés bizonyítékkal (rust-http vs Obscura);
- spike-log + payload sha256 a checkpointban; katalógus-bejegyzés magyar szöveg visszaolvasással.

### 6.2 S1 egész

- W1+W2 minden forrása igazolt vagy explicit elvetett;
- W3/W4 minden bejegyzése státusszal és bizonyítékkal vagy halasztási indokkal;
- FA Kupa-résre döntés-előkészítés (melyik forrás fedi, milyen szállítással);
- bizalmi mátrix + adoptációs sorrend felhasználói jóváhagyással;
- séma-rés lista összeállítva (S2 spec bemenet);
- OpenFoot-kulcs újrahasználat és regisztrációs kérdések külön döntési pontként rögzítve;
- döntés az S2 spec indításáról.

## 7. Tesztelés és bizonyíték

- Minden spike eldobható (`/tmp`), de **assertionökkel** fut: séma, tartalmi őr, dátum-guard,
  decoy-teszt; a futtatott parancs és kimenet a checkpointba kerül.
- Spike nem ír DB-be; payload nem kerül repóba; 403/429 → stop + státusz-jelölés.
- A v1 mérései kiindulópontok, nem tények: minden spike frissen mér (az oldalak változnak).
- ToS/robots-konfliktus → elvetett; IP-ban → stop, nem kliens-csere; dekoy-200 → tartalmi őr
  nélkül nincs igazolt státusz; licenc-korlát (StatsBomb non-commercial, FM27, logópackek)
  → csak attribution-nel/engedéllyel, üzleti használat előtt döntés.
- Katalógus + checkpoint magyar szöveg visszaolvasása; a katalógust a felhasználó review-zolja
  az S2 indítása előtt.

## 8. Kockázatok és stop-feltételek

- **FotMob DOM/séma-törékenység:** parser-szintű tartalmi őr + séma-verzió a katalógusban;
  törés esetén breaker + review, nem csendes nullázás.
- **Identitás-adósság:** a v1 100+ javító migrációja tanulság — alias/gender/ország-konvenciók
  előre tervezve (S2), fail-closed linker, review-sor; fuzzy merge tilos.
- **Kvóta-heavy többforrásos orchestration:** a worker ledger + breaker S2-ben; S1-ben a
  spike-budget fegyelem.
- **Tipplap dátum-eltérés:** fail-closed guard kötelező (múlt-napi oldal ma-listát szolgál).
- **Hiányzó adat nem nulla:** minden rés NULL/státusz; a „not found” nem lehet nulla
  (decoy-200 tanulság).
- **Regisztrációs/kulcs-falak:** stop + felhasználói döntés; kulcs Keychainben, nem fájlban/naplóban.

## 9. Következő lépések

1. Spec review (felhasználó) → jóváhagyás után commit külön kérésre.
2. `writing-plans` skill: S1 megvalósítási terv (spike-szeletek hullámonként, checkpointokkal).
3. S1 futtatása: katalógus építése a `docs/research/source-catalog.md`-ben.
4. S1 kilépés → S2 spec (ingest keretrendszer) a séma-rés listával és bizalmi mátrixszal.

## Hivatkozások

- v1 (csak olvasható): `../football-app/scrappers/` (tipplap, web/engine.py, web/linking.py,
  sources/*), `../football-app/docs/obscura-source-matrix-2026-11-09.md`,
  `../football-app/docs/obscura-transport-probe.md`,
  `../football-app/docs/fix-log-2026-11-05-fabrikalt-golszamok.md`,
  `../football-app/docs/fix-log-2026-11-07-openfoot-hibas-import.md`.
- Obscura: https://www.obscura.sh/ , https://github.com/h4ckf0r0day/obscura
- OpenFoot: https://openfootapi.com/ ; Livescore MCP: https://livescoremcp.com/