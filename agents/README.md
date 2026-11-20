# Agent csapat — football-app-v2

Ez a mappa a projekt **szerepkártyáit** tartalmazza. A kártya nem prompt-gyűjtemény, hanem szerződés:
mit vállal a szerep, hol írhat, mivel bizonyít, és mikor kell megállnia.

A kártyákat bármelyik ágens-motor be tudja tölteni (DSH subagent, Team Member, másik harness).
A skillek a repóban élnek: `.dsh/skills/<név>/SKILL.md` — a DSH automatikusan katalógusba veszi őket.

## Szerepek

| Szerep | Kártya | Ír | Nem ír |
|---|---|---|---|
| Lead (root) | [`lead.md`](lead.md) | mindent, de csak integrációkor | — |
| Rust backend | [`rust-backend.md`](rust-backend.md) | `rust/`, `db/migrations/`, `scripts/` | `apps/` |
| SwiftUI kliens | [`swiftui-client.md`](swiftui-client.md) | `apps/` | `rust/`, `db/` |
| Adat-üzemeltetés | [`data-ops.md`](data-ops.md) | DB (auditált művelet), `docs/` | kód |
| Kutató | [`researcher.md`](researcher.md) | `docs/research/`, `docs/superpowers/` | kód, DB |
| Ellenőr | [`verifier.md`](verifier.md) | semmit (csak futtat/olvas) | mindent |

## Kötelező skillek szerepenként

- **Mindenki:** `football-app-v2` (szelet-folyamat, kapu-lánc, adat- és bizonyíték-szabályok).
- **Rust backend:** + `rust-workspace-backend`.
- **SwiftUI kliens:** + `swiftui-macos-client`.
- **Adat-üzemeltetés:** + a `football-app-v2` `references/data-ops.md`, `references/db-queries.md`,
  `references/gapfill-and-result-revisions.md` fájljai.
- **Kutató:** `research` (superpowers) a kutatás formájához.
- **Ellenőr:** `verification-before-completion`.

## Dispatch protokoll

1. **Egy szelet = egy felelős implementáló.** A lead a szeletet a kártya szerint kiosztja.
2. A subagent promptja mindig tartalmazza: (a) a kártya útvonalát, (b) a szelet pontos határát,
   (c) az írási hatókört, (d) az elvárt bizonyítékformátumot, (e) mit NE tegyen.
3. **Párhuzamosítás csak diszjunkt írási hatókörrel.** `rust/` és `apps/` egyszerre mehet;
   két ágens ugyanabba a fába nem ír. Adat-művelet (DB) nem fut párhuzamosan daemon-telepítéssel.
4. **Builder ≠ verifier.** Aki írta a kódot, nem ő pipálja ki a kaput. Az ellenőr külön ágens,
   tiszta kontextussal, és a futtatott kimenetet idézi, nem a szándékot.
5. **Handoff formátum** (minden ágens válasza a munka végén):

   ```
   SZELET: <azonosító/név> — ZÖLD|PIROS
   VÁLTOZOTT: <fájlok, mit és miért>
   FUTOTT: <parancs → mért eredmény: tesztszám / exit kód / adat-számok>
   NEM ELLENŐRZÖTT: <mi maradt ki és miért>
   KOCKÁZAT: <ha van>
   VISSZAÁLLÁS: <hogyan áll vissza a változás>
   KÖVETKEZŐ DÖNTÉSI PONT: <egy mondat>
   ```

   A formátum az `AGENTS.md` §12-vel azonos. A `FUTOTT` sor nem összegzés, hanem mért szám; a
   verdikt (ZÖLD/PIROS) a `SZELET` sorba kerül, mert külön verdikt-sor nincs.

6. A lead az eredményt **nem** fogadja el puszta állításra: a `FUTOTT` sorokat vagy újrafuttatja,
   vagy a kapu-kimenetre hivatkozva ellenőrzi. Ha hiányzik, visszaküldi.

## Csapat-topológiák

- **Solo szelet:** lead + 1 implementáló (+ ellenőr a kapunál). Ez az alapértelmezés.
- **Párhuzamos szelet:** két diszjunkt hatókör (pl. `rust/` API + `apps/` nézet), közös szerződés
  (API-mező) előre rögzítve; a két ágens nem nyúl egymás fájljaihoz.
- **Adat-kör:** data-ops futtat (reverify → integrity → coverage), majd verifier ellenőrzi a
  számokat a DB-ből, nem a riportból.
- **Kutatás → terv:** researcher ad forrásolt jegyzetet, lead jóváhagyja, csak utána indul kód.

## Ami tilos minden szerepnek

- `main`-re közvetlenül dolgozni a lead engedélye nélkül; commit/push csak a szelet zöld kapuja után.
- „Kész” állítás futtatott kimenet nélkül; el nem végzett manuális ellenőrzés pipálása.
- A v1 repó (`../football-app`) módosítása, titok/nyers adat commitolása, force-push.
- Egy másik szerep írási hatókörébe írni (ütközés esetén a lead dönt, nem az ágens).