#!/usr/bin/env python3
"""Csapatlogó-URL-ek a luukhopman/football-logos repóból (S40).

A v1 ugyanezt a repót használta URL-forrásként (`scrappers/web/sources/team_logos.py`).
A repóban nincs licencfájl (a v1 jogi jegyzete szerint a klubcímerek a klubok
tulajdonában vannak), ezért itt is CSAK URL-referenciát írunk be, képet nem
másolunk, és az `attribution` megnevezi a forrást.

Illesztés: pontos, ékezet nélküli név-egyezés a repó fájlneveivel; a jelölt
csak akkor kerül be, ha a név a repón belül egyértelmű. Országot nem
kényszerítünk, de a találatnál a mappa neve megjelenik az attributionben.

Használat:
    python3 scripts/import-luukhopman-logos.py [--dry-run]
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import unicodedata
import urllib.request

REPO = "luukhopman/football-logos"
API = f"https://api.github.com/repos/{REPO}/git/trees/master?recursive=1"
RAW = f"https://raw.githubusercontent.com/{REPO}/master/"
NOISE = re.compile(r"\b(fc|cf|sc|afc|ac|as|ss|us|ca|fk|sk|bk|if|club|calcio|cfc)\b")


def fold(name: str) -> str:
    text = unicodedata.normalize("NFKD", name or "")
    text = "".join(ch for ch in text if not unicodedata.combining(ch)).lower()
    text = text.replace("&", " and ")
    text = re.sub(r"[^a-z0-9]+", " ", text)
    text = NOISE.sub(" ", text)
    return " ".join(text.split())


def tokens(name: str) -> list[str]:
    return fold(name).split()


def prefix_equal(left: str, right: str) -> bool:
    if left == right:
        return True
    if len(left) >= 3 and right.startswith(left):
        return True
    return len(right) >= 3 and left.startswith(right)


def similar_name(team_name: str, candidate: str) -> bool:
    """Rövidítés-tűrő egyezés („Ath Bilbao” ↔ „Athletic Bilbao”)."""
    left, right = tokens(team_name), tokens(candidate)
    if not left or not right:
        return False
    if left == right:
        return True
    if len(left) == len(right) and all(prefix_equal(a, b) for a, b in zip(left, right)):
        return True
    # a csapatnév minden tokenje megtalálható a jelöltben (legfeljebb 2 extra tokennel)
    return all(any(prefix_equal(a, b) for b in right) for a in left) and len(right) <= len(left) + 2


def psql(dsn: str, sql: str) -> list[list[str]]:
    out = subprocess.run(["psql", dsn, "-tAF", "\t", "-c", sql], capture_output=True, text=True, check=True).stdout
    return [line.split("\t") for line in out.splitlines() if line.strip()]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    dsn = os.environ.get("DATABASE_URL")
    if not dsn:
        print("DATABASE_URL hiányzik", file=sys.stderr)
        return 2

    req = urllib.request.Request(API, headers={"User-Agent": "football-app-v2-logo-indexer/1.0"})
    with urllib.request.urlopen(req, timeout=60) as response:
        tree = json.load(response)
    files: dict[str, list[tuple[str, str]]] = {}
    for node in tree.get("tree", []):
        path = node.get("path", "")
        if not path.startswith("logos/") or not path.lower().endswith((".png", ".jpg", ".jpeg", ".webp")):
            continue
        folder = path.split("/")[1] if path.count("/") >= 2 else ""
        stem = path.rsplit("/", 1)[-1].rsplit(".", 1)[0]
        files.setdefault(fold(stem), []).append((path, folder))

    teams = psql(dsn, """
        SELECT t.id, t.name, COALESCE(c.code, '')
          FROM core.team t LEFT JOIN core.country c ON c.id = t.country_id
    """)
    candidates = []
    ambiguous = 0
    fuzzy = 0
    for team_id, name, country in teams:
        hits = files.get(fold(name)) or []
        if not hits:
            # rövidítés-tűrő második kör: csak egyértelmű találatot fogadunk el
            hits = [hit for key, rows_ in files.items() if similar_name(name, key) for hit in rows_]
            fuzzy += 1 if hits else 0
        if not hits:
            continue
        if len(hits) > 1:
            # ország-mappa szerinti szűkítés, ha egyértelmű marad
            narrowed = [hit for hit in hits if country and country.lower() in hit[1].lower()]
            if len(narrowed) != 1:
                ambiguous += 1
                continue
            hits = narrowed
        path, folder = hits[0]
        candidates.append((team_id, RAW + urllib.request.quote(path, safe="/"), name, folder))

    print(f"repó fájlok: {sum(len(v) for v in files.values())}, jelölt: {len(candidates)} (ebből rövidítés-egyezés: {fuzzy}), többértelmű kihagyva: {ambiguous}")
    if args.dry_run:
        for row in candidates[:12]:
            print("  ", row[2], "→", row[3], "|", row[1][:80])
        return 0

    if not candidates:
        return 0
    values = ",".join(
        f"(gen_random_uuid(),'team','{team_id}','logo',NULL,'{url}','luukhopman','{folder.replace(chr(39), chr(39)*2)}','unknown','{REPO} (nincs licencfájl a repóban; a klubcímerek a klubok tulajdona) {folder.replace(chr(39), chr(39)*2)}',now())"
        for team_id, url, _name, folder in candidates
    )
    psql(dsn, f"""INSERT INTO ops.asset (id, entity_kind, entity_id, kind, storage_key, url, source, licence, licence_status, attribution, captured_at)
                  VALUES {values} ON CONFLICT DO NOTHING""")
    total = psql(dsn, "SELECT count(*) FROM ops.asset WHERE entity_kind='team' AND kind='logo'")
    print(f"kész; team-logo asset sorok: {total[0][0]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
