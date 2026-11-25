# Checkpoint — 2026-11-25

## Induló állapot

- A stabil állapot a `docs/checkpoints/2026-11-24/checkpoint.md`-ben és a `README.md`-ben olvasandó.
- A tegnapi S128–S132 szeletek (bugirtás, header, élő fázis, Következik-szűrő, Sofascore összevetés) pusholva, az utolsó commit `672ac7c`.
- A mai S133 (soccerway integritás, visszatartás, teljes napi check) a dátumváltás miatt még a 11-24-i checkpointbe került, commit `4704a1d`.
- Felhasználói irány: Soccerway integritás → 2. pont (lefedettség) → 3. pont (percátadás) → napi check → backfill folytatása.

## S134 — Research élő perc átadása a kliensig

- [x] **Probléma**: a research API-ben pontos `status.minute` van (pl. 87'), de az ingest `Status` struktúrája csak a `code`-ot őrizte meg, így a perc nem jutott el az API-be. A kliens S129 óta a kezdésből becsült (45+15+45), ami pontatlan, ha a meccs késik.
- [x] **Ingest** (`research.rs`): a `Status` struktúra felveszi a `minute: Option<i32>` mezőt, és a `match_core` payload új kulcsot kap: `status_minute`. Teszt: `status_minute_survives_deserialisation`.
- [x] **Storage** (`day.rs`): új `DayMatchRow.minute` mező, LATERAL join a legfrissebb research `match_core` obszervációból (`fact.provenance` → `stage.observation`, `o2.observed_at DESC`) — **nincs új migráció, nincs új kanonikus oszlop**. Integrációs teszt: `fetch_day_returns_research_live_minute` (valós artifact→capture→observation→provenance lánccal, minute=87).
- [x] **API** (`routes/day.rs`): `DayMatch.minute: Option<i32>`; a kontrakt-fixture két meccse `"minute": null`-lal bővült; új teszt `live_minute_is_serialized_when_present` (87 értékre). OpenAPI séma-teszt zöld.
- [x] **Kliens**: `DayMatch.minute` a Swift DTO-ban (CodingKeys `minute`), `MatchCardModel.from` átadja (`minute: match.minute`) — a korábban `nil` volt. Az S129-es `livePhaseText` így most a valódi percet használja, ha van; ha nincs, a kickoff-becslés marad. Tesztek: `testResearchMinuteReachesCardAndOverridesEstimate` (87' → „2. félidő · 87'"), `testMissingMinuteFallsBackToKickoffEstimate` (20' → „1. félidő · 20'").
- [x] **Telepítés**: az új `footballd` bináris telepítve (`scripts/install-footballd-agent.sh`, felhasználói jóváhagyással a launchd restarthoz), a frissített kliens is telepítve.
- [x] **Kapuk**: Rust **588** (2 új + 1 integrációs teszt), clippy 0, fmt 0, Core **125**, app unit **TEST SUCCEEDED**, UI **TEST SUCCEEDED**, daily-check **EXIT=0**, git diff 0.

## Döntések / korlátok

- [x] **2. pont (lefedettségi hézag) — kivizsgálva, blokkolt**: a Sofascore referencia ~11 meccse / 22 csapata (Buru SC, Vitesse U21, Torreense U23, ASFA Dakar, Koper U19, Ajman, Al Duhail, Halcones stb.) **nincs meg** a research nyers adatában és egyik engedélyezett forrás (research, openfoot, tipplap, footballdata) obszervációiban sem — csak csapatidentitások vannak. A `footballdata` 22+15 rögzített ligát követ, ezek nincsenek köztük. Sofascore adapter kifejezetten tiltott. Új forrás vagy ligakonfig csak külön döntéssel.
- [ ] **S131 nyitott döntés**: a 6 darab, 5 percnél régebben kezdődött de még `scheduled` meccs egyik státusz-szűrőben sincs (a `lejárt, nyitott` 3 órás küszöb alatt). „Mind"-ben láthatók.
- [ ] **Backfill**: a szeptemberi backfill 2026-09-30-tól visszafelé 2026-08-01-ig még hátra van.

## Következő lépés

- A szeptemberi backfill folytatása: `./scripts/backfill-day.sh --fetch 2026-09-30`, majd napi integritás/coverage és véletlen minták 2026-08-01-ig.
