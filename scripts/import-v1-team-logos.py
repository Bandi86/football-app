#!/usr/bin/env python3
"""v1 csapatlogók importálása a v2 asset-tárba (S40).

A v1 a címerképeket a Wikimedia Commonsból töltötte le (CC0/CC-BY),
`public/assets/team/<team_id>.webp` néven, és a `assets-manifest/manifest.json`
térképben tartja nyilván. A v1 `teams.logo_url` mezője a
`luukhopman/football-logos` repóra mutató URL-eket tartalmaz (licenc nélkül).

Ez a script:
  1. a v1 manifest + képfájlok alapján a v2 `ops.asset` tárba másolja a
     képeket tartalom-címzett kulccsal (`assets/<aa>/<bb>/<sha256>.webp`),
     így a `/v1/asset/...` végpont kiszolgálja őket;
  2. ahol nincs helyi kép, de van `logo_url`, URL-referenciát ír be.

A licenc-állapot minden sornál `unknown`, mert a v1 manifest nem hordoz
soronkénti licencet; a forrás megnevezése az `attribution` mezőben van.
Használat:
    python3 scripts/import-v1-team-logos.py [--dry-run]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import sys

V1_ROOT = pathlib.Path.home() / "Documents/Code/football-app"
V1_MANIFEST = V1_ROOT / "assets-manifest/manifest.json"
V2_DATA = pathlib.Path.home() / "Library/Application Support/FootballAppV2/agent/data"
ATTRIBUTION = "v1 import: Wikimedia Commons (CC0/CC-BY), v1 assets-manifest"


def psql(dsn: str, sql: str) -> list[list[str]]:
    done = subprocess.run(
        ["psql", dsn, "-tAF", "\t", "-v", "ON_ERROR_STOP=1", "-c", sql],
        capture_output=True, text=True,
    )
    if done.returncode != 0:
        raise SystemExit(f"psql hiba: {done.stderr.strip()}")
    out = done.stdout
    return [line.split("\t") for line in out.splitlines() if line.strip()]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    dsn = os.environ.get("DATABASE_URL")
    if not dsn:
        print("DATABASE_URL hiányzik", file=sys.stderr)
        return 2
    if not V1_MANIFEST.exists():
        print(f"v1 manifest nem található: {V1_MANIFEST}", file=sys.stderr)
        return 2

    # v1 team id -> v2 team id (v1t- refek)
    rows = psql(dsn, """
        SELECT r.source_entity_id, r.entity_id
          FROM identity.source_entity_ref r
          JOIN ops.source s ON s.id = r.source_id
         WHERE s.code = 'v1-import' AND r.entity_kind = 'team'
           AND r.source_entity_id LIKE 'v1t-%'
    """)
    mapping = {int(src.removeprefix("v1t-")): dst for src, dst, *_ in rows}
    print(f"v1->v2 csapat-híd: {len(mapping)}")

    manifest = json.loads(V1_MANIFEST.read_text())
    copied = url_refs = missing = already = 0
    inserts: list[tuple[str, str, str]] = []  # (team_id, storage_key_or_url, kind)

    for key, entry in manifest.items():
        if not key.startswith("team|"):
            continue
        # a manifest kulcsa a SofaScore-azonosító; a v1 team id a `core_id`.
        try:
            v1_id = int(entry.get("core_id") or key.split("|", 1)[1])
        except (TypeError, ValueError):
            continue
        team_id = mapping.get(v1_id)
        if not team_id:
            missing += 1
            continue
        # a manifest útvonala a v1 `public/` alatt értendő
        path = V1_ROOT / "public" / entry.get("path", "")
        if not path.exists():
            missing += 1
            continue
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        suffix = path.suffix.lower() or ".webp"
        storage_key = f"assets/{digest[:2]}/{digest[2:4]}/{digest}{suffix}"
        target = V2_DATA / "raw" / storage_key
        if not args.dry_run and not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
        copied += 1
        inserts.append((team_id, storage_key, "storage"))

    # logo_url tételek (helyi kép nélküli csapatokhoz)
    for v1_id, url in psql("dbname=football_app", "SELECT id, logo_url FROM teams WHERE logo_url IS NOT NULL"):
        team_id = mapping.get(int(v1_id))
        if not team_id:
            missing += 1
            continue
        inserts.append((team_id, url, "url"))
        url_refs += 1

    print(f"manifest képek: {copied}, logo_url: {url_refs}, kihagyva (nincs v2 híd): {missing}")
    if args.dry_run:
        print(f"dry-run: {len(inserts)} sor lenne beírva")
        return 0

    # A tábla `url` mezője NOT NULL, ezért a helyi blobokhoz is kell egy
    # stabil, belső hivatkozás (a nap-feed a storage_key-t részesíti előnyben).
    def one(row: tuple[str, str, str]) -> str:
        team_id, value, kind = row
        if kind == "storage":
            return (
                f"(gen_random_uuid(),'team','{team_id}','logo','{value}',"
                f"'local:{value}','v1-import',NULL,'unknown','{ATTRIBUTION}',now())"
            )
        return (
            f"(gen_random_uuid(),'team','{team_id}','logo',NULL,'{value}',"
            f"'v1-import',NULL,'unknown','{ATTRIBUTION}',now())"
        )

    columns = (
        "id, entity_kind, entity_id, kind, storage_key, url, source, licence, "
        "licence_status, attribution, captured_at"
    )
    for start in range(0, len(inserts), 100):
        chunk = inserts[start : start + 100]
        values = ",".join(one(row) for row in chunk)
        psql(
            dsn,
            f"INSERT INTO ops.asset ({columns}) VALUES {values} ON CONFLICT DO NOTHING",
        )
        print(f"  beírva: {min(start + 100, len(inserts))}/{len(inserts)}")
    total = psql(dsn, "SELECT count(*) FROM ops.asset WHERE entity_kind='team' AND kind='logo'")
    print(f"kész; team-logo asset sorok: {total[0][0]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
