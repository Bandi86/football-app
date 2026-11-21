# Napi adatfrissítés — odds, eredmények, hiányok

A napi lista pontossága három, egymást kiegészítő folyamaton múlik: a research
projekt napi futásán, a v2 ingest catch-up jobjain és a hiány-panel visszajelzésén.

## 1. Research napi futás (külső projekt)

A `~/Library/Application Support/FootballResearch` projekt launchd feladatai:

- `com.footballresearch.daily` — 08:00: éjszakai eredmények, napi lista, odds,
  ellenőrzés, `api/v1` export.
- `com.omp.football` — 01:15 (`refresh yesterday`), 10:30/14:30/18:30
  (research-kör), 13:00 (`sync today`).

Az `api/v1` export kézi előállítása egy napra:

```bash
cd "$HOME/Library/Application Support/FootballResearch"
python3 tools/football.py fetch 2026-11-22     # napi lista (FotMob/ESPN/LiveScore)
python3 tools/football.py odds 2026-11-22      # 1X2/BTTS/OU odds (tipplap, BetExplorer, …)
python3 docs/api-export.py 2026-11-22          # api/v1/daily + api/v1/matches
```

A `odds` parancs a napi `raw/odds-<dátum>.json` pillanatképet írja, és a
meccs-detail fájlokba (`api/v1/matches/.../*.json`) teszi a piacokat; a v2
research-ingest innen olvassa az oddsot.

## 2. v2 ingest catch-up

A `footballd` schedulerje research forrásra naponta egyszer visszamenőleges
(7 napos) refresh jobokat tesz be (`catchup-<dátum>`), így a research friss
fájljai automatikusan bekerülnek a DB-be. Kézi pótlás egy napra:

```bash
psql "$DATABASE_URL" -c "INSERT INTO ops.job (id, source_slug, domain, entity_key, event_type, time_window, due_at, priority, status) VALUES (gen_random_uuid(), 'research', 'day', 'manual-refresh-2026-11-21', 'refresh', '2026-11-21', now(), 5, 'pending') ON CONFLICT DO NOTHING;"
cd rust && OBSCURA_BIN="$PWD/../football-app/tools/obscura/obscura" cargo run -q -p footballctl --features obscura -- worker tick
```

## 3. Odds-lefedettség értelmezése

A fogadóirodák jellemzően 1–3 nappal a meccs előtt teszik közzé az oddsot,
ezért a távolabbi napokon a kártyán „–" áll, és a hiány-panel „odds nélkül"
számlálója magas. Ez nem hiba: a napi catch-up a következő futásokkor pótolja.
2026-11-21-re a research odds pipeline 266 meccshez adott piacot (≈3 900
quote), a 11-22-i napra 41-hez; a hét további napjain 0–7.

## 4. Hiány-panel és audit

- A napi API `gaps` mezője naponta számolja a hiányokat (kezdésidő nélkül,
  lejárt-lezáratlan, eredmény nélkül, csapat-/ligalogó nélkül, odds nélkül,
  visszatartva).
- A `footballctl audit --from <d> --to <d>` a duplikátumokat és
  konzisztencia-hibákat ellenőrzi; találatnál exit 1.
- A >24 órás, eredmény nélküli meccsekre a research/livescore/FotMob nem ad
  adatot (utánpótlás-selejtezők, kis ligák); ezek a hiány-panelben maradnak,
  amíg egy új forrás nem fedi le őket.
