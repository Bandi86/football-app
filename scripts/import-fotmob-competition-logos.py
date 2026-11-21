#!/usr/bin/env python3
"""FotMob verseny-logók importja (S51).

A napi meccs-JSON `leagues[]` elemei megadják a verseny FotMob-azonosítóját
(`id`), nevét (`name`) és országkódját (`ccode`). A `leaguelogo/{id}.png`
kép csak böngésző User-Agent + Referer fejléccel jön le (403 nélkül).

A v2 versenyekhez normalizált név + ország-család alapján párosítunk, a képet
tartalom-címzett útvonalra mentjük a DATA_DIR/raw/assets alá, és `ops.asset`
sort írunk. Alapértelmezésben dry-run; `--apply` ír.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import unicodedata
import urllib.request
from pathlib import Path

HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) "
        "AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36"
    ),
    "Referer": "https://www.fotmob.com/",
    "Accept": "image/png,image/*;q=0.9,*/*;q=0.8",
}

ISO3_TO_ISO2 = {
    "ESP": "ES", "ENG": "GB-ENG", "SCO": "GB-SCT", "WAL": "GB-WLS", "NIR": "GB-NIR",
    "GER": "DE", "FRA": "FR", "ITA": "IT", "NED": "NL", "POR": "PT", "BEL": "BE",
    "TUR": "TR", "GRE": "GR", "HUN": "HU", "POL": "PL", "CZE": "CZ", "SVK": "SK",
    "AUT": "AT", "SUI": "CH", "DEN": "DK", "SWE": "SE", "NOR": "NO", "FIN": "FI",
    "CRO": "HR", "SRB": "RS", "ROU": "RO", "BUL": "BG", "UKR": "UA", "RUS": "RU",
    "USA": "US", "MEX": "MX", "BRA": "BR", "ARG": "AR", "URU": "UY", "COL": "CO",
    "CHI": "CL", "PER": "PE", "ECU": "EC", "BOL": "BO", "PAR": "PY", "VEN": "VE",
    "JPN": "JP", "KOR": "KR", "CHN": "CN", "AUS": "AU", "IRN": "IR", "IRQ": "IQ",
    "KSA": "SA", "QAT": "QA", "EGY": "EG", "MAR": "MA", "ALG": "DZ", "TUN": "TN",
    "RSA": "ZA", "NGA": "NG", "GHA": "GH", "ISR": "IL", "IND": "IN", "IDN": "ID",
    "MAS": "MY", "THA": "TH", "VIE": "VN", "SIN": "SG", "UZB": "UZ", "KAZ": "KZ",
}


def fold(text: str) -> str:
    text = unicodedata.normalize("NFKD", text or "")
    text = text.encode("ascii", "ignore").decode()
    return re.sub(r"[^a-z0-9]+", " ", text.lower()).strip()


def country_family(code: str) -> str:
    code = (code or "").upper()
    if not code:
        return ""
    if code.startswith("GB"):
        return "GB"
    return ISO3_TO_ISO2.get(code, code)


def fetch_png(league_id: int) -> bytes | None:
    url = f"https://images.fotmob.com/image_resources/logo/leaguelogo/{league_id}.png"
    req = urllib.request.Request(url, headers=HEADERS)
    try:
        with urllib.request.urlopen(req, timeout=20) as response:
            if response.status != 200:
                return None
            return response.read()
    except Exception as exc:  # noqa: BLE001 - forrás hiba nem állítja meg az importot
        print(f"  ! {league_id}: {exc}")
        return None


def psql(sql: str) -> str:
    out = subprocess.run(
        ["psql", os.environ["DATABASE_URL"], "-At", "-F", "\t", "-c", sql],
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.strip()


def load_leagues(cache_dir: Path) -> dict[str, dict]:
    leagues: dict[str, dict] = {}
    for path in sorted(cache_dir.glob("fotmob-2026-*.json")):
        try:
            data = json.loads(path.read_text())
        except Exception:  # noqa: BLE001
            continue
        for league in data.get("leagues") or []:
            name = league.get("name") or ""
            key = fold(name)
            if not key or not league.get("id"):
                continue
            leagues.setdefault(key, {
                "id": int(league["id"]),
                "name": name,
                "ccode": league.get("ccode") or "",
            })
    return leagues


def fuzzy_league(key: str, cc: str, leagues: dict[str, dict]) -> dict | None:
    """Egyértelmű hasonló nevű liga (pl. szponzor-név/utótag eltérés)."""
    import difflib

    fam_db = country_family(cc)
    best: tuple[float, dict] | None = None
    second = 0.0
    for name_key, entry in leagues.items():
        fam_fm = country_family(entry["ccode"])
        if fam_db and fam_fm and fam_db != fam_fm:
            continue
        ratio = difflib.SequenceMatcher(None, key, name_key).ratio()
        if best is None or ratio > best[0]:
            second = best[0] if best else 0.0
            best = (ratio, entry)
        elif ratio > second:
            second = ratio
    if best and best[0] >= 0.82 and best[0] - second >= 0.05:
        return best[1]
    return None


def load_competitions() -> list[dict]:
    rows = psql(
        """
        SELECT c.id, c.entity_id, c.name, coalesce(co.code,'')
        FROM core.competition c
        LEFT JOIN core.country co ON co.id = c.country_id
        WHERE EXISTS (
            SELECT 1 FROM core.match m
            WHERE m.competition_id = c.id
              AND m.kickoff_at >= '2026-11-13' AND m.kickoff_at < '2026-12-02'
        )
        AND NOT EXISTS (
            SELECT 1 FROM ops.asset a
            WHERE a.entity_kind='competition' AND a.entity_id=c.entity_id AND a.kind='logo'
        )
        ORDER BY c.name
        """
    )
    result = []
    for line in rows.splitlines():
        parts = line.split("\t")
        if len(parts) == 4:
            result.append({"id": parts[0], "entity_id": parts[1], "name": parts[2], "cc": parts[3]})
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--cache-dir", default="/tmp")
    args = parser.parse_args()

    data_dir = Path(os.environ.get("DATA_DIR", "./data"))
    leagues = load_leagues(Path(args.cache_dir))
    competitions = load_competitions()
    print(f"FotMob liga: {len(leagues)}, logó nélküli v2 verseny: {len(competitions)}")

    statements: list[str] = []
    matched = 0
    for comp in competitions:
        key = fold(comp["name"])
        entry = leagues.get(key)
        if entry is None:
            entry = fuzzy_league(key, comp["cc"], leagues)
        if entry is None:
            continue
        fam_db = country_family(comp["cc"])
        fam_fm = country_family(entry["ccode"])
        if fam_db and fam_fm and fam_db != fam_fm:
            continue
        matched += 1
        if not args.apply:
            continue
        png = fetch_png(entry["id"])
        if not png or not png.startswith(b"\x89PNG"):
            continue
        digest = hashlib.sha256(png).hexdigest()
        rel = f"raw/assets/{digest[:2]}/{digest[2:4]}/{digest}.png"
        target = data_dir / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            target.write_bytes(png)
        statements.append(
            "INSERT INTO ops.asset (id, entity_kind, entity_id, kind, url, storage_key, source, "
            "licence, licence_status, attribution, captured_at) VALUES "
            f"(gen_random_uuid(),'competition','{comp['entity_id']}','logo',"
            f"'https://images.fotmob.com/image_resources/logo/leaguelogo/{entry['id']}.png',"
            f"'assets/{digest[:2]}/{digest[2:4]}/{digest}.png','fotmob','fotmob','unknown',"
            f"'FotMob leaguelogo {entry['id']} ({entry['name']})', now()) "
            "ON CONFLICT (entity_kind, entity_id, kind, url) DO NOTHING;"
        )

    print(f"párosított verseny: {matched}, letöltött logó: {len(statements)}")
    if statements:
        sql_path = Path("/tmp/fotmob-competition-logos.sql")
        sql_path.write_text("BEGIN;\n" + "\n".join(statements) + "\nCOMMIT;\n")
        subprocess.run(
            ["psql", os.environ["DATABASE_URL"], "-v", "ON_ERROR_STOP=1", "-f", str(sql_path)],
            check=True,
        )
        print("ops.asset sorok kiírva")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
