#!/usr/bin/env python3
"""Fold-azonos csapat-entitások csoportosítása és merge-elése (football-app-v2).

A v1 import és a footballdata gyakran ugyanazt a klubot két entitásként hozza
létre: eltérő kanonikus név ("AFC Ajax" vs "Ajax"), ékezet (Fenerbahçe vs
Fenerbahce) vagy török betű (Beşiktaş vs Besiktas). Ez a script a core.team
sorokat foldolt név + kontextus szerint csoportosítja, és a >1 elemű
generációkat a legtöbb meccses entitásba merge-eli.

Használat:
    set -a && source .env && set +a
    python3 scripts/team_dup_groups.py            # dry-run: csak listáz
    python3 scripts/team_dup_groups.py --apply    # tényleges merge (footballctl)

Környezet:
    DATABASE_URL        kötelező (a .env-ből)
    FOOTBALL_APP_V2     opcionális repo-gyökér (default: ~/Documents/Code/football-app-v2)
    FOOTBALLCTL         opcionális footballctl útvonal
"""

from __future__ import annotations

import os
import subprocess
import sys
import unicodedata
import uuid
from collections import defaultdict


def fold(name: str) -> str:
    """Ékezet-független, kisbetűs név (török ı/İ is)."""
    lowered = " ".join(name.split()).lower()
    decomposed = unicodedata.normalize("NFKD", lowered)
    out = []
    for ch in decomposed:
        if unicodedata.combining(ch):
            continue
        if ch == "ı":
            out.append("i")
        elif ch == "ß":
            out.append("ss")
        else:
            out.append(ch)
    return "".join(out)


SQL = """SELECT t.id, t.entity_id, t.name, coalesce(k.code, ''),
       t.gender, t.age_group, t.branch,
       (SELECT count(*) FROM core.match m
         WHERE m.home_team_id = t.id OR m.away_team_id = t.id)
  FROM core.team t
  LEFT JOIN core.country k ON k.id = t.country_id"""


def are_opponents(db: str, first: str, second: str) -> bool:
    """Ha a két entitás valaha egymás ellen játszott, NEM ugyanaz a klub."""
    a = str(uuid.UUID(first))
    b = str(uuid.UUID(second))
    sql = (
        "SELECT count(*) FROM core.match m "
        "JOIN core.team th ON th.id=m.home_team_id "
        "JOIN core.team ta ON ta.id=m.away_team_id "
        f"WHERE (th.entity_id='{a}' AND ta.entity_id='{b}') "
        f"   OR (th.entity_id='{b}' AND ta.entity_id='{a}')"
    )
    run = subprocess.run(
        ["psql", db, "-t", "-A", "-c", sql],
        capture_output=True,
        text=True,
    )
    return run.returncode == 0 and run.stdout.strip() not in ("", "0")


def main() -> int:
    apply = "--apply" in sys.argv[1:]
    db = os.environ.get("DATABASE_URL")
    if not db:
        print("DATABASE_URL kötelező (source .env)", file=sys.stderr)
        return 2

    repo = os.environ.get(
        "FOOTBALL_APP_V2", os.path.expanduser("~/Documents/Code/football-app-v2")
    )
    ctl = os.environ.get("FOOTBALLCTL", os.path.join(repo, "rust/target/debug/footballctl"))
    if apply and not os.path.exists(ctl):
        print(f"hiányzó footballctl: {ctl}", file=sys.stderr)
        return 2

    proc = subprocess.run(
        ["psql", db, "-P", "pager=off", "-t", "-A", "-F", "\t", "-c", SQL],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        print(proc.stderr.strip(), file=sys.stderr)
        return 1

    groups: dict[tuple[str, str, str, str, str], list[tuple[int, str, str]]] = defaultdict(list)
    for line in proc.stdout.splitlines():
        parts = line.split("\t")
        if len(parts) != 8:
            continue
        _team_id, entity_id, name, country, gender, age_group, branch, matches = parts
        try:
            count = int(matches)
        except ValueError:
            count = 0
        groups[(fold(name), country, gender, age_group, branch)].append(
            (count, entity_id, name)
        )

    dupes = {k: v for k, v in groups.items() if len(v) > 1}
    if not dupes:
        print("nincs fold-azonos duplikált csapatcsoport")
        return 0

    total = sum(len(v) - 1 for v in dupes.values())
    print(f"duplikált csoportok: {len(dupes)}, merge-elendő entitás: {total}")
    failures: list[tuple[str, str]] = []
    for key, rows in sorted(dupes.items(), key=lambda kv: -max(r[0] for r in kv[1])):
        rows.sort(key=lambda r: (-r[0], r[2]))
        survivor_count, survivor, survivor_name = rows[0]
        print(f"\n[{key[0]} | {key[1] or '-'}] túlélő: {survivor_name} ({survivor_count} meccs)")
        for count, entity, name in rows[1:]:
            if are_opponents(db, survivor, entity):
                print(f"  - {name} ({count} meccs) -> KIHAGYVA: egymás ellen játszottak")
                continue
            print(f"  - {name} ({count} meccs) -> merge")
            if not apply:
                continue
            run = subprocess.run(
                [
                    ctl,
                    "identity",
                    "merge-team",
                    "--from",
                    entity,
                    "--to",
                    survivor,
                    "--note",
                    "fold-azonos duplikátum (team_dup_groups.py)",
                    "--apply",
                ],
                capture_output=True,
                text=True,
                env={**os.environ, "DATABASE_URL": db},
                cwd=repo,
            )
            if run.returncode != 0:
                failures.append((name, (run.stderr or run.stdout)[-200:]))

    if failures:
        print(f"\nhibás merge: {len(failures)}", file=sys.stderr)
        for name, err in failures:
            print(f"  {name}: {err}", file=sys.stderr)
        return 1
    if apply:
        print(f"\nkész: {total} entitás merge-elve")
    else:
        print("\ndry-run — `--apply` a merge-hez")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
