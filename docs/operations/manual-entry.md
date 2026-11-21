# Kézi adatbevitel a kliensből (S70)

A rendszer alapelve a forrásvezérelt, ellenőrzött adat. Előfordul azonban, hogy
egy eredmény egyik elérhető forrásból sem jön meg (kis liga, elavult forrás,
forrás-oldali hiba). Ilyenkor a felhasználó a **meccs-részletezőben** maga is
felveheti az eredményt — de nem „csak úgy": az írás ugyanolyan nyomot hagy,
mint egy forrásból származó revízió.

## Használat

1. Nyisd meg a meccs részletezőjét a napi listáról.
2. Kattints az **„Eredmény javítása"** gombra (`match.detail.manual`).
3. Add meg a hazai és vendég gólszámot (egész szám, >= 0) és egy **kötelező
   indoklást** (pl. „SofaScore 20/11: 2–1").
4. A **Mentés** (`manual.save`) után a részletező automatikusan újratölti a
   kanonikus adatot; a kapuállapotot az újraellenőrzés számolja ki.

## Technikai háttér

- **Végpont:** `PUT /v1/match/{id}/manual-result`, JSON body:
  `{"home_ft": 2, "away_ft": 1, "note": "…"}`.
- **Jogosultság:** csak **ops token** (a read token 401-et kap). A kliens a
  `runtime.json`-ban lévő ops tokent használja — ugyanazt, mint az Üzemeltetés
  nézet.
- **Forrás:** `ops.source.code = 'manual-ui'`.
- **Bizonyíték:** nyers JSON artifact (content-addressed) → `raw.capture` →
  `stage.observation` (`aspect='match_core'`) → `fact.provenance` a `result` és
  `status` mezőkre (`is_selected=true`, a régi selected sorok kikapcsolva).
- **Kanonikus írás:** `core.match_result` upsert (`decided_by='ft'`), a meccs
  státusza `finished`.
- **Audit:** `ops.audit_event`, actor `ui`, action `manual_result_entry`,
  részletben a gólszámokkal, indoklással és az observation azonosítóval.
- **Publikálási kapu:** a handler a meglévő `reverify_match` úton újraellenőrzi
  a meccset — a kézi út **nem kerüli meg** a verification láncot.

## Korlátok és visszaállítás

- Csak sima `FT` eredményt lehet felvenni. Hosszabbítás, tizenegyes, megítélt
  (awarded) eredmény a kanonikus, forrásvezérelt útra tartozik.
- A korábbi érték nem vész el: a régi provenance sorok `is_selected=false`-re
  kerülnek (a megfigyelések maguk megmaradnak), és az audit-event teljes
  indoklást tartalmaz. Visszaállítás = új, ellenkező értékű kézi bevitel vagy a
  forrás-oldali revízió újrafuttatása.
- A kliens `APIError.badRequest` üzenete jelenleg „Hibás dátum." — a kézi űrlap
  saját validációja (nem negatív gólszám, nem üres indoklás) előre kiszűri a
  hibás eseteket.

## ellenőrzés

```bash
cargo test -p football-api --test manual_result -- --test-threads=1
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path apps/FootballAppCore
```
