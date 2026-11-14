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
