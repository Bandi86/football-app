# Napon belüli frissítés (S36b, 2026-11-20)

**Helyzet:** a felhasználó a források/napi működés javítását kérte, és külön engedélyezte a
napon belüli frissítést is: „A napon beluli frissites is mehet nemtudom mely forrasokat
hasznal de minel tobbet annal jobb.” Eddig minden engedélyezett forrásnak egyetlen
`discovery` eseménye volt (`interval_minutes=1440`), ezért a nap folyamán a nyitó szorzók
nem frissültek, és a meccsek állapota/oddsa csak másnap látszott.

## Döntés

1. **A napon belüli frissítés a meglévő policy-nyelven megy:** nem új eseménytípus és nem
   új DB-oszlop, hanem a `interval_minutes` szemantikájának kiterjesztése. A napi (1440) és
   heti (10080) interval mellett mostantól **a napot egyenlő sávokra osztó interval** is
   engedett (30 és 1439 perc között, ha osztja az 1440-et). A sávok száma `1440/interval`;
   sávonként egy forrás-szintű job keletkezik (`entity_key = slot-00`, `slot-01`, …,
   `time_window = UTC-dátum`, `due_at = sáv kezdete`, prioritás = esemény-alap + sávindex).
   A jövőbeli sávok a `due_at`-ig `pending`-ben várnak; a claim csak az esedékeset viszi.
2. **Tipplap kap napon belüli frissítést:** `refresh` esemény `interval_minutes=360` →
   napi 4 sáv (00/06/12/18 UTC) + a napi `discovery` = **5 kérés/nap**, ami belefér a
   `daily_budget=6`-ba. A Tipplap-oldal egy kérésben a teljes napi listát adja, ezért a
   sávos frissítés a nap összes meccsének szorzóit frissíti, nem egyenként.
3. **Többi forrás most nem kap napon belüli sávot, mert nem lenne értelme vagy nem fér bele:**
   - `openfoot`: a mai keret már elhasználódott (8/8 canary budget), nincs headroom;
   - `footballdata`: napi CSV-k, a szorzó csak lejátszott meccsre van bennük → napon belül
     nem ad többet;
   - `soccerway`: obscura-transport, a dayPicker továbbra is ellenőrizhetetlen (S30d), ezért
     a sávos futtatás vakon költené a keretet;
   - `fotmob`, `demo`: kikapcsolva.
   Ha ezek közül bármelyik állapota változik, a policy ugyanezzel a nyelvvel bővíthető.

## Bizonyíték

- `rust/crates/worker/src/scheduler.rs`: `supported_interval` (1440 / 10080 / a napot osztó
  30..1439), sáv-generálás a napon belüli ágban.
- Tesztek (test DB, `--test-threads=1`): `cargo test -p football-worker --test scheduler` →
  6 zöld (új: `subdaily_interval_generates_slot_jobs_with_due_times`: 240 → 6 sáv,
  `slot-00`…`slot-05`, prioritás 10…15, 12:00-kor 4 sáv esedékes, 20:00-kor mind a 6);
  `cargo test -p football-worker --test scheduler_validation` → 4 zöld (új:
  `subdaily_interval_generates_one_job_per_slot`; az érvénytelen interval teszt 50 percre
  állítva, mert a 45 osztja a napot).
- Dev DB policy frissítve: `ops.source_policy` tipplap eseményei
  `[discovery 1440, refresh 360]`; a többi engedélyezett forrás változatlan.

## Korlátok

- A sávok **UTC** szerint kezdődnek, a Tipplap-oldal viszont magyar (Europe/Budapest)
  napot szolgál ki; a 00 UTC-s sáv 01:00/02:00 helyi idő, ahol a dátum még egyezik, ezért a
  sávok a napot nem csúsztatják át. Ez dokumentált korlát, nem hiba.
- Régi binárison a `refresh 360` eseményt a scheduler „nem engedett interval” figyelmeztetéssel
  kihagyja, a napi discovery viszont fut — a policy-változás önmagában nem töri el a napot.
## Éles mérés (2026-11-20 14:31, a daemon újratelepítése után)

- Az új worker első tickje a policy-ból **4 sáv-job**ot generált (`refresh slot-00…slot-03`,
  `due_at` 01:00/07:00/13:00/19:00 helyi idő), majd a `slot-00` le is futott
  (`processed=1`): a frissített oldal (576 617 byte) parse-olva, 18 verseny materializálva.
- A `slot-01` és `slot-02` ekkor `quota_blocked` lett, mert a nap folyamán a kézi
  ellenőrző kérések már elhasználták a 6-os keretet — ezért emeltük 8-ra (fent).
- A mai nap oddsos meccsszáma változatlan 29/131; a további növekedés (a 44 parsolt
  Tipplap-meccsből még hiányzó ~15) a csapat-identitás review-ken múlik, nem a sávokon.