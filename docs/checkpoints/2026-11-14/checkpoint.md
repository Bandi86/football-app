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
