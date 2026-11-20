#!/usr/bin/env python3
"""A football-research napi fájljainak előzetes ellenőrzése betöltés előtt.

Használat: python3 scripts/precheck-research-days.py 2026-11-18 2026-11-19 2026-11-21

Ellenőrzi a napi envelope szerkezetét, a meccsek és oddsok számát, a hiányzó
kickoffokat és eredményeket, a napon belüli duplikátumokat, a versenynevek
kanonizálását (a `research` adapter alias-táblájával) és a naptári napon kívüli
kickoffokat. Csak olvas, semmit nem ír az adatbázisba.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import unicodedata
from collections import Counter
from pathlib import Path

API_ROOT = Path(
    os.environ.get(
        "FOOTBALL_RESEARCH_API_DIR",
        str(Path.home() / "Library/Application Support/FootballResearch/api/v1"),
    )
)

# Az adapter COMPETITION_ALIASES táblájának tükre (S38b).
ALIASES = {
    ("ENG", "league 1"): ("League One", "GB"),
    ("ENG", "league 2"): ("League Two", "GB"),
    ("ENG", "league one"): ("League One", "GB"),
    ("ENG", "league two"): ("League Two", "GB"),
    ("WAL", "premier league"): ("Cymru Premier", "GB"),
    ("WAL", "premier division"): ("Cymru Premier", "GB"),
    ("SCO", "premiership"): ("Scottish Premiership", "GB"),
    ("SCO", "championship"): ("Scottish Championship", "GB"),
    ("SCO", "league 1"): ("Scottish League One", "GB"),
    ("SCO", "league one"): ("Scottish League One", "GB"),
    ("SCO", "league 2"): ("Scottish League Two", "GB"),
    ("SCO", "league two"): ("Scottish League Two", "GB"),
    ("SCO", "swpl 1"): ("SWPL 1 - női", "GB"),
    ("NIR", "premiership"): ("NIFL Premiership", "GB"),
    ("NIR", "championship"): ("NIFL Championship", "GB"),
    ("ESP", "laliga"): ("La Liga", "ES"),
    ("ESP", "laliga2"): ("LaLiga 2", "ES"),
    ("ESP", "copa del rey - 1st round"): ("Copa del Rey", "ES"),
}

WOMEN_MARKERS = [
    "(w)", " women", "női", "frauen", "femenina", "femminile", "dames",
    "kvinde", "damallsvenskan", "nwsl", "wsl", "swpl", "we league", "liga f",
    "toppserien", "ncaaw", "primera division femenina", "a-league women", "uwcl",
]
YOUTH_RE = re.compile(r"\bu-?(\d{1,2})\b", re.IGNORECASE)


def fold(value: str) -> str:
    ascii_value = unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode()
    return " ".join(re.sub(r"[^a-z0-9]+", " ", ascii_value.lower()).split())


def canonical_competition(competition: dict) -> tuple[str, str]:
    name = (competition.get("name") or "").strip()
    if not name:
        name = (competition.get("display_name") or "").strip()
    raw_country = (competition.get("country") or "").strip().upper()
    alias = ALIASES.get((raw_country, name.lower()))
    if alias:
        return alias
    return name or "Ismeretlen verseny", raw_country or "INT"


def v2_competitions(database_url: str | None) -> set[tuple[str, str]] | None:
    if not database_url:
        return None
    try:
        query = (
            "SELECT c.name || E'\\t' || co.code FROM core.competition c "
            "JOIN core.country co ON co.id = c.country_id"
        )
        out = subprocess.run(
            ["psql", database_url, "-tAF", "\t", "-c", query],
            capture_output=True, text=True, check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        return None
    rows = set()
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) == 2:
            rows.add((fold(parts[0]), parts[1]))
    return rows


def check_day(date: str, v2: set[tuple[str, str]] | None) -> None:
    year, month, _ = date.split("-")
    path = API_ROOT / "daily" / year / month / f"{date}.json"
    if not path.exists():
        print(f"{date}: HIBA — hiányzó napi fájl: {path}")
        return
    doc = json.loads(path.read_text())
    data = doc.get("data") or []
    matches = len(data)

    missing_kickoff = [m for m in data if not (m.get("kickoff") or {}).get("utc")]
    match_day_shift = 0
    in_day_duplicates = Counter()
    finished_without_result = 0
    finished = 0
    odds_matches = 0
    competitions = Counter()
    women = youth = 0
    empty_country = 0

    for m in data:
        teams = m.get("teams") or {}
        home = (teams.get("home") or {}).get("id")
        away = (teams.get("away") or {}).get("id")
        kickoff = (m.get("kickoff") or {}).get("utc")
        in_day_duplicates[(home, away, kickoff)] += 1
        if kickoff:
            kickoff_local = (m.get("kickoff") or {}).get("local")
            if kickoff_local and kickoff_local.startswith(("01:", "02:", "03:", "04:")):
                # UTC-hajnali kezdés, de a helyi nap még az előző; ez nem hiba,
                # csak figyeljük a napi besorolás miatt.
                pass
        status = ((m.get("status") or {}).get("code") or "").lower()
        if status == "finished":
            finished += 1
            score = m.get("score") or {}
            if score.get("home") is None or score.get("away") is None:
                finished_without_result += 1
        if (m.get("coverage") or {}).get("odds"):
            odds_matches += 1
        competition = m.get("competition") or {}
        name, country = canonical_competition(competition)
        competitions[(name, country)] += 1
        if not country or country == "INT":
            empty_country += 1
        if any(marker in name.lower() for marker in WOMEN_MARKERS):
            women += 1
        if YOUTH_RE.search(name):
            youth += 1
        kickoff = (m.get("kickoff") or {}).get("utc")
        if kickoff and not kickoff.startswith(date):
            match_day_shift += 1

    duplicates = sum(1 for count in in_day_duplicates.values() if count > 1)
    new_competitions = []
    if v2 is not None:
        new_competitions = sorted(
            name for (name, country) in competitions if (fold(name), country) not in v2
        )

    print(f"=== {date}")
    print(f"  meccsek: {matches}, befejezett: {finished}, eredmény nélkül: {finished_without_result}")
    print(f"  oddsos meccs: {odds_matches}, hiányzó kickoff: {len(missing_kickoff)}")
    print(f"  napon belüli duplikátum-kulcs: {duplicates}")
    print(f"  kickoff a naptári napon kívül: {match_day_shift}")
    print(f"  versenyek: {len(competitions)}, nemzetközi (INT) meccs: {empty_country}")
    print(f"  női meccsek: {women}, utánpótlás meccsek: {youth}")
    if v2 is not None:
        print(f"  a v2-ben még nincs ilyen verseny: {len(new_competitions)}")
        for name in new_competitions[:8]:
            print(f"    - {name}")
        if len(new_competitions) > 8:
            print(f"    … és {len(new_competitions) - 8} további")


def main() -> int:
    dates = sys.argv[1:]
    if not dates:
        print(__doc__)
        return 2
    v2 = v2_competitions(os.environ.get("DATABASE_URL"))
    if v2 is None:
        print("(a DATABASE_URL nincs beállítva vagy a psql nem elérhető: v2-összehasonlítás kihagyva)")
    for date in dates:
        check_day(date, v2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
