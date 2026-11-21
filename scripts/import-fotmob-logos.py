"""S49: hiányzó csapatlogók importja a FotMob nyilvános API-jából.

A FotMob napi meccs-JSON-jaiból név alapján párosítja a hiányzó logójú
csapatokat, letölti a teamlogo képet, tartalom-címzett útvonalra menti a
DATA_DIR/raw/assets alá, és ops.asset sort ír (licence_status=unknown).

Használat:
    set -a && source .env && set +a
    python3 scripts/import-fotmob-logos.py --from 2026-11-21 --to 2026-11-30

A --apply nélkül csak riportot ad (dry-run).
"""

from __future__ import annotations

import argparse
import datetime as dt
import glob
import hashlib
import json
import os
import re
import subprocess
import unicodedata
import urllib.request
from concurrent.futures import ThreadPoolExecutor

FOTMOB_MATCHES = "https://www.fotmob.com/api/data/matches?date={date}"
FOTMOB_TEAM_LOGO = "https://images.fotmob.com/image_resources/logo/teamlogo/{id}.png"
CACHE = "/tmp/fotmob-{date}.json"


def fold(value: str) -> str:
    decomposed = unicodedata.normalize("NFKD", value or "").encode("ascii", "ignore").decode()
    return re.sub(r"[^a-z0-9]+", " ", decomposed.lower()).strip()


def fetch_cache(date: str) -> None:
    path = CACHE.format(date=date)
    if os.path.exists(path) and os.path.getsize(path) > 1000:
        return
    url = FOTMOB_MATCHES.format(date=date.replace("-", ""))
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(request, timeout=30) as response:
        payload = response.read()
    with open(path, "wb") as handle:
        handle.write(payload)
    print(f"fetched {date} ({len(payload)} bytes)")


def load_team_ids(dates: list[str]) -> dict[str, int]:
    by_name: dict[str, int] = {}
    for date in dates:
        for path in glob.glob(CACHE.format(date=date)):
            try:
                with open(path) as handle:
                    payload = json.load(handle)
            except (OSError, json.JSONDecodeError):
                continue
            for league in payload.get("leagues", []):
                for match in league.get("matches", []):
                    for side in ("home", "away"):
                        team = match.get(side) or {}
                        name, team_id = team.get("name"), team.get("id")
                        if name and team_id:
                            by_name.setdefault(fold(name), team_id)
    return by_name


def resolve_team(name: str, by_name: dict[str, int]) -> int | None:
    """Pontos fold-egyezés, majd egyértelmű token-részhalmaz egyezés."""
    folded = fold(name)
    if folded in by_name:
        return by_name[folded]
    tokens = set(folded.split())
    if not tokens or any(len(token) < 3 for token in tokens):
        return None
    womens = " w" in f" {folded}" or "women" in folded or "női" in folded
    candidates: set[int] = set()
    for candidate_name, team_id in by_name.items():
        other = set(candidate_name.split())
        if not other or any(len(token) < 3 for token in other):
            continue
        other_womens = " w" in f" {candidate_name}" or "women" in candidate_name
        if womens != other_womens:
            continue
        if tokens <= other or other <= tokens:
            candidates.add(team_id)
    if len(candidates) == 1:
        return next(iter(candidates))
    return None


def missing_teams(start: str, end: str) -> list[tuple[str, str]]:
    query = f"""
        SELECT DISTINCT t.id, t.name
        FROM core.match m
        JOIN core.team t ON t.id IN (m.home_team_id, m.away_team_id)
        WHERE m.kickoff_at >= '{start}' AND m.kickoff_at < '{end}'::date + 1
          AND NOT EXISTS (
              SELECT 1 FROM ops.asset a
              WHERE a.entity_kind = 'team' AND a.entity_id = t.id AND a.kind = 'logo'
          )
        ORDER BY t.name
    """
    result = subprocess.run(
        ["psql", os.environ["DATABASE_URL"], "-At", "-F", "\t", "-c", query],
        capture_output=True,
        check=True,
        text=True,
    )
    rows = []
    for line in result.stdout.splitlines():
        if "\t" in line:
            team_id, name = line.split("\t", 1)
            rows.append((team_id, name))
    return rows


def download(item: tuple[str, str, int]) -> tuple[str, str, str, bytes] | None:
    team_id, name, fotmob_id = item
    url = FOTMOB_TEAM_LOGO.format(id=fotmob_id)
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            payload = response.read()
    except OSError:
        return None
    if len(payload) < 200:
        return None
    return team_id, name, url, payload


def write_assets(items: list[tuple[str, str, str, bytes]], data_dir: str) -> str:
    statements = ["BEGIN;"]
    for team_id, _name, url, payload in items:
        digest = hashlib.sha256(payload).hexdigest()
        relative = f"assets/{digest[:2]}/{digest[2:4]}/{digest}.png"
        path = os.path.join(data_dir, "raw", relative)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        if not os.path.exists(path):
            with open(path, "wb") as handle:
                handle.write(payload)
        statements.append(
            f"""INSERT INTO ops.asset(id, entity_kind, entity_id, kind, url, storage_key, source,
                    licence, licence_status, attribution, captured_at)
                SELECT gen_random_uuid(), 'team', '{team_id}', 'logo', '{url}', '{relative}',
                       'fotmob', 'fotmob', 'unknown', 'FotMob', now()
                WHERE NOT EXISTS (
                    SELECT 1 FROM ops.asset a
                    WHERE a.entity_kind = 'team' AND a.entity_id = '{team_id}' AND a.kind = 'logo'
                );"""
        )
    statements.append("COMMIT;")
    path = "/tmp/fotmob-logos.sql"
    with open(path, "w") as handle:
        handle.write("\n".join(statements))
    subprocess.run(["psql", os.environ["DATABASE_URL"], "-v", "ON_ERROR_STOP=1", "-f", path], check=True)
    return path


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--from", dest="start", default="2026-11-21")
    parser.add_argument("--to", dest="end", default="2026-11-30")
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()

    start = dt.date.fromisoformat(args.start)
    end = dt.date.fromisoformat(args.end)
    dates = [(start + dt.timedelta(days=offset)).isoformat() for offset in range((end - start).days + 1)]
    for date in dates:
        fetch_cache(date)

    by_name = load_team_ids(dates)
    print(f"FotMob csapatnév: {len(by_name)}")

    teams = missing_teams(args.start, args.end)
    print(f"Hiányzó logójú csapat a napokban: {len(teams)}")

    targets = []
    for team_id, name in teams:
        fotmob_id = resolve_team(name, by_name)
        if fotmob_id:
            targets.append((team_id, name, fotmob_id))
    print(f"FotMob azonosítóval párosítva: {len(targets)}")

    downloaded = []
    with ThreadPoolExecutor(max_workers=12) as executor:
        for item in executor.map(download, targets):
            if item:
                downloaded.append(item)
    print(f"Letöltött logó: {len(downloaded)}")

    if not args.apply:
        print("dry-run: az ops.asset írás elmaradt (--apply a mentéshez)")
        return

    path = write_assets(downloaded, os.environ.get("DATA_DIR", "./data"))
    print(f"ops.asset írás kész: {path}")


if __name__ == "__main__":
    main()