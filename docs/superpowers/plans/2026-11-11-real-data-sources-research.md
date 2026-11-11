# Valós adatforrások kutatása és katalógusa (S1) — megvalósítási terv

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A specben rögzített forrás-univerzum élő spike-próbákkal igazolt katalógussá álljon össze (hozzáférés, szállítás, mezőtérkép, lefedettség, bizalom, kvóta), ami az S2 ingest-keretrendszer spec bemenete.

**Architecture:** Eldobható Python3 stdlib spike-harness (`/tmp/s1/`) robots-kapuval, napi budget-loggal, tartalmi őrökkel és decoy-tesztekkel; minden spike csak nyers mintát ír `/tmp`-be, egyetlen repó-kimenet a `docs/research/source-catalog.md` élő katalógus és a napi checkpoint. Production kód, migráció, scheduler nem készül S1-ben.

**Tech Stack:** Python 3 (stdlib: urllib, json, csv, hashlib, re, time), macOS `security` CLI (Keychain), Obscura bináris csak meglévő v1 példányból (opt-in), PostgreSQL érintetlen.

**Spec:** `docs/superpowers/specs/2026-11-11-real-data-sources-research-design.md`

## Global Constraints

- AGENTS.md érvényes: magyar dokumentáció és hibaüzenetek; Write/Edit után a magyar szöveg visszaolvasása kötelező.
- Commit/push CSAK kifejezett felhasználói kérésre — a terv lépései nem tartalmaznak commitot; hullámonként checkpoint-jelentés és review-kapu van.
- Branch: a végrehajtás a `feat/source-research` ágon indul (a jelenlegi HEAD-ről létrehozva); nem a main.
- Spike-kód és nyers payload csak `/tmp/s1/`-ben; repóba payload nem kerül; méretkorlát 5 MB, timeout 30 s.
- Spike-ok nem írnak dev/teszt DB-be; kizárólag read-only GET/HEAD vagy dokumentált read-only API-hívás.
- Kutatási budget: forrásonként ≤ 10 kérés/nap (a robots.txt kérés is számít); delay a v1 értékeknél nem kisebb.
- 403/429 → azonnali stop (`StopSpike`), nincs retry; státusz-jelölés a katalógusban.
- Robots-tiltott útvonal nem kerül megkerülésre; `robots_override` és stealth tilos.
- Regisztrációs falnál stop + felhasználói kérdés; kulcsot nem hozunk létre engedély nélkül.
- OpenFoot: a v1 `.env` Bearer kulcsa read-only, 5000 kérés/hó keretben, Keychainbe migrálva; a felhasználó visszavonhatja.
- Hiányzó adat NULL/státusz, soha nem nulla; decoy-200 esetén parser-szintű tartalmi őr kötelező.
- Minden időpont UTC; a katalógus mezőtérképe a v2 sémára képez (`core.match`, `core.match_result`, `fact.odds_snapshot`, `core.standing`); rések jelölve.
- Új külső függőség nem telepíthető; Obscura bináris csak a v1 `tools/obscura/` példányából használható (ha nincs, a szállítási döntés halasztott indokkal).
- Spike-futtatás: `python3.13` (a rendszer `python3` 3.9-es robotparser-e nem kezeli a `*`/`$` robots-szabályokat; a harness 3.13 alatt fail-closed deny). A terv `python3` parancsai ennek megfelelően `python3.13`-ként értendők.

---

### Task 1: Spike-harness + katalógus-váz + tiltólista

**Files:**
- Create: `/tmp/s1/harness.py` (eldobható)
- Create: `/tmp/s1/test_harness.py` (eldobható)
- Create: `docs/research/source-catalog.md`

**Interfaces:**
- Produces: `fetch(source, url, delay, timeout=30, max_bytes=5_000_000) -> bytes`,
  `robots_allowed(source, base, path) -> (bool, str)`, `log_budget(source, url, status, nbytes)`,
  `budget_left(source) -> int`, `sha256_file(path) -> str`, `guard_rows(rows, label)` (üresnél `DecoyError`),
  `save_sample(source, name, data) -> Path`, `StopSpike`, `DecoyError` exceptionök.
  A katalógus fejezetei: `## Státusz-legenda`, `## Tiltólista`, `## Források` (bejegyzés-séma a spec 3.1 szerint).

- [ ] **Step 1: Harness offline bukó tesztek**

`/tmp/s1/test_harness.py`:

```python
import unittest
from harness import guard_rows, DecoyError, budget_left, log_budget

class GuardTests(unittest.TestCase):
    def test_empty_rows_raise_decoy(self):
        with self.assertRaises(DecoyError):
            guard_rows([], "understat-decoy")

    def test_nonempty_rows_pass(self):
        guard_rows([{"a": 1}], "ok")

    def test_budget_decrements(self):
        before = budget_left("unittest_source")
        log_budget("unittest_source", "https://example.invalid/x", 200, 10)
        self.assertEqual(budget_left("unittest_source"), before - 1)

if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Futtatás — piros**

Run: `cd /tmp/s1 && python3 test_harness.py`
Expected: FAIL, `ModuleNotFoundError: No module named 'harness'`.

- [ ] **Step 3: Harness implementáció**

`/tmp/s1/harness.py`:

```python
"""S1 spike-harness: robots-kapu, napi budget, tartalmi őr, minta-mentés. Eldobható."""
import hashlib
import json
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path("/tmp/s1")
BUDGET = ROOT / "budget.json"
SAMPLES = ROOT / "samples"
DAILY_LIMIT = 10

class StopSpike(Exception):
    """403/429/robots-tilt: azonnali stop, nincs retry."""

class DecoyError(Exception):
    """Üres/decoy payload: a 200 nem bizonyíték."""

def _load():
    if BUDGET.exists():
        return json.loads(BUDGET.read_text())
    return {}

def _today():
    return time.strftime("%Y-%m-%d", time.gmtime())

def budget_left(source):
    day = _load().get(_today(), {})
    return DAILY_LIMIT - day.get(source, 0)

def log_budget(source, url, status, nbytes):
    data = _load()
    day = data.setdefault(_today(), {})
    day[source] = day.get(source, 0) + 1
    BUDGET.write_text(json.dumps(data, indent=2))
    print(f"budget {source}: {day[source]}/{DAILY_LIMIT} {status} {nbytes}B {url}")

def fetch(source, url, delay, timeout=30, max_bytes=5_000_000):
    if budget_left(source) <= 0:
        raise StopSpike(f"{source}: napi budget elfogyott")
    time.sleep(delay)
    req = urllib.request.Request(url, headers={"User-Agent": "football-app-v2-research/1.0"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            data = resp.read(max_bytes + 1)
            status = resp.status
    except urllib.error.HTTPError as exc:
        log_budget(source, url, exc.code, 0)
        if exc.code in (403, 429):
            raise StopSpike(f"{source}: {exc.code} — stop, nincs retry") from exc
        raise
    if len(data) > max_bytes:
        raise StopSpike(f"{source}: payload > 5 MB")
    log_budget(source, url, status, len(data))
    return data

def robots_allowed(source, base, path):
    try:
        body = fetch(source, base.rstrip("/") + "/robots.txt", delay=2.5).decode("utf-8", "replace")
    except StopSpike:
        return False, "robots.txt nem elérhető (stop)"
    except Exception as exc:
        return False, f"robots.txt hiba: {exc}"
    disallows = []
    for line in body.splitlines():
        line = line.split("#", 1)[0].strip()
        if line.lower().startswith("disallow:"):
            rule = line.split(":", 1)[1].strip()
            if rule:
                disallows.append(rule)
    for rule in disallows:
        if path.startswith(rule):
            return False, f"robots tilt: {rule}"
    return True, "robots engedett" if disallows else "robots üres/engedett"

def guard_rows(rows, label):
    if not rows:
        raise DecoyError(f"{label}: üres/decoy payload — 200 nem bizonyíték")
    return rows

def save_sample(source, name, data):
    directory = SAMPLES / source
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / name
    path.write_bytes(data)
    return path

def sha256_file(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()
```

- [ ] **Step 4: Futtatás — zöld**

Run: `cd /tmp/s1 && python3 test_harness.py`
Expected: OK (3 teszt).

- [ ] **Step 5: Katalógus-váz + tiltólista**

`docs/research/source-catalog.md` (magyar; a spec 3.1 sémájával):

```markdown
# Forráskatalógus (S1) — élő dokumentum

Frissítés: minden spike után; státusz csak bizonyítékkal változik.
Spec: docs/superpowers/specs/2026-11-11-real-data-sources-research-design.md

## Státusz-legenda

- `javasolt` — leltárban szerepel, spike még nem futott.
- `igazolt` — spike zöld: robots, budget, mezőtérkép, lefedettség, tartalmi őr, bizalom, szállítás.
- `elvetett` — indoklással (robots/ToS/blokk/decoy/licenc).
- `regisztráció szükséges` — auth-fal; spike megállt, felhasználói döntés kell.
- `halasztott` — próba-szintű adat megvan, teljes igazolás a saját adapter-szeletben.

## Bejegyzés-séma

slug / név / base URL / típus; hozzáférés + auth (Keychain); szállítás
(rust-http | obscura | chrome-fallback) + bizonyíték; domainek + mezőtérkép
(v2 tábla.oszlop, típus, null, időzóna) + séma-rések; demo-kör lefedettség;
identitás (ID-stabilitás, crosswalk, alias); kvóta/robots (budget, delay, breaker);
frissesség-javaslat; bizalom domainenként; v1-érettség + kötelező őrök;
kockázatok + mitigáció; státusz + bizonyíték (spike-log dátum, sha256, v1-doc);
adoptációs rang.

## Tiltólista (elvetett, v1-bizonyítékkal)

| forrás | indok | bizonyíték |
|---|---|---|
| eredmenyek.com | fabrikált eredmény hamis provenance-szal | v1 docs/fix-log-2026-11-05-fabrikalt-golszamok.md |
| SofaScore | IP-ban, robots.txt is 403 | v1 docs/obscura-source-matrix-2026-11-09.md |
| WhoScored | Cloudflare-tiltás mindkét szállítással | v1 obscura-mátrix |
| UEFA.com | hálózati/IP blokk | v1 obscura-mátrix |
| Understat | decoy-200 (üres oldal 200-cal) | v1 obscura-mátrix 2. pont |
| FlashScore | robots tiltja a hasznos útvonalakat | v1 obscura-mátrix |
| kicker.de | robots tilt | v1 obscura-mátrix |
| BBC Sport | ToS tiltja a scrape-et | v1 obscura-mátrix |
| FBref | ToS + Cloudflare | v1 docs/web-scraping-architecture.md |

## Források

(minden spike-task ide fűzi a bejegyzését)
```

- [ ] **Step 6: Magyar szöveg visszaolvasása + checkpoint**

Run: `grep -n "Státusz-legenda\|Tiltólista\|bizonyíték" docs/research/source-catalog.md`
Expected: a fejezetek ékezetesen jelennek meg.
Checkpoint-bejegyzés: Task 1 kész, harness-tesztek zöldek, katalógus-váz létrejött.

---

### Task 2: W1 — football-data.co.uk spike

**Files:**
- Create: `/tmp/s1/probe_footballdata.py`
- Modify: `docs/research/source-catalog.md` (bejegyzés)

**Interfaces:**
- Consumes: Task 1 harness (`fetch`, `robots_allowed`, `guard_rows`, `save_sample`, `sha256_file`).
- Produces: katalógus-bejegyzés `footballdata` státusszal; mezőtérkép a v1 `sources/footballdata.py` oszlopai szerint; séma-rés lista (pl. `match_stats`, `referees`, bookmakerenkénti nyitó/záró odds táblák).

- [ ] **Step 1: Robots-kapu**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('footballdata', 'https://www.football-data.co.uk', '/mmz4281/'))"`
Expected: `(True, ...)` — a v1 szerint robots teljesen engedett; ha False → StopSpike-jegyzet, státusz elvetett.

- [ ] **Step 2: Probe szkript (assertionökkel)**

`/tmp/s1/probe_footballdata.py`:

```python
import csv
import io
from harness import fetch, guard_rows, save_sample, sha256_file

BASE = "https://www.football-data.co.uk"
# demo kör: E0 (PL), SP1 (La Liga), I1 (Serie A), D1 (Bundesliga), F1 (Ligue 1)
for div in ["E0", "SP1", "I1", "D1", "F1"]:
    data = fetch("footballdata", f"{BASE}/mmz4281/2627/{div}.csv", delay=4.0)
    path = save_sample("footballdata", f"2627_{div}.csv", data)
    rows = list(csv.DictReader(io.StringIO(data.decode("utf-8", "replace"))))
    guard_rows(rows, f"footballdata {div}")
    need = ["Date", "HomeTeam", "AwayTeam", "FTHG", "FTAG", "HTHG", "HTAG",
            "HS", "AS", "HC", "AC", "HY", "AY", "HR", "AR", "Referee",
            "B365H", "B365D", "B365A", "PSH", "PSD", "PSA", "AvgH", "AvgD", "AvgA"]
    missing = [c for c in need if c not in rows[0]]
    assert not missing, f"hiányzó oszlopok {div}: {missing}"
    print(div, len(rows), "sor; sha256:", sha256_file(path))
print("MEZŐTÉRKÉP: Date/HomeTeam/AwayTeam -> core.match.kickoff_at/home/away (alias kell);")
print("FTHG/FTAG/HTHG/HTAG -> core.match_result.home_ft/away_ft/home_ht/away_ht;")
print("HS/AS/HC/AC/HY/AY/HR/AR -> séma-rés: match_stats; Referee -> séma-rés: referees;")
print("B365*/PS*/Avg* -> fact.odds_snapshot bővítés (bookmaker, opening/closing) séma-rés.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_footballdata.py`
Expected: 5 liga sor-számmal és sha256-tal; assertion nem bukik. Ha valamelyik oszlop hiányzik
(a szezon elején üres CSV lehet): `guard_rows` DecoyError → státusz `javasolt` marad,
jegyzet „szezonális üresség”, nem elvetett.

- [ ] **Step 4: Dekoy-teszt offline**

Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'footballdata-decoy'); raise SystemExit('FAIL: guard nem bukott')
except DecoyError:
    print('OK: decoy guard bukik üres payloadon')"`
Expected: OK.

- [ ] **Step 5: Katalógus-bejegyzés**

A `## Források` alá bejegyzés a sémával: típus archívum; hozzáférés none; szállítás rust-http
(CSV, spike-bizonyíték); domainek fixture/eredmény/stats/odds; lefedettség: demo kör mind az 5
ligája megvan (NB I/NB II **hiányzik** → rés-jelölés, Tipplap/fotmob pótolja); kvóta 200/nap,
delay 4+3 s; bizalom: eredmény/tabella magas, odds kétforrásos; őrök: üres CSV guard, aliased
archív kódok tiltva (v1 sources/footballdata.py:79); státusz a spike eredménye szerint;
sha256 a mintákról; adoptációs rang 1.

- [ ] **Step 6: Visszaolvasás + checkpoint**

Run: `grep -n "footballdata" docs/research/source-catalog.md | head -5`
Checkpoint: futtatott parancsok, sor-számok, sha256, státusz, séma-rések.

---

### Task 3: W1 — TheSportsDB spike (stadion/referee is)

**Files:**
- Create: `/tmp/s1/probe_thesportsdb.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness.
- Produces: `thesportsdb` bejegyzés; stadion/referee mezők a v2 `core.match` séma-rés listájára; a „3” publikus kulcs mai feltételeinek jegyzete (regisztrációs fal esetén státusz `regisztráció szükséges`).

- [ ] **Step 1: Robots + feltételek**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('thesportsdb', 'https://www.thesportsdb.com', '/api/v1/json/'))"`
Expected: engedett (API intended-use); jegyzet a kulcs feltételeiről a docs-ból.

- [ ] **Step 2: Probe**

`/tmp/s1/probe_thesportsdb.py`:

```python
import json
from harness import fetch, guard_rows, save_sample, sha256_file

BASE = "https://www.thesportsdb.com/api/v1/json/3"
data = fetch("thesportsdb", f"{BASE}/eventsseason.php?id=4328&s=2026-2027", delay=2.0)
path = save_sample("thesportsdb", "eventsseason_4328.json", data)
payload = json.loads(data)
events = guard_rows(payload.get("events") or [], "thesportsdb events")
need = ["idEvent", "strEvent", "dateEvent", "strTime", "strHomeTeam", "strAwayTeam",
        "intHomeScore", "intAwayScore", "strVenue", "strReferee", "strStatus", "intRound"]
missing = [k for k in need if k not in events[0]]
assert not missing, f"hiányzó mezők: {missing}"
print(len(events), "esemény; sha256:", sha256_file(path))
print("MEZŐTÉRKÉP: idEvent -> külső ID (identity crosswalk); dateEvent+strTime -> kickoff_at UTC;")
print("strVenue/strReferee -> séma-rés: core.match.stadium/referee; intRound -> round_label.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_thesportsdb.py`
Expected: eseményszám + sha256. Ha a kulcs már regisztrációhoz kötött (401/403 vagy üres
events + hibaüzenet): státusz `regisztráció szükséges`, stop, felhasználói kérdés.

- [ ] **Step 4: Dekoy-teszt**

Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'thesportsdb-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`

- [ ] **Step 5: Katalógus-bejegyzés + visszaolvasás + checkpoint**

Bejegyzés: típus API; hozzáférés publikus kulcs (feltétel-jegyzet); szállítás rust-http;
domainek fixture/eredmény/stadion/referee; lefedettség: top-5 megvan, NB I/NB II ellenőrzendő
league-id kereséssel (külön kérés a budgeten belül); bizalom fixture magas, eredmény közepes
(státusz-mező pontossága spike-jegyzet); kvóta 40/nap, 2+1 s; rang 2.

---

### Task 4: W1 — Tipplap spike (dátum-guarddal)

**Files:**
- Create: `/tmp/s1/probe_tipplap.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `scrappers/tipplap/client.py` mezőlista (Match: id, match_date, league, home/away, kickoff, h/d/v_odds, h/d/v_votes, min_combi, event_expire, more_markets).
- Produces: `tipplap` bejegyzés; **dátum-eltérés guard** teszttel; HU↔EN alias-igény lista; odds kétforrásos bizalmi jegyzet.

- [ ] **Step 1: Robots**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('tipplap', 'https://www.tipplap.hu', '/kalkulator/'))"`

- [ ] **Step 2: Probe dátum-guarddal**

`/tmp/s1/probe_tipplap.py`:

```python
import json
import re
from datetime import date, timedelta
from harness import fetch, guard_rows, save_sample, sha256_file, StopSpike

BASE = "https://www.tipplap.hu/kalkulator/"
today = date.today().isoformat()
data = fetch("tipplap", BASE, delay=0.3)
path = save_sample("tipplap", f"day_{today}.html", data)
text = data.decode("utf-8", "replace")
# dátum-guard: a mai oldalnak a mai dátumot kell tartalmaznia (v1 TipplapDateMismatch minta)
assert today in text or date.today().strftime("%Y.%m.%d") in text, "dátum-eltérés: stop"
markets = re.findall(r'"(?:h|d|v)_odds"\s*:\s*([0-9.]+)', text)
guard_rows(markets, "tipplap odds")
print(len(markets), "odds-érték; sha256:", sha256_file(path))
# múlt-napi oldal probe: ha ma-listát szolgál, a guardnak buknia kell
past = (date.today() - timedelta(days=3)).isoformat()
past_data = fetch("tipplap", BASE + f"?date={past}", delay=0.3)
past_text = past_data.decode("utf-8", "replace")
mismatch = today in past_text and past not in past_text
print("múlt-napi oldal ma-listát szolgál:", mismatch, "(guard kötelező)")
print("MEZŐTÉRKÉP: h/d/v_odds -> fact.odds_snapshot odds.home/draw/away (snapshot_kind opening/closing);")
print("league/home/away HU nevek -> alias tábla igény (HU<->EN); more_markets -> piacok bővítés séma-rés.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_tipplap.py`
Expected: odds-szám + sha256 + mismatch-jegyzet. Ha a mai oldal nem tartalmazza a mai dátumot:
StopSpike-jegyzet, státusz `javasolt`, guard-élesítés jegyzet (nem elvetett).

- [ ] **Step 4: Dekoy-teszt**

Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'tipplap-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`

- [ ] **Step 5: Katalógus-bejegyzés + visszaolvasás + checkpoint**

Bejegyzés: típus aggregator (HU); hozzáférés none; szállítás rust-http (HTML+JSON fragment);
domainek odds/fixture; lefedettség: demo kör + NB I/NB II + kupák ellenőrzése a ligalistából;
bizalom odds kétforrásos (football-data closing a párja); őrök: dátum-guard, üres odds guard;
kvóta: 0,3 s delay (v1), napi 3 futás + closing; rang 3.

---

### Task 5: W1 — FotMob spike (szállítási döntéssel)

**Files:**
- Create: `/tmp/s1/probe_fotmob.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `sources/fotmob.py` `__NEXT_DATA__` útvonalak.
- Produces: `fotmob` bejegyzés; szállítási döntés (rust-http vs obscura) spike-bizonyítékkal; lineups/xG/stats mezőtérkép; séma-rések (`match_lineups`, `player_availability`, `match_team_stats`, `match_incidents`, `season_team_stats`).

- [ ] **Step 1: Robots**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('fotmob', 'https://www.fotmob.com', '/matches'))"`
Expected: `/matches` engedett, `/api/*` tiltott → csak HTML út.

- [ ] **Step 2: Probe**

`/tmp/s1/probe_fotmob.py`:

```python
import json
import re
from datetime import date
from harness import fetch, guard_rows, save_sample, sha256_file

BASE = "https://www.fotmob.com"
day = date.today().isoformat()
data = fetch("fotmob", f"{BASE}/matches?date={day}", delay=6.0)
path = save_sample("fotmob", f"matches_{day}.html", data)
text = data.decode("utf-8", "replace")
m = re.search(r'<script id="__NEXT_DATA__" type="application/json">(.*?)</script>', text, re.S)
assert m, "nincs __NEXT_DATA__ — szállítási döntés: obscura probe kell"
payload = json.loads(m.group(1))
save_sample("fotmob", f"matches_{day}.json", json.dumps(payload).encode())
leagues = payload.get("props", {}).get("pageProps", {}).get("leagues") or []
guard_rows(leagues, "fotmob leagues")
print(len(leagues), "liga; sha256:", sha256_file(path))
print("MEZŐTÉRKÉP: league/match id -> külső ID; status/kickoff -> core.match;")
print("lineups/missing/xG/stats -> séma-rés táblák (S2); standings row -> core.standing keresztforrás.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_fotmob.py`
Expected: liga-szám + sha256. Ha nincs `__NEXT_DATA__` plain HTTP-vel: Obscura probe a v1
meglévő mérőscriptjével (read-only futtatás, a v1 környezet függőségeivel):

Run: `cd ../football-app && python3 scripts/probe_obscura_transport.py --url "https://www.fotmob.com/matches?date=$(date +%Y-%m-%d)" 2>&1 | tail -20`
Expected: a script a production parserekkel összeveti a curl/obscura kimenetet; a FotMob
`__NEXT_DATA__` jelenlétét az obscura ágon kell igazolni. Ha a v1 bináris (`tools/obscura/`)
vagy a script függőségei hiányoznak → szállítás `halasztott` indokkal, státusz `javasolt`;
telepítés csak külön felhasználói engedéllyel.

- [ ] **Step 4: Dekoy-teszt**

Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'fotmob-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`

- [ ] **Step 5: Katalógus-bejegyzés + visszaolvasás + checkpoint**

Bejegyzés: típus aggregator; hozzáférés none; szállítás a spike döntése szerint; domainek
lineups/hiányzók/xG/stats/tabella; lefedettség demo kör; bizalom közepes (keresztforrás);
őrök: `__NEXT_DATA__` guard, séma-verzió jegyzet; kvóta 200/nap, 6+6 s; rang 4.

---

### Task 6: W1 — Transfermarkt spike (tartalmi őrrel)

**Files:**
- Create: `/tmp/s1/probe_transfermarkt.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `sources/transfermarkt.py` `_has_table` minta, season-id quirkek.
- Produces: `transfermarkt` bejegyzés; hiányzók/góllövők mezőtérkép (`player_availability`, `season_player_stats` séma-rés).

- [ ] **Step 1: Robots**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('transfermarkt', 'https://www.transfermarkt.com', '/premier-league/verletztespieler/'))"`

- [ ] **Step 2: Probe**

`/tmp/s1/probe_transfermarkt.py`:

```python
import re
from harness import fetch, guard_rows, save_sample, sha256_file

BASE = "https://www.transfermarkt.com"
url = f"{BASE}/premier-league/verletztespieler/wettbewerb/GB1"
data = fetch("transfermarkt", url, delay=8.0)
path = save_sample("transfermarkt", "verletztespieler_GB1.html", data)
text = data.decode("utf-8", "replace")
# tartalmi őr: a v1 _has_table mintája — üres/decoy oldalnak buknia kell
rows = re.findall(r'<td class="hauptlink">(.*?)</td>', text, re.S)
guard_rows(rows, "transfermarkt injuries")
print(len(rows), "hiányzó-sor; sha256:", sha256_file(path))
print("MEZŐTÉRKÉP: name/status/reason -> player_availability (séma-rés);")
print("góllövők: goals/apps/minutes -> season_player_stats (séma-rés); season-id quirkek jegyzet.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_transfermarkt.py`
Expected: sor-szám + sha256; DecoyError esetén státusz `javasolt` + guard-jegyzet.

- [ ] **Step 4: Dekoy-teszt** (a Step 2 guard_rows ága offline is)

Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'tm-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`

- [ ] **Step 5: Katalógus-bejegyzés + visszaolvasás + checkpoint**

Bejegyzés: típus aggregator; hozzáférés none; szállítás rust-http; domainek hiányzók/góllövők;
lefedettség ~25 liga (v1 registry), demo kör ellenőrzés; bizalom közepes, display-only;
őrök: `_has_table` tartalmi őr; kvóta 40/nap, 8+8 s; rang 5.

---

### Task 7: W1 — worldfootball.net spike (kupa-gerinc)

**Files:**
- Create: `/tmp/s1/probe_worldfootball.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `sources/worldfootball.py` selectorok (`data-match_id`, `data-datetime`, `.match-result`, `pso`).
- Produces: `worldfootball` bejegyzés; UEFA-kupa lefedettség; **FA Kupa rés** explicit jelölés (hazai kupák nincsenek → W2 probe).

- [ ] **Step 1: Robots**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('worldfootball', 'https://www.worldfootball.net', '/competition/'))"`

- [ ] **Step 2: Probe**

`/tmp/s1/probe_worldfootball.py`:

```python
import re
from harness import fetch, guard_rows, save_sample, sha256_file

BASE = "https://www.worldfootball.net"
url = f"{BASE}/competition/co19/uefa-champions-league-2026-2027/all-matches/"
data = fetch("worldfootball", url, delay=8.0)
path = save_sample("worldfootball", "cl_2026_27.html", data)
text = data.decode("utf-8", "replace")
ids = re.findall(r'data-match_id="(\d+)"', text)
guard_rows(ids, "worldfootball matches")
print(len(ids), "meccs; sha256:", sha256_file(path))
print("MEZŐTÉRKÉP: data-match_id -> külső ID; data-datetime -> kickoff_at UTC;")
print(".match-result -> FT; pso -> pens observed státusz; FA Kupa: NINCS (rés, W2 probe).")
```

- [ ] **Step 3: Futtatás + decoy-teszt**

Run: `cd /tmp/s1 && python3 probe_worldfootball.py`
Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'wf-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`

- [ ] **Step 4: Katalógus-bejegyzés + visszaolvasás + checkpoint**

Bejegyzés: típus archívum; hozzáférés none; szállítás rust-http; domainek kupa-fixture/eredmény;
lefedettség UEFA-kupák megvan, hazai kupák rés; bizalom eredmény magas (hivatalos közeli);
őrök: üres lista guard, pso observed; kvóta 40/nap, 8+8 s, 1 kérés/verseny/szezon; rang 6.
**W1 záró checkpoint:** mind a 6 gerincforrás státusza + séma-rés lista eddigi elemei.

---

### Task 8: W2 — logók/zászlók (GitHub + Wikimedia)

**Files:**
- Create: `/tmp/s1/probe_logos.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness.
- Produces: `logos-github`, `logos-wikimedia` bejegyzések; licenc-jegyzet (attribution); FotMob CDN csak jegyzet (CDN-hotlink kockázat), FM MegaPack licenc-ellenőrzés jegyzet.

- [ ] **Step 1: Probe**

`/tmp/s1/probe_logos.py`:

```python
import json
from harness import fetch, guard_rows, save_sample, sha256_file

gh = fetch("logos-github", "https://raw.githubusercontent.com/luukhopman/football-logos/master/logos/england/arsenal.png", delay=2.0)
p1 = save_sample("logos-github", "arsenal.png", gh)
assert gh[:8] == b"\x89PNG\r\n\x1a\n", "nem PNG"
wd = fetch("logos-wikimedia", "https://commons.wikimedia.org/w/api.php?action=query&titles=File:Arsenal_FC.svg&prop=imageinfo&iiprop=url|extmetadata&format=json", delay=2.0)
p2 = save_sample("logos-wikimedia", "arsenal_meta.json", wd)
meta = json.loads(wd)
pages = meta.get("query", {}).get("pages", {})
guard_rows(list(pages.values()), "wikimedia pages")
print("PNG sha256:", sha256_file(p1), "| wikimedia sha256:", sha256_file(p2))
print("LICENC: GitHub repo licenc + Wikimedia extmetadata LicenseShortName ellenőrzés a mintából.")
```

- [ ] **Step 2: Futtatás**

Run: `cd /tmp/s1 && python3 probe_logos.py`
Expected: PNG assert + sha256-k; licenc-mező kiolvasása a mintából (ha hiányzik → jegyzet, státusz javasolt).

- [ ] **Step 3: Katalógus-bejegyzések + visszaolvasás + checkpoint**

Két bejegyzés: típus open-data/CDN; hozzáférés none; szállítás rust-http; domain logó/zászló;
bizalom display-only; licenc/attribution kötelező jegyzet; rang 7.

---

### Task 9: W2 — OpenFoot spike (Keychain-kulccsal, 5000/hó)

**Files:**
- Create: `/tmp/s1/probe_openfoot.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `.env` `OPENFOOT_API_KEY` (csak olvasás); macOS `security` CLI.
- Produces: `openfoot` bejegyzés; havi keret-log (`/tmp/s1/openfoot_month.json`); séma-rés: lineups/xG plan-gate 403.

- [ ] **Step 1: Kulcs Keychainbe (felhasználói engedély a tervvel adott)**

Run: `KEY=$(grep '^OPENFOOT_API_KEY=' ../football-app/.env | cut -d= -f2-); security add-generic-password -a football-app-v2 -s openfoot-api-key -w "$KEY" -U`
Expected: exit 0. Ha a v1 `.env`-ben nincs kulcs → státusz `regisztráció szükséges`, stop + kérdés.
Megjegyzés: a kulcs értékét nem naplózzuk, nem írjuk fájlba.

- [ ] **Step 2: Probe**

`/tmp/s1/probe_openfoot.py`:

```python
import json
import subprocess
import urllib.request
from harness import guard_rows, save_sample, sha256_file, log_budget, StopSpike, budget_left

key = subprocess.run(["security", "find-generic-password", "-a", "football-app-v2",
                      "-s", "openfoot-api-key", "-w"], capture_output=True, text=True, check=True).stdout.strip()
url = "https://openfootapi.com/v1/matches?date=2026-11-11"
if budget_left("openfoot") <= 0:
    raise StopSpike("openfoot: napi budget elfogyott")
req = urllib.request.Request(url, headers={"Authorization": f"Bearer {key}"})
with urllib.request.urlopen(req, timeout=30) as resp:
    data = resp.read(5_000_001)
    status = resp.status
log_budget("openfoot", url, status, len(data))
path = save_sample("openfoot", "matches_2026-11-11.json", data)
payload = json.loads(data)
matches = guard_rows(payload.get("matches") or payload.get("data") or [], "openfoot matches")
print(len(matches), "meccs; sha256:", sha256_file(path))
print("KERET: 5000/hó; a havi számláló az openfoot_month.json-ban vezetendő (S2 worker).")
print("MEZŐTÉRKÉP: fixture + FT eredmény -> core.match/core.match_result; lineups/xG 403 plan-gate -> séma-rés.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_openfoot.py`
Expected: meccsszám + sha256. 401/403 → StopSpike, státusz `regisztráció szükséges`/elvetett indokkal.

- [ ] **Step 4: Dekoy-teszt + katalógus-bejegyzés + visszaolvasás + checkpoint**

Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'openfoot-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`
Bejegyzés: típus API; hozzáférés Bearer (Keychain); szállítás rust-http; domain fixture/eredmény;
bizalom közepes (keresztforrás); read-only fegyelem az incidens miatt (v1 fix-log 2026-11-07);
kvóta 5000/hó ≈ 166/nap; rang 8.

---

### Task 10: W2 — Livescore MCP spike (becsületes robots-ellenőrzéssel)

**Files:**
- Create: `/tmp/s1/probe_livescore.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness.
- Produces: `livescore-mcp` bejegyzés; robots-ítélet az API útvonalra; ha tiltott → felhasználói döntési pont (nem csendes megkerülés).

- [ ] **Step 1: Robots az API útvonalon**

Run: `cd /tmp/s1 && python3 -c "from harness import robots_allowed; print(robots_allowed('livescore-mcp', 'https://livescoremcp.com', '/mcp'))"`
Expected: engedett (intended-use API). Ha tiltott: státusz `elvetett` helyett **döntési pont** jegyzet + stop, felhasználói kérdés.

- [ ] **Step 2: Probe (JSON-RPC tools/call)**

`/tmp/s1/probe_livescore.py`:

```python
import json
import urllib.request
from harness import guard_rows, save_sample, sha256_file, log_budget, budget_left, StopSpike

url = "https://livescoremcp.com/mcp"
body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
                   "params": {"name": "get_matches_by_date", "arguments": {"date": "2026-11-11"}}}).encode()
if budget_left("livescore-mcp") <= 0:
    raise StopSpike("livescore: napi budget elfogyott")
req = urllib.request.Request(url, data=body, headers={"Content-Type": "application/json"})
with urllib.request.urlopen(req, timeout=30) as resp:
    data = resp.read(5_000_001)
    status = resp.status
log_budget("livescore-mcp", url, status, len(data))
path = save_sample("livescore-mcp", "matches_2026-11-11.json", data)
payload = json.loads(data)
content = payload.get("result", {}).get("content") or []
guard_rows(content, "livescore content")
print("tools/call válasz; sha256:", sha256_file(path))
print("MEZŐTÉRKÉP: napi fixture/eredmény + lineups/incidents/stats -> séma-rés táblák;")
print("a tools/list válaszából a pontos tool-nevek a katalógusba kerülnek (első hívás tools/list).")
```

Megjegyzés: ha a `get_matches_by_date` tool-név nem egyezik, először `tools/list` hívás
(a budgeten belül), majd a valódi névvel újra — a szkript ezt a sorrendet követi.

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_livescore.py`
Expected: válasz + sha256; 403/429 → StopSpike + státusz.

- [ ] **Step 4: Dekoy-teszt + katalógus-bejegyzés + visszaolvasás + checkpoint**

Bejegyzés: típus API (JSON-RPC); hozzáférés none; szállítás rust-http; domainek fixture/eredmény/
lineups/incidents/stats; bizalom közepes; kvóta 100/nap, 6 s; **robots_override nem kerül át**; rang 9.

---

### Task 11: W2 — PL-hivatalos hiányzók + ESPN újraértékelés

**Files:**
- Create: `/tmp/s1/probe_pl_official.py`, `/tmp/s1/probe_espn.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `docs/espn-source-2026-11-09.md` (11% linkelhetőség, AET/pen skip).
- Produces: `pl-official` és `espn` bejegyzések; ESPN linkelhetőség-mérés (csapatnév-egyezés a football-data mintával); AET/pen rés-jelölés.

- [ ] **Step 1: PL-hivatalos probe**

`/tmp/s1/probe_pl_official.py`:

```python
import re
from harness import fetch, robots_allowed, guard_rows, save_sample, sha256_file

ok, note = robots_allowed("pl-official", "https://www.premierleague.com", "/news/")
print("robots:", ok, note)
if not ok:
    raise SystemExit("robots tilt — státusz elvetett")
data = fetch("pl-official", "https://www.premierleague.com/news/4450606", delay=20.0)
path = save_sample("pl-official", "injuries_4450606.html", data)
text = data.decode("utf-8", "replace")
names = re.findall(r'<h3[^>]*>(.*?)</h3>', text, re.S)
guard_rows(names, "pl injuries")
print(len(names), "név-blokk; sha256:", sha256_file(path))
```

- [ ] **Step 2: ESPN probe + linkelhetőség**

`/tmp/s1/probe_espn.py`:

```python
import json
import re
from harness import fetch, robots_allowed, guard_rows, save_sample, sha256_file

ok, note = robots_allowed("espn", "https://www.espn.com", "/soccer/scoreboard")
print("robots:", ok, note)
if not ok:
    raise SystemExit("robots tilt — státusz elvetett")
data = fetch("espn", "https://www.espn.com/soccer/scoreboard?date=20261111", delay=6.0)
path = save_sample("espn", "scoreboard_20261111.html", data)
text = data.decode("utf-8", "replace")
m = re.search(r'__espnfitt__\s*=\s*(\{.*?\});?\s*</script>', text, re.S)
assert m, "nincs __espnfitt__ — obscura probe vagy halasztás"
payload = json.loads(m.group(1))
save_sample("espn", "scoreboard_20261111.json", json.dumps(payload).encode())
events = payload.get("page", {}).get("content", {}).get("scoreboard", {}).get("events") or []
guard_rows(events, "espn events")
print(len(events), "esemény; sha256:", sha256_file(path))
print("LINKELHETŐSÉG: csapatnevek összevetése a footballdata-mintával (S2 identity bemenet);")
print("AET/pen: 90-perces konvenció -> rés-jelölés (ET/pens csak explicit forrásból).")
```

- [ ] **Step 3: Futtatás mindkettő**

Run: `cd /tmp/s1 && python3 probe_pl_official.py && python3 probe_espn.py`
Expected: név-blokkok + eseményszám + sha256-k; robots-tilt esetén elvetett státusz.

- [ ] **Step 4: Dekoy-tesztek + katalógus-bejegyzések + visszaolvasás + checkpoint**

Két bejegyzés: pl-official (hivatalos, magas bizalom hiányzókra, PL only, 30/nap 20+25 s),
espn (aggregator, közepes; linkelhetőség % a mérésből; AET/pen rés; rang 10–11).

---

### Task 12: W2 — FA Kupa-rés probe (thefa / Sky / Soccerway / Tipplap)

**Files:**
- Create: `/tmp/s1/probe_facup.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; Task 4 Tipplap minta (kupák listája).
- Produces: FA Kupa lefedettségi döntés-előkészítés: melyik forrás fedi, milyen szállítással; `facup-gap` katalógus-jegyzet a jelöltek státuszaival.

- [ ] **Step 1: Robots-kapuk**

Run: `cd /tmp/s1 && python3 -c "
from harness import robots_allowed
for src, base, path in [('thefa', 'https://www.thefa.com', '/competitions'),
                        ('sky', 'https://www.skysports.com', '/football/fixtures'),
                        ('soccerway', 'https://int.soccerway.com', '/matches/')]:
    print(src, robots_allowed(src, base, path))"`

- [ ] **Step 2: Probe a engedett forrásokra**

`/tmp/s1/probe_facup.py`:

```python
import re
from harness import fetch, guard_rows, save_sample, sha256_file

# csak a robots-engedett források kerülnek ide; a tiltottak elvetett státuszt kapnak
targets = [
    ("sky", "https://www.skysports.com/football/fixtures", 6.0, r'fixres__item|match-head'),
    ("soccerway", "https://int.soccerway.com/matches/", 8.0, r'data-testid'),
]
for src, url, delay, pattern in targets:
    data = fetch(src, url, delay=delay)
    path = save_sample(src, "facup_probe.html", data)
    text = data.decode("utf-8", "replace")
    hits = re.findall(pattern, text)
    guard_rows(hits, f"{src} facup")
    facup = "FA Cup" in text or "fa-cup" in text.lower()
    print(src, len(hits), "találat; FA Cup jelen:", facup, "; sha256:", sha256_file(path))
print("DÖNTÉS-ELŐKÉSZÍTÉS: a FA Cup-ot fedő forrás + szállítás (Soccerway csak obscurával, 16 s);")
print("ha egyik sem fedi: a Tipplap kup-listája (Task 4 minta) az elsődleges jelölt.")
```

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_facup.py`
Expected: forrásonként találat-szám + FA Cup jelenlét. Ha a Soccerway plain HTTP-vel üres
(decoy-guard bukik): Obscura probe a v1 mérőscriptjével:

Run: `cd ../football-app && python3 scripts/probe_obscura_transport.py --url "https://int.soccerway.com/matches/" 2>&1 | tail -20`
Expected: a v1 mátrix szerint obscurával 1901 data-testid vs 18 curl-lel; ha a bináris vagy a
függőségek hiányoznak → Soccerway `halasztott` indokkal, és a FA Kupa döntés-előkészítés a
Tipplap kup-listájára (Task 4 minta) támaszkodik.

- [ ] **Step 4: Katalógus-jegyzet + visszaolvasás + checkpoint**

`facup-gap` bejegyzés: jelöltek státuszai, fedettség, szállítási költség; döntés-előkészítés
az S1 kilépési review-ra (melyik forrás kapja a FA Kupát).

---

### Task 13: W3 — mélység-probe-ok (StatsBomb, reep, ClubElo, Wikidata, FM27)

**Files:**
- Create: `/tmp/s1/probe_depth.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness; v1 `sources/statsbomb.py`, `sources/reep_load.py`, `sources/clubelo.py`.
- Produces: öt bejegyzés próba-szintű státusszal (`igazolt` ha a probe teljes, különben `halasztott` indokkal); licenc-jegyzetek (StatsBomb attribution/non-commercial, reep CC0, FM27 licencelt).

- [ ] **Step 1: Probe**

`/tmp/s1/probe_depth.py`:

```python
import json
from harness import fetch, guard_rows, save_sample, sha256_file

sb = fetch("statsbomb", "https://raw.githubusercontent.com/statsbomb/open-data/master/data/competitions.json", delay=2.0)
p = save_sample("statsbomb", "competitions.json", sb)
comps = guard_rows(json.loads(sb), "statsbomb competitions")
print("statsbomb", len(comps), "verseny; sha256:", sha256_file(p))

ce = fetch("clubelo", "https://raw.githubusercontent.com/xgabora/club-football-match-data/master/README.md", delay=2.0)
p = save_sample("clubelo", "README.md", ce)
print("clubelo tükör elérhető; sha256:", sha256_file(p))

wd = fetch("wikidata", "https://www.wikidata.org/w/api.php?action=wbsearchentities&search=Arsenal&language=en&format=json", delay=2.0)
p = save_sample("wikidata", "arsenal_search.json", wd)
hits = guard_rows(json.loads(wd).get("search") or [], "wikidata search")
print("wikidata", len(hits), "találat; sha256:", sha256_file(p))
print("reep: heti CC0 cut URL a v1 reepcut.plist-ből; probe a release layout szerint (v4).")
print("FM27: helyi save licenc-ellenőrzés (offline, nincs hálózati probe).")
print("LICENC: StatsBomb attribution + non-commercial; reep CC0 strong rung only; FM27 licencelt.")
```

- [ ] **Step 2: reep probe a v1 plist-ből kiolvasott URL-lel**

Run: `grep -A2 'reep' ../football-app/launchd/reepcut.plist | head -8`
Majd a kapott URL-re `fetch("reep", url, delay=2.0)` + `guard_rows` a CSV fejlécére;
ha a release layout változott → `halasztott` indok (v1 fetch_reep_cut.py v4 kezelés).

- [ ] **Step 3: Futtatás + decoy-teszt**

Run: `cd /tmp/s1 && python3 probe_depth.py`
Run: `cd /tmp/s1 && python3 -c "
from harness import guard_rows, DecoyError
try:
    guard_rows([], 'depth-decoy'); raise SystemExit('FAIL')
except DecoyError:
    print('OK')"`

- [ ] **Step 4: Öt katalógus-bejegyzés + visszaolvasás + checkpoint**

Státuszok: próba-szintű igazolás; teljes igazolás a saját adapter-szeletben; licenc-jegyzetek;
Wikidata crosswalk csak egzakt egyezésre (review-sor minden másra); rang 12–16.

---

### Task 14: W4 — hozzáférési mátrix (API-Football, football-data.org, The Odds API, SSR-probe-ok)

**Files:**
- Create: `/tmp/s1/probe_access.py`
- Modify: `docs/research/source-catalog.md`

**Interfaces:**
- Consumes: harness.
- Produces: hozzáférési mátrix bejegyzések: `regisztráció szükséges` státuszok a falaknál (stop + kérdés), SSR-probe eredmények (Bundesliga-hivatalos, Sky, Guardian); MLSsoccer katalógus-jegyzet (demo körön kívül).

- [ ] **Step 1: Auth-fal probe-ok (kulcs nélkül)**

`/tmp/s1/probe_access.py`:

```python
import urllib.request
import urllib.error
from harness import log_budget, budget_left, StopSpike

targets = [
    ("api-football", "https://v3.football.api-sports.io/fixtures?date=2026-11-11"),
    ("football-data-org", "https://api.football-data.org/v4/competitions/PL/matches"),
    ("the-odds-api", "https://api.the-odds-api.com/v4/sports/soccer_epl/odds"),
]
for src, url in targets:
    if budget_left(src) <= 0:
        raise StopSpike(f"{src}: budget")
    req = urllib.request.Request(url)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            log_budget(src, url, resp.status, 0)
            print(src, resp.status, "kulcs nélkül is válaszol — ellenőrzendő")
    except urllib.error.HTTPError as exc:
        log_budget(src, url, exc.code, 0)
        print(src, exc.code, "auth-fal -> regisztráció szükséges (stop, kérdés)")
```

- [ ] **Step 2: SSR-probe-ok**

Run: `cd /tmp/s1 && python3 -c "
from harness import fetch, robots_allowed, guard_rows
import re
for src, base, path, url, pat in [
    ('bundesliga', 'https://www.bundesliga.com', '/en/bundesliga/matchday', 'https://www.bundesliga.com/en/bundesliga/matchday', r'match-day|kickoff'),
    ('guardian', 'https://www.theguardian.com', '/football/results', 'https://www.theguardian.com/football/results', r'football-match'),
]:
    ok, note = robots_allowed(src, base, path)
    print(src, 'robots:', ok, note)
    if ok:
        data = fetch(src, url, delay=6.0)
        hits = re.findall(pat, data.decode('utf-8', 'replace'))
        guard_rows(hits, src)
        print(src, len(hits), 'találat')"`

- [ ] **Step 3: Futtatás**

Run: `cd /tmp/s1 && python3 probe_access.py`
Expected: auth-fal státuszok (401/403 → regisztráció szükséges), SSR-találatok.

- [ ] **Step 4: Mátrix-bejegyzések + visszaolvasás + checkpoint**

Hozzáférési mátrix a katalógusban: forrás × (kulcs nélkül / ingyenes regisztráció / fizetős),
ingyenes csomagkeretekkel (docs-ból, spike-jegyzet); MLSsoccer jegyzet; **W2–W4 záró checkpoint**.

---

### Task 15: S1 kilépés — bizalmi mátrix, sorrend, séma-rés lista, review-kapu

**Files:**
- Modify: `docs/research/source-catalog.md` (összegző fejezetek)
- Create: `docs/checkpoints/2026-11-11/checkpoint.md` kiegészítés vagy új napi checkpoint (a futtatás napja szerint)

**Interfaces:**
- Consumes: minden korábbi task katalógus-bejegyzése és checkpoint-bizonyítéka.
- Produces: S2 spec bemenet: jóváhagyott bizalmi mátrix + adoptációs sorrend + séma-rés lista + FA Kupa döntés-előkészítés + döntési pontok (OpenFoot-kulcs, regisztrációk).

- [ ] **Step 1: Összegző fejezetek a katalógusba**

`## Bizalmi mátrix (jóváhagyásra)` — a spec 3.2 táblája a spike-eredményekkel frissítve;
`## Adoptációs sorrend (jóváhagyásra)` — rangok a státuszok függvényében;
`## Séma-rés lista (S2 bemenet)` — minden jelölt rés: match_stats, referees, bookmakerenkénti
opening/closing odds, stadium/referee, match_lineups, player_availability, match_team_stats,
match_incidents, season_team_stats, season_player_stats, ET/pens explicit forrásjelölés;
`## Döntési pontok` — OpenFoot-kulcs újrahasználat, regisztrációs falak, FA Kupa forrás-választás.

- [ ] **Step 2: Kilépési kritériumok ellenőrzése**

Run: `grep -c "igazolt\|elvetett\|regisztráció szükséges\|halasztott" docs/research/source-catalog.md`
Expected: minden W1/W2 forrásnak van státusza; W3/W4 bejegyzések státusszal vagy halasztási indokkal.
Checklist a spec 6.2 szerint pontról pontra a checkpointba.

- [ ] **Step 3: Magyar szöveg visszaolvasása**

Run: `grep -n "Bizalmi mátrix\|Adoptációs sorrend\|Séma-rés\|Döntési pontok" docs/research/source-catalog.md`
Expected: ékezetes fejezetek.

- [ ] **Step 4: Checkpoint + felhasználói review-kapu**

Checkpoint: futtatott parancsok és eredmények hullámonként, státuszok, séma-rések, döntési pontok,
következő lépés (S2 spec). **STOP: a felhasználó review-zolja a katalógust és a mátrixot;**
jóváhagyás után indul az S2 spec (külön brainstorming ciklus). Commit/push csak külön kérésre.

---

## Self-review jegyzet (a terv írójának)

- Spec-lefedettség: 1. szekció → Task 1 + Global Constraints; 2. szekció → harness + minden probe;
  3. szekció → Task 1 séma + Task 15 mátrix + Obscura döntések (Task 5, 12); 4. szekció → Task 2–14
  forrásai (OpenFoot Task 9, Livescore Task 10); 5. szekció hullámok → Task 2–14 sorrendje;
  6. kilépés → Task 15; 7. tesztelés → decoy-tesztek + checkpointok; 8. kockázatok → guardok.
- Típus-konzisztencia: a harness függvénynevek minden probe-ban azonosak (fetch, robots_allowed,
  guard_rows, save_sample, sha256_file, log_budget, budget_left, StopSpike, DecoyError).
- Placeholder: nincs TBD/TODO; a probe-kódok futtathatók, a katalógus-szövegek a spike kimenetéből
  töltendők (a sablon a Task 1-ben adott).