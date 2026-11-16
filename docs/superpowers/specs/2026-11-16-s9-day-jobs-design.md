# S9 — Napi ingest-modell (forrás × nap) és verseny-bontás

**Dátum:** 2026-11-16
**Állapot:** jóváhagyott irány (A), implementáció folyamatban
**Kapcsolódó:** [S2a ingest keretrendszer](2026-11-12-s2a-ingest-framework-design.md),
[S4d Soccerway](2026-11-15-s4d-soccerway-design.md),
[S3a kapuk](2026-11-14-s3a-entry-gates-design.md),
[2026-11-16 checkpoint](../../checkpoints/2026-11-16/checkpoint.md)
**Branch:** `main` (b4ab41d)

## 1. Probléma (mért)

- A day-feed források természetes egysége a nap, a modell viszont
  ligánkénti jobokra vágja: Soccerway 7 render/nap ugyanazért az oldalért
  (2026-11-16: 8 kérés, **7,8 MB**, mind `no_data`), FotMob 5× ugyanaz az
  URL. A `plan() -> Vec<FetchRequest>` több kérést engedne, de a tick
  csak az elsőt használja.
- A `NormalizedDay` egy nap = egy verseny metát hordoz — ezért lett a
  day-feed forrásoknál ligánkénti job.
- A liga-slug szűrő miatt a nap többi versenye (kupák, alsóbb ligák)
  elveszik.

## 2. Döntések (jóváhagyva 2026-11-16)

1. **Lefedettség:** a Soccerway (és később a FotMob) a nap **összes
   versenyét** beolvassa, besorolással; a kliens alapnézete a **kiemelt
   kör** (priority ≤ 10), „Minden verseny" kapcsolóval.
2. **Architektúra (A):** job = **forrás × nap** (`domain = "day"`); a
   tick a `plan()` **minden** kérését feldolgozza; a parse után a napot
   **verseny szerint bontjuk**, és versenyenként materializálunk +
   gate-elünk. A `NormalizedDay` szerződés marad.
3. **Atomicitás:** bármely kérés végleges hibája → a job `failed`, a
   napra nincs kanonikus írás. Tranziens hiba → backoff/requeue (a
   megszokott út). Robots/kvóta általi blokk → pending requeue.
4. **Kvóta:** fetch-enként számol (változatlan). Büdget: Soccerway 6/nap
   (ma+tegnap ablak, retry-val), footballdata 25/nap (5 CSV × 2 ablak).
5. **Coverage:** egy slot/nap (`source, "day", discovery, ablak`).
6. **Kérések közti várakozás:** a tick a policy min_delay-ét a jobon
   belül kivárja (max 15 s); ennél hosszabb delay + többkérésű terv
   esetén a job pending requeue (dokumentált korlát; ma nincs ilyen
   forrás: Soccerway/FotMob 1 kérés, footballdata 2 s × 5).

## 3. Verseny-osztályozás

- **Ismert versenyek** (top-5 + NB I/II): kanonikus angol név, ISO
  ország, priority 6–10 — a footballdata soraival azonos entitás.
- **Ismeretlen verseny:** név = a fejléc megjelenített neve, ország =
  zászló angol `title` → ISO (kurált tábla; ismeretlen → NULL),
  priority 50; kupa-név (kupa/cup/copa/coupe) → tier 0.
- **Nem/korosztály heurisztika** (konzervatív): női/W → female/women;
  U17–U23/youth/ifjúsági → youth. Bizonytalannál male/senior.
- **Csapat-ország:** hazai liga → verseny országa; kupa/nemzetközi →
  NULL (a séma engedi); meglévő entitás országát a materializálás nem
  írja felül NULL-lal.
- **Review:** ismeretlen csapat → review-sor, a meccs kimarad; nincs
  fuzzy egyeztetés. Az első körökben a review-szám nőhet (elfogadott).

## 4. Változó komponensek

| Komponens | Változás |
|---|---|
| `worker::tick` | többkérésű job-loop, kérésenkénti kapuk, intra-job várakozás, csoportos normalizálás |
| `ingest::normalized` | `grouped_from_observations` (verseny-kulcs: ország+név+tier+priority+season) |
| `ingest::canonicalize` | nullable verseny/csapat ország; competition find NULL-safe; team upsert Option<country> |
| `ingest::soccerway` | minden csoport parse, osztályozás, 1 kérés |
| `ingest::fotmob` | minden liga, 1 kérés (tiltva marad) |
| `ingest::footballdata` | 1 job, 5 kérés (ligánkénti request_key megmarad) |
| policy-seed | `domains = ["day"]` (clubelo: `["elo"]`) |
| kliens | kiemelt kör alapból + „Minden verseny" kapcsoló |
| automatizálás | launchd agent fájlok + útmutató (telepítés külön jóváhagyással) |

## 5. Tesztelés / DoD

- Unit: grouping (2 verseny), NULL-ország materializálás (kupa),
  competition NULL-safe find, soccerway-osztályozás (ismert +
  ismeretlen + női/utánpótlás + kupa), footballdata 5 kéréses terv,
  fotmob minden liga, kliens kiemelt szűrő.
- Integráció: többkérésű tick (2 fixture-kérés, egy nap = 2 verseny);
  all-or-nothing hibaág; intra-job delay.
- Élő: Soccerway 1 render/nap, minden verseny besorolva (a mai nap
  szünet miatt `no_data` is elfogadható, de a render-szám 1).
- Teljes kapu: `check-foundation.sh`, workspace tesztek, clippy/fmt,
  app tesztek, teszt-DB reset.

## 6. Visszaállítási út

- Kód: branch-revert (nincs migráció; séma-változás nincs).
- Policy: a régi domains lista visszaállítható SQL-lel; a meglévő
  per-liga coverage/ledger historikus sorok megmaradnak.
- Kanonikus adat: az új nap-versenyek a megszokott forrás-kötött
  entitások; a review-sorok visszafordíthatók.