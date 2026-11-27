# 2026-11-27 — Predikció Phase 0 — megvalósítási terv

**Spec (RFC):** [`2026-11-23-modern-predikcios-rendszer-terv`](../specs/2026-11-23-modern-predikcios-rendszer-terv.md)
**Döntés:** a felhasználó 2026-11-27-ben a **mérési szerződés + Shin** szeletet
választotta a Dixon–Coles modell elé (v1 tanulságai: a mérőréteg hibás, a modell
minden piacon rosszabb volt a piacnál).
**Branch:** `main` (folyamatos felhatalmazás).

## 0. Határ

- Nem része: modell illesztés, machine learning, fogadási verdikt, valódi tét,
  UI-panel. A Phase 0 csak a **megítélhetőség** és a **piac-árazás** alapjait adja.
- Nem része a v1 kód átmigrálása; a v1 csak referencia ( Brier/3 ~0,20, `-14,4%`
  papír-ROI, minden piacon a piac nyert, mérőréteg CI nélkül).

## 1. P0a — tiszta Rust modul: margin, metrika, as-of (első szelet)

**Írási hatókör:** `rust/crates/prediction/**`. Nincs migráció, nincs API, nincs írás.

- [x] Új `football-prediction` crate a workspace-ben (`crates/*` automatikusan).
- [x] `margin.rs`: Shin-féle marginmentesítés (biszekció, determinisztikus),
      `fair_odds`, `edge`; 3 utas és többkimenetes piacokra is; üres/hibás
      odds → `None` (fail-closed).
- [x] `metrics.rs`: Brier (multi-class, opcionális /klassz normálás), log-loss,
      Wilson-konfidenciaintervallum, **klaszterezett bootstrap** a páros Brier-
      különbséghez (fix seed, konvergencia-, nem pszeudo-pontosság-teszt).
- [x] `as_of.rs`: szigorú időrendi őr — csak kickoff **előtt** rögzített
      feature/predikció használható; `date_only` placeholder nem lehet as-of.
- [x] Tesztek: Shin-összeg=1, favorite-longshot torzítás, edge, Brier/log-loss,
      Wilson, bootstrap-CI (szignifikáns és nem szignifikáns eset), as-of.
- [ ] Kapu: `cargo test -p football-prediction`, `clippy -D warnings`,
      `fmt --check`, teljes workspace, `daily-check`, `git diff --check`.
- [ ] Checkpoint + commit; P0a után megállás.

## 2. P0b — az as-of adatfeltétel és a mérési riport

- [ ] `analytics` séma (migráció) a predikciókhoz és a mérési snapshotokhoz;
      minden sor `model_version`, `as_of`, `inputs_hash`, `quality`, forrás.
- [ ] Offline mérő CLI: `footballctl prediction measure --from --to` —
      Brier/log-loss + Wilson + klaszterezett bootstrap + bázis/piac összehasonlítás,
      shadow módban (nem ír kanonikus adatot).
- [ ] Az első valódi mérés a meglévő odds-tárra (1X2/BTTS/OU + a most bekötött
      653 piac), verziózott, auditált.

## 3. P1 — Dixon–Coles / Poisson gólmodell (későbbi szelet)

- [ ] Tiszta Rust solver, 20×20 gólmátrix, 1X2/OU/BTTS, fair odds vs piac.
- [ ] Shadow-only, `analytics.predictions`, API/UI verdikt nélkül.
- [ ] A mérési szerződés (P0) szerinti elfogadási kapu: csak szignifikáns,
      klaszterezett, as-of-tiszta javulásnál lehet aktív modell.

## 4. Globális kapu-lánc

Lásd a `2026-11-27-tipplap-kiegeszito-forras-design.md` §8 és AGENTS.md §8.
Minden szelet: futtatott parancs + mért szám; magyar magyarázat; a nem
ellenőrzött részek külön kimondása.

## 5. Visszaállás

Szeletenkénti `git revert`; a P0a nem ír adatot, nem módosít sémát. A későbbi
migrációk visszaállítása a checkpointban rögzített SQL-lel.
