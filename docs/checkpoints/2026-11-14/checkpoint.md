# 2026-11-14 — Napi checkpoint

**Állapot:** S2c (Obscura-integráció) brainstorming indul
**Előző nap:** 2026-11-13
**Kapcsolódó dokumentumok:** [tegnapi checkpoint](../2026-11-13/checkpoint.md),
[S1 spec](../../superpowers/specs/2026-11-11-real-data-sources-research-design.md) §3.3 (Obscura),
[S2a spec](../../superpowers/specs/2026-11-12-s2a-ingest-framework-design.md) (Fetcher trait)
**Branch:** `main` (5644471, szinkronban originennel); az S2c implementáció
a `feat/s2c-obscura` ágon indul majd (létrehozás a terv jóváhagyása után).

## Napi cél

S2c: Obscura-integráció a Fetcher trait mögé — browser-fetcher réteg a
JS-renderelt oldalakhoz (pl. Soccerway, FotMob nap-feed), csak
spike-igazolt többletnél; stealth tiltott. Brainstorming → spec → terv →
implementáció.

## Kiinduló állapot

- [x] Az S2b-1 (identitás) + S2b-2 (bizalmi gate) tegnap merge-elve a
      main-be (b63e286); a záró checkpoint commitolva (5644471).
- [x] 152 teszt zöld; demo 20/20 verified + verified odds; dev 33 tábla.
- [x] Kötelező olvasási sorrend felvéve: AGENTS.md, README, S1/S2a spec,
      tegnapi checkpoint, git állapot (main, tiszta).
- [x] A felhasználó a mai szeletnek az S2c-t választotta.
- [ ] Technikai korlát: a subagent-típusok (Explore/Plan/general-purpose)
      NEM elérhetők ebben a sessionben → az SDD subagent-driven végrehajtás
      nem használható; inline execution (executing-plans) a fallback.
      A brainstorming/spec/terv tervezést nem érinti.

## Tervezett szelet

S2c — Obscura browser-fetcher:
- Az S1 spec §3.3 szerint az Obscura (Rust headless engine, V8,
  CDP-kompatibilis, self-host) a browser-fetcher réteg első jelöltje.
- Default rust-http; Obscura csak ahol a spike igazolja a tartalmi
  többletet (JS-renderelt oldal). A v1 2026-11-09-i mátrixa szerint a
  Cloudflare/IP-blokk nem nyílt meg; az SSR oldalak olcsóbban jönnek
  HTTP-vel.
- Decoy-200 szabály: minden adapterben parser-szintű tartalmi őr
  kötelező. Stealth/blokk-megkerülés tilos (a v1 STEALTH=1 nem kerül át).
- A Fetcher trait (S2a) mögé integrálódik — a keretrendszer nem változik.

## Következő döntési pont

1. S2c brainstorming → spec → terv → implementáció (inline execution).
2. Utána: S2d (séma-rések) vagy S3 (első éles adapter, 13+ belépési kapu).

## Visszaállítási út

- Új fájl: ez a checkpoint (munkafa) — elvetés csak kifejezett engedéllyel.
- DB és kód érintetlen (a szelet még nem indult).

## S2c — spike + brainstorming + spec (2026-11-14)

- [x] Kontextus: v1 obscura-transport-probe (2026-11-09, ez a gép) —
      engine feasibility igazolt; S1 spec §3.3; S2a Fetcher trait.
- [x] Spike (throwaway /tmp/s2c-spike, felhasználói engedéllyel):
      chromiumoxide 0.9.1 → Obscura CDP connect OK, goto renderel (~3.6s),
      content() 1.5s settle + retry után stabil; **Soccerway /matches/
      obscura 2060 marker vs reqwest 0** (többlet igazolt). Nyitott detail:
      HTTP-státusz capture Network.enable-t igényel. Obscura serve leállítva.
- [x] Brainstorming: spike-first + opt-in infra (jóváhagyva); megközelítés A
      (ObscuraFetcher a Fetcher trait mögé + footballd-managed szerver);
      5 design szekció egyenként jóváhagyva.
- [x] Spec: `docs/superpowers/specs/2026-11-14-s2c-obscura-design.md`
      — önreview: v1 probe link javítva, glyph-scan tiszta (← diagram-nyíl
      szándékos), magyar visszaolvasás kész, záró újsor megvan.
- [ ] Felhasználói spec-review: FÜGGŐBEN.
- [ ] writing-plans + implementáció: NEM indult (subagent nem elérhető →
      inline execution fallback).

## Következő döntési pont

1. A felhasználó review-zza a specet; jóváhagyás után writing-plans
   (terv: docs/superpowers/plans/2026-11-14-s2c-obscura.md).
2. Implementáció inline execution-nel (executing-plans), taskonkénti STOP.

## S2c végrehajtás (inline execution, feat/s2c-obscura, docs commit b051498)

- [x] Task 1: Alap + 0007 migráció — Transport::Obscura variáns + db_str;
      0007_obscura_transport.sql (source_policy.transport CHECK bővítés,
      constraint név ellenőrizve); ingest obscura cargo feature
      (chromiumoxide 0.9.1 optional rustls+zip8, tokio/futures/thiserror
      optional); ObscuraError + ObscuraConfig (from_env_with injektálható).
      Tesztek: domain 3/3, storage 2/2, ingest config 4/4; fmt/clippy
      (obscura feature) tiszta; default build (feature KI) fordul.
- [x] Task 2: ObscuraServer lifecycle — server.rs (lazy start: OBSCURA_URL
      → external health-check; egyébként bináris-ellenőrzés + spawn +
      /json/version poll ~10 s), cdp_url(), stop() (kill+wait), Drop;
      BinaryNotFound / ServerStart hiba utak. Tesztek: obscura_server 2/2;
      teljes ingest suite 24/24 (obscura feature); default build, fmt,
      clippy tiszta. Terv-eltérés: is_external() accessor (dead_code őr)
      + Debug derive (unwrap_err miatt).
- [x] Task 3: ObscuraFetcher CDP fetch — a terv szerinti chromiumoxide-alapú
      státusz-capture bizonyítottan nem működik (az Obscura
      Network.responseReceived payloadjából hiányoznak a chromiumoxide
      kötelező mezői → az egész CDP-üzenet parse-a elbukik; custom event,
      Fetch-válaszstádium, performance timing, scrape CLI és második
      kapcsolat is zsákutca — nyers CDP-probe-okkal igazolva). Felhasználói
      döntéssel (2026-11-14) a chromiumoxide kikerült; helyette saját,
      lenient mini CDP-kliens (async-tungstenite 0.32): open/attach →
      Network.enable → navigate → status → outerHTML → closeTarget.
      Tesztek: obscura_fetch 1/1 (élő lokális serve + wiremock), teljes
      ingest suite 25/25, default build, fmt, clippy tiszta; a
      chromiumoxide kikerült a lockból.
- [x] Task 4: Fetchers szelektor + wiring — fetch.rs `Fetchers`
      (new/with_obscura/rust_http/for_transport), storage
      `has_enabled_transport`, tick/runner `&Fetchers`, footballd lazy
      Obscura-szerver + shutdown stop, footballctl Fetchers. Eltérés a
      tervtől: (a) a robots-őr fixture transzportnál a fixture fetcheren
      megy (különben a tesztek élő hálózatra mennének), valódi forrásnál
      plain HTTP; (b) a transzport-választás hibája nem `?`, hanem
      graceful Failed a job lezárásában (érthető ok). Tesztek:
      fetchers_select 1/1 (default + obscura), storage obscura schema 3/3,
      workspace 157/157, ingest obscura 26/26, clippy/fmt tiszta,
      `cargo check -p footballd --features obscura` fordul.
- [x] Task 5: Élő render teszt + operations doc + zárás — új
      `obscura_live.rs` (opt-in, OBSCURA_LIVE_TEST=1; Soccerway /matches/
      + reqwest összehasonlítás), `docs/operations/obscura.md` (bináris
      beszerzés/asset API, env, opt-in, stealth KI,
      --allow-private-network, rollback, hibaelhárítás), .gitignore
      (/tools/obscura/), db-check.sh megjegyzés (0001–0007, 33 tábla).
      Élő bizonyíték: 2136 vs 0 marker (Soccerway, 2026-11-14), korábbi
      futás 300 vs 0; spike: 2060 vs 0. Megfigyelés: rövid időn belüli
      ismételt futásoknál a Soccerway/engine elakad (Timeout, fail-closed)
      — a teszt opt-in, cooldown kell; a fetcher beragadás ellen korlátos
      (navigate 20 s, ready 20 s, evaluate 15 s, close 5 s).
      Implementációs tanulságok (a kliensben rögzítve): Page.enable kell a
      Page.navigate válaszához; a navigate válasza a load-ig késhet; a
      státuszt a Document-típusú válaszból kell venni (redirect).
      Záró kapuk: check.sh exit 0, 157/157 workspace, ingest obscura 27/27,
      clippy (workspace + feature), footballd --features obscura, fmt.
- [x] Dev migráció: 0007 lefutott a dev DB-n (footballctl migrate:
      „migráció kész"); db-check: dev 33 tábla OK. A test DB a kézi
      tesztfutások maradékaitól db-reset-tel tisztítva (engedélyezett;
      csak TEST_DATABASE_URL): test 33 tábla, 0 üzleti sor.
- [x] Commit: S2c szelet egy commitban (jóváhagyott kérésre, push nélkül).

## Deferred minorok (S2c)

1. Policy-szintű fetch-timeout: a worker FETCH_TIMEOUT_SECS = 30 s statikus;
   browser-renderelésnél szűk lehet (a live tesztek 8–45 s tartományban
   futottak). Ops-döntés a policy bővítéséről (fetch_timeout_ms).
2. Az ObscuraFetcher nem utasítja el a nem-2xx választ (a RustHttpFetcher
   igen): a renderelt hiboldal + státusz megy tovább, a parser-szintű
   őrök/karantén fognak. Szándékos (a decoy-őr a parser rétegben van),
   de a breaker-viselkedés ettől eltér — dokumentálva.
3. Upstream chromiumoxide-jegyzet (a strict `Response` mezők miatt) nem
   készült el — külön feladat, publikus repó-interakció engedéllyel.

## Visszaállítási út (S2c zárás)

- Kód: a `feat/s2c-obscura` branch commitja revertelhető; a main
  (5644471) érintetlen.
- DB: a 0007 csak a CHECK-et bővíti; visszaállítás: a constraint
  visszaállítása ('rust_http','fixture') — csak ha nincs obscura sor.
- Feature: default build (obscura KI); a policy transport visszaállítása
  rust_http-ra; forrás-váltás nem történik.

## S2d-1 indul (2026-11-14, S2c zárása után)

- [x] Szelet-választás: S2d első ciklusa = S3-előkészítő minimum (D: odds,
      C: referee/venue, A1: match_stats); a többi S2d-csoport külön ciklus.
- [x] Brainstorming (szekciónként jóváhagyva): normalizált `fact.odds_quote`
      a fejléc megtartásával; match_stats per-source fix oszlopokkal;
      referee/venue entitás-FK.
- [x] Spec: `docs/superpowers/specs/2026-11-14-s2d1-schema-gaps-design.md`
      — felhasználói review jóváhagyva.
- [x] Terv: `docs/superpowers/plans/2026-11-14-s2d1-schema-gaps.md`
      — önreview: spec-lefedettség, típus-konzisztencia, glyph-scan tiszta.
- [x] Implementáció: Task 1 + Task 2 kész (lásd lent). Spec és terv
      commitolatlan (commit csak kifejezett kérésre).

## S2d-1 végrehajtás (feat/s2d1-schema-gaps)

- [x] Task 1: 0008 migráció + atomi odds-vágás — fakt.odds_quote
      (7 dimenziós PK, price/line/phase CHECK, FK-cascade) + backfill +
      odds-oszlop drop; fact.match_stats (per-source, fix nullable oszlopok);
      core.match referee/venue entitás-FK; canonicalize (fejléc + quote-blokk
      csere), load_match_odds (7 kulcs, fail-closed), day-LATERAL (JSON a
      quote-okból); test_support seed_min_match + TRUNCATE; db-check 35.
      Tesztek: s2d1_schema 3/3, canonicalize 3/3, trust 4/4, day 3/3;
      check.sh exit 0, 161/161; a tervből kimaradt két seed-hely
      (connection 33→35 táblaszám, api/routes odds-insert) a kapu kimutatta,
      javítva.
- [x] Task 2: dev migráció („migráció kész"); backfill-ellenőrzés:
      40 fejléc → 280 quote (40×7 csoport); demo újratöltés (26 fájl, 0 hiba),
      day 2026-11-11: 20/20 verified_publish + verified odds; db-check:
      dev 35 tábla / 1101 üzleti sor (+40 historikus verify/gate sor a
      újratöltésből) / 6 worker, test 35 / 0 / 0 (reset után).
- Deferred minorok: policy-szintű fetch-timeout (S2c-ből), multi-bookmaker
  trust + multi-market display (S3), a maradék S2d-csoportok (A teljes, B,
  E, F, G Elo).
- Visszaállítási út: a 0008 inverz SQL a migráció fejlécében (odds jsonb
  visszaépítés a default/closing quote-okból; új táblák/mezők drop; dev-en
  dry-run engedéllyel); test DB: reset; kód: branch-revert.
- Következő döntési pont: A-csoport (match_team_stats, lineups, incidents,
  shots, player-stats) vagy S3 (első éles adapter).

## S2d-2 végrehajtás (feat/s2d2-match-micro)

- [x] Task 1: 0009 migráció (öt meccs-mikro tábla: match_team_stats,
      match_player_stats, match_lineups, match_shots, match_incidents;
      per-source minta, dupla játékos-hivatkozás, CHECK-elt zárt dimenziók),
      constraint- és kaszkád-tesztek (s2d2_match_micro_schema 4/4),
      test_support TRUNCATE +5, connection 40, db-check 40; check.sh exit 0,
      165/165; commit **e904860**. Gotcha: az új migrációs fájlt a
      `sqlx::migrate!` rebuild nélkül nem vette észre (touch a storage lib-re).
- [x] Task 2: dev migráció („migráció kész"); dev 40 tábla / 1101 üzleti /
      6 worker; test DB reset után 40 / 0 / 0; checkpoint-commit külön.
- Deferred: a mikro-adatok writer/API/trust rétege (S3); a maradék
      S2d-csoportok — B (szezon/elérhetőség), E (ET/pens + kickoff
      konvenciók), F (identitás/assets), G (Elo).
- Visszaállítási út: az öt tábla dropja (0009 inverze); test DB: reset;
      kód: branch-revert.
- Következő döntési pont: B/E/F/G csoportok vagy S3 (első éles adapter).

## S3a végrehajtás (feat/s3a-entry-gates)

- [x] Task 1 (**a248b7f**): fetch-reziliencia — `RateLimited` + Retry-After
      parse, ledger-alapú exponenciális backoff (60 s·2^n, 1 h sapka) + jitter,
      késleltetett requeue (tranziens → pending/blokkolt, végleges → failed),
      fetch-szintű breaker-siker (half-open nem ragad be). Tesztek:
      http_fetcher 429 + 4 tick e2e + backoff unit; check.sh 169/169.
- [x] Task 2 (**a0d7a35**): robots origin cache-kulcs + origin robots-URL,
      Sitemap-only/BOM engedés, differenciált TTL (átmeneti 1 h), robots-ledger
      (job_id NULL) + kvóta- és min_delay-kizárás; robots_gate 11/11.
      A kapu kimutatta a robots-sor min_delay-óra mellékhatását (javítva).
      check.sh 173/173.
- [x] Task 3 (**d37613e**): scheduler event_type/interval validáció
      (kihagyás + warn), media_type a payload content-type-jából, CLI
      doctor/worker status repo-függvényeken; scheduler_validation 3/3 +
      persist_raw_media_type 1/1; check.sh 177/177.
- [x] Task 4: cross-kind approve (`IdentityError::KindMismatch`),
      `identity::crosswalk_conflicts` + CLI `identity crosswalk-check`,
      közös fail-closed linkeres feloldás a footballctl IngestFixture-ban
      (a hardcode törölve). Tesztek: review 4/4, crosswalk_check 1/1;
      check.sh 179/179.
- [x] Demo/ellenőrzés (dev, read-only): `day --date 2026-11-11` → 20/20
      verified_publish + verified odds; `identity crosswalk-check` → nincs
      konfliktus.
- [ ] Halasztva: #10 OpenFoot havi számláló (OpenFoot-szelet), #11 többworker
      fencing + half-open atomitás (többworker bevezetésekor).
- Visszaállítási út: branch-revert; séma érintetlen (nincs migráció); a
  robots-cache új origin-sorai a TTL-lel kifutnak; a ledger-sorok auditként
  megmaradnak.
- Következő döntési pont: **S3b — footballdata-adapter** (élő forrás, külön
  ciklus: brainstorming → spec → terv → taskonkénti STOP).

## S3b-1 végrehajtás (feat/s3b1-observer-path)

- [x] Task 1 (**4f117b6**): NormalizedDay + materializer-port — közös,
      forrás-független kanonikus típus (verseny/szezon/csapat/meccs
      id-hintekkel a demo id-stabilitáshoz); `Canonicalizer::materialize(
      &NormalizedDay)`; tick/CLI átállás; teljes demo-regresszió zöld.
- [x] Task 2 (**2fbe20d**): `NormalizedDay::from_observations` — verziózott
      observation-séma (a match_core verseny/szezon-meta-t, source_code-ot,
      captured_at-et is hordozza); a DemoAdapter a közös
      `fixture::build_observations`-t használja; a tick live ág az
      observationökből épít (a FixtureSource-újraolvasás törölve);
      builder ≡ `From<&FixtureSource>` egyenlőség-teszt; check.sh 181/181.
- [x] Task 3 (**e358e9a**): identity bootstrap — `review::approve_create`
      (kind-tudatos: team → entity + team, ország a kontextusból; a
      approve-lánc egy tranzakcióban), CLI `identity approve-create`;
      check.sh 182/182.
- [x] Kétfázisú élő bootstrap (dokumentált üzemeltetési minta): 1. élő kör
      → review-sorok az ismeretlen csapatokra (fail-closed, nincs kanonikus
      meccs) → `identity approve-create`/`approve` → 2. kör → kanonikus
      meccsek + gate.
- Visszaállítási út: branch-revert; **nincs migráció**; a stage/kanonikus
  adatok forrás-kötöttek.
- Következő: **S3b-2** — footballdata CSV-adapter (5 liga), B365→Avg→Max
  odds-lánc (load_match_odds + day), policy/trust seed, élő kör külön
  engedéllyel; utána a maradék S2d-csoportok (B/E/F/G).

## S3b-2 végrehajtás (feat/s3b2-footballdata)

- [x] Task 1 (**fc21f7e**): élő módú materializálás — competition/season/match
      natural-key find-or-create (NULL-safe round_label), linkeres
      csapatfeloldás (nem egzakt → a meccs kimarad + review-sor), provenance
      a `source_record_key` alapján; `live_materialize` 2/2.
- [x] Task 2 (**2f24a42**): odds-lánc `default` → `B365` → `Avg` → `Max` a
      `load_match_odds`-ban és a day-LATERAL-ban; trust_repos + day_query
      lánc-tesztek.
- [x] Task 3 (**ea5b016**): `FootballdataAdapter` — 5 liga, BOM/CRLF,
      header-alapú séma-drift hiba, ±7 napos ablak, Europe/London → UTC
      (date_only 12:00 UTC), B365/Avg/Max 1x2 + O/U 2.5 opening/closing
      quote-ok; valós formátumú CSV-fixture; adapter 2/2.
- [x] Adapter-regisztráció a footballd és a footballctl worker tick
      adapters-mapjében; `docs/operations/footballdata.md` (policy/trust
      seed, kétfázisú bootstrap, canary→live, rollback).
- [x] Élő kör (jóváhagyott engedéllyel, 2026-11-14): policy/trust seed a dev
      DB-n; 5 CSV **valódi fetch 200-zal** (43–57 KB) + robots (canary,
      delay-tisztelet: ligánként 1 tick); staging: 51 match_core + 43
      match_odds + 86 team_identity observation; **86 csapat review-sor
      (pending)**; a live forrásból **0 kanonikus meccs** (fail-closed,
      kétfázisú bootstrap — a review-jóváhagyás után jönnek a meccsek);
      jobs 5/5 done, coverage 5/5 done, ledger success-sorok.
      A futás közben talált és javított hiba: a kimaradó (feloldatlan)
      meccshez is írt volna odds-fejlécet (FK-sértés) — a `write_odds` most
      csak a materializált meccshez fut, regressziós teszttel.
      Nyitott üzemeltetési igény: a 86 review-sor batch-jóváhagyása
      (approve-create egyenként) — S3b-3 jelölt.
- Eltérések (dokumentálva): a stats/referee observation-aspketek emissionje
  elhalasztva (S3b-3 jelölt); a wiremock-os tick-e2e helyett az élő kör a
  végponttól végpontig bizonyíték (a tick-út a demón bizonyított);
  `csv` = 1.x.
- Visszaállítás: branch-revert; séma érintetlen; policy/trust sorok törlése.
- Következő döntési pont: élő kör engedélyezése, majd S3b-3 (stats/referee +
  e2e) vagy a maradék S2d-csoportok (B/E/F/G).

## S3b-3 — élő bootstrap + második élő kör (2026-11-14)

- [x] Batch identity bootstrap: `identity approve-create-all` (86 sor,
      ország find-or-create) — commit `ae53d2c`.
- [x] Élő kör második futása (jóváhagyott engedéllyel): 5 CSV újra 200,
      kanonikus meccsek a ±7 napos ablakban, **1290 élő odds-quote**
      (B365/Avg/Max), a gate **43/43 verified_publish**.
- [x] A futás két valós hiányt talált és javított:
      (a) a trust-olvasó a hiányzó BTTS-t hibaként kezelte → `OddsValues`
      mezők `Option<f64>` (hiányzó kulcs = NULL, csak a mindkét oldalon
      jelen lévő kulcsokat hasonlítja) — `ae53d2c`;
      (b) az identitás-jelzők hint-alapú feloldása élő csapatoknál false
      volt → a `MaterializedMatch.identity_exact` (konstrukció szerinti
      igaz: csak feloldott csapatú meccs materializálódik) — `3b63f7e`.
- [x] Stats/referee aspketek (commit `ea81614`): adapter (HS/AS/…/Referee),
      builder (match_stats/match_referee), materializer (per-source
      fact.match_stats upsert + linkeres játékvezető; nem egzakt → NULL +
      review). Élő eredmény: **43/43 napi-ablakos meccs stats-sora** a dev
      DB-n; a játékvezetők review-ra várnak (fail-closed).
- Nyitott: játékvezető-review jóváhagyás (opcionális, display-hez); a
  canary→live váltás a felhasználó döntése.

## S2d-3 (B csoport) — szezon-statisztikák + elérhetőség (2026-11-14)

- [x] 0010 migráció: `fact.season_team_stats` (keyed, season+team+source),
      `fact.season_player_stats` (keyed, source_player_id + opcionális
      entitás-FK), `fact.player_availability` (kind CHECK, expected_return
      dátum, PK source+player+kind); constraint-tesztek 3/3; db-check 43;
      connection 43; check.sh 194/194.
- Író nélkül (mint a korábbi séma-szeletek); a FotMob/Transfermarkt
  adapterek töltik később.
- Következő: E (ET/pens + kickoff konvenciók), F (identitás/assets), G (Elo).

## S2d-4 (E csoport) — ET/pens + kickoff-precízió (2026-11-14)

- [x] 0011 migráció: `core.match_result.decided_by` (NULL = ismeretlen;
      CHECK ft/et/pens — a 90 perces FT/HT konvenció sérthetetlen, az ET/pens
      megfigyelt érték) + `core.match.kickoff_precision` (default `exact`,
      CHECK exact/date_only — a date_only déli helyőrző explicit jelölése).
      Constraint-tesztek 2/2; check.sh 196/196; táblaszám változatlan (43).
- A kickoff UTC-konverzió adapter-szintű (Europe/London → UTC, date_only
  12:00 UTC) — a footballdata adapterben már él.
- Hátra: F (identitás-kiegészítés/assets), G (Elo).

## S2d-5 (F csoport) — asset-nyilvántartás (2026-11-14)

- [x] 0012 migráció: `ops.asset` (entity_kind/entity_id, kind logo/flag/other,
      url, storage_key, source, licence, licence_status unknown/cleared/
      rejected, attribution, captured_at; egyediség entitás+kind+url).
      Constraint-tesztek 2/2; check.sh 198/198; db-check 44.
- A 16. tétel (identitás-crosswalk) már lefedett: S2b-1 táblák
  (source_entity_ref, alias, crosswalk, review_queue) + S3b-3
  crosswalk-uniqueness validáció + `identity crosswalk-check`.
- Hátra: G (Elo-pillanatkép, saját feature) + a játékvezető-review /
  canary→live operatív döntések.

## S2d-6 (G csoport) — club_elo pillanatkép (2026-11-14)

- [x] 0013 migráció: `fact.club_elo` (ország+klub+név as-of dátum, elo,
      **kötelező provenance** observed/provisional, source='clubelo',
      captured_at; entitás-FK opcionális). Határ-szabály: a 2025-06-01 előtti
      adat csak `provisional` lehet (jövőbeli információ nem szivároghat).
      Teszt 1/1 (határ + címke + PK); check.sh 199/199; db-check 45.
- Ezzel az S2d 18 tételes séma-rés listája teljes (A–G); hátra operatív
  döntések (játékvezető-review, canary→live) és a következő források.

## Operatív zárás (2026-11-14, este)

- [x] Játékvezető-review: `identity approve-create-all` → 9 sor jóváhagyva;
      az újra-futtatott tickek a játékvezetőket **9/9 kanonikus entitáshoz**
      kötötték (E0 referee-link élőben).
- [x] **Canary → live**: a footballdata policy módja `live`; ellenőrző
      tickek zöldek (delay/kvóta szerint szétosztva), a napi-ablakos meccsek
      gate-je változatlanul **43/43 verified_publish**.
- Megjegyzés: az éjszakai/ismételt tickek a 25-ös napi büdzsét és a 2 s-os
  delay-t tiszteletben tartják (blokkolt sorok = ütemezett várakozás).

## S4a — clubelo Elo-adapter (2026-11-14, új session)

- [x] `ClubEloAdapter` (clubelo API CSV) → `club_elo` observation-aspket →
      `ClubEloDay` builder → `Canonicalizer::materialize_elo` (as-of upsert,
      kötelező provenance, 2025-06-01 határ-szabály); tick-ág az elo-feedre.
      Tesztek: adapter 2/2 (BOM/CRLF, határ), materializer 1/1 (idempotens);
      check.sh 202/202. Operations doc: docs/operations/clubelo.md.
- [ ] Élő clubelo kör: FÜGGŐBEN — charter-kapu (új élő forrás; a clubelo
      robots.txt/elérés nem mért, a katalógus szerint is verifikálandó).

## FotMob research-kör (2026-11-15, 1 kérés az Obscura-n át)

- [x] Élő minta: fotmob.com kezdőlap, 546 KB renderelt HTML; a
      `__NEXT_DATA__` **megvan**, de a nap-feed már **nem** benne van:
      `pageProps` csak `fallback`-et tartalmaz (React Query →
      `/api/translationmapping?...`), azaz a FotMob **API-alapú** adatútra
      váltott az S1-katalógus mérése (2026-11-10) óta.
- Következtetés: a FotMob-adapter **nem** írható meg __NEXT_DATA__-parserkén
      spekuláció nélkül; külön research-szelet kell (API-végpontok
      feltérképezése, robots, séma-rögzítés), mielőtt adapter készül.
- A temporális research-módosítás visszaállítva; a repóban nincs nyers payload.

## S4b — FotMob nap-feed adapter (2026-11-15)

- [x] Research: a nap-feed **nyílt JSON API** („/api/data/matches?..."), a
      robots.txt explicit `Allow: /api/*`; a `status.utcTime` UTC, a
      csapatok stabil FotMob-id-k — **rust_http** elég, Obscura nem kell.
- [x] `FotMobAdapter` (liga-szűrő top-5, dátum-guard, status/eredmény
      mapping) → közös observation-séma → NormalizedDay; adapter a
      footballd/CLI regiszterben; teszt 2/2; check.sh 204/204.
- [x] Megjegyzés: 2026-11-14 válogatott szünet — a top-5 ligáknak nincs
      meccse a feedben (a live tick `no_data`, ez helyes).
- [x] Élő ellenőrzés: a robots.txt **tiltja az /api/*-ot** (ellentmondásos
      Allow sorok mellett) → a worker robots-kapuja **blokkol**
      (`robots_blocked`). Ez a helyes fail-closed viselkedés: a FotMob élesben
      **nem használható** a robots-megkerülés tilalma miatt. A policy
      `enabled=false`; az adapter kódja tesztelve marad, dokumentált
      alternatív hozzáférés esetére.
- Operations doc: docs/operations/fotmob.md.

## S4c — Soccerway research + asset-import + review-láthatóság (2026-11-15)

- [x] Soccerway research (1 Obscura-kérés): 1328 `wcl-tableScore` marker,
      Flashscore-szerű minified DOM (`data-testid`, `data-side`), meccs-href
      id-k; a kickoff-formátum a nyers HTML-ből nem evidens → a Soccerway
      parser **elhalasztva**, célzott research szükséges (nincs spekulatív
      parser).
- [x] `footballctl asset import --file <json>` + `repo::upsert_asset`
      (natural-key upsert, licence-státusz); teszt 1/1.
- [x] `worker status` kiírja a **függő identitás-review** számot (operatív
      láthatóság, item 4 egy szelete); teszt 1/1.
- [x] check.sh 206/206.
- [x] Soccerway célzott research (offline, a mentett 2 MB-ból — 0 új kérés):
      sor-wrapper `div.event__match[data-event-row=true]` (`id="g_1_<mid>"`,
      status-modifier: `event__match--scheduled`), kickoff `div.event__time`
      (HH:MM), csapatok `event__homeParticipant`/`event__awayParticipant`
      (`data-testid="wcl-simple-text-01"` a névhez), gólok
      `data-testid="wcl-tableScore"` span-ek `data-side=1/2`-vel; stabil
      azonosítók: `?mid=<id>` az href-ben + forrás-csapatkód a slug végén
      (`/merkozes/<home-slug>-<Code1>/<away-slug>-<Code2>/`). Dátum: nincs
      sor-szintű dátum-attribútum (napi csoportosítás, „Ma"); a dátum-specifikus
      URL (`?date=YYYY-MM-DD`) verifikálandó. AET/pens markup ebben a mintában
      nem volt (nincs ilyen lezárt meccs) — célzott minta kell hozzá.
- Következő: Soccerway-parser e spec alapján (dátum-URL verifikáció + AET-
  minta), asset-adatok operátori feltöltése, S8 kliens-integráció.
