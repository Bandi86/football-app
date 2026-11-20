#!/usr/bin/env python3
"""v1 megapack csapat- és ligalogók importja a v2 adatkönyvtárba (S42).

A v1 (`../football-app`) a `dist/assets/megapack/team/<id>.webp` (8729 fájl)
és `dist/assets/league/<id>.webp` (96 fájl) készletet szállítja, az
`image_assets` tábla pedig megmondja, melyik v1 entitáshoz tartozik a fájl.
Ez a script a v2 `core.team` / `core.competition` sorokhoz illeszti a v1
neveket (ékezet-független, zajszó-tűrő pontos egyezés, ország-egyeztetéssel),
tartalom-címzett kulccsal átmásolja a bájtokat a v2 `raw/assets/` alá, és
`ops.asset` sort ír (storage_key = `assets/<aa>/<bb>/<sha>.webp`).

Használat:
    set -a && source .env && set +a
    python3 scripts/import-v1-megapack-logos.py --dry-run
    python3 scripts/import-v1-megapack-logos.py --apply

A script kizárólag a v2 dev DB-be ír (ops.asset), a v1 DB-hez csak olvas.
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import os
import re
import subprocess
import sys
import unicodedata
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
V1_DIST = ROOT.parent / "football-app" / "dist"
NOISE = re.compile(r"\b(fc|cf|sc|afc|ac|as|ss|us|ca|fk|sk|bk|if|club|calcio|cfc|cd|ud|sd)\b")
SPORT_SOCCER = 1


def env(key: str) -> str:
    value = os.environ.get(key)
    if not value:
        sys.exit(f"Hiányzó környezeti változó: {key} (forrásoldd a .env-et)")
    return value


def psql(url: str | None, sql: str, db: str | None = None) -> list[list[str]]:
    command = ["psql", "-At", "-F", "\x1f"]
    if url:
        command += [url]
    if db:
        command += ["-d", db]
    command += ["-c", sql]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        sys.exit(f"psql hiba: {result.stderr.strip()}")
    rows = []
    for line in result.stdout.splitlines():
        if line:
            rows.append(line.split("\x1f"))
    return rows


def fold(name: str) -> str:
    text = unicodedata.normalize("NFKD", name or "")
    text = "".join(ch for ch in text if not unicodedata.combining(ch)).lower()
    text = text.replace("&", " and ")
    text = re.sub(r"[^a-z0-9]+", " ", text)
    text = NOISE.sub(" ", text)
    return " ".join(text.split())


# A v1 adatbázis ISO2, a v2-ben a forrásokból ISO3 kódok is élnek; a
# zászló/logó egyeztetéshez mindkettőt ISO2 családra hozzuk.
ISO3_TO_ISO2 = {
    "ALB": "AL", "ARM": "AM", "AZE": "AZ", "BFA": "BF", "BIH": "BA",
    "BOT": "BW", "BUL": "BG", "CHI": "CL", "CYP": "CY", "ECU": "EC",
    "GEO": "GE", "GIB": "GI", "HKG": "HK", "IDN": "ID", "IND": "IN",
    "KEN": "KE", "KSA": "SA", "KVX": "XK", "LBY": "LY", "LVA": "LV",
    "MAS": "MY", "MKD": "MK", "MLT": "MT", "MNE": "ME", "NIG": "NE",
    "ROM": "RO", "ROU": "RO", "SIN": "SG", "SMR": "SM", "TAN": "TZ",
    "THA": "TH", "UAE": "AE", "UZB": "UZ", "VIE": "VN", "PAR": "PY",
    "PER": "PE", "URU": "UY", "USA": "US", "MAR": "MA", "ALG": "DZ",
    "TUN": "TN", "EGY": "EG", "RSA": "ZA", "NGA": "NG", "GHA": "GH",
    "ISR": "IL", "IRQ": "IQ", "SAU": "SA", "QAT": "QA", "IRN": "IR",
    "JPN": "JP", "KOR": "KR", "CHN": "CN", "AUS": "AU", "BRA": "BR",
    "ARG": "AR", "COL": "CO", "MEX": "MX", "CAN": "CA", "DEN": "DK",
    "SWE": "SE", "NOR": "NO", "SUI": "CH", "AUT": "AT", "BEL": "BE",
    "NED": "NL", "POR": "PT", "GRE": "GR", "TUR": "TR", "POL": "PL",
    "CZE": "CZ", "SVK": "SK", "SVN": "SI", "CRO": "HR", "SRB": "RS",
    "UKR": "UA", "RUS": "RU", "HUN": "HU", "GER": "DE", "FRA": "FR",
    "ITA": "IT", "ESP": "ES", "INT": "XX",
}


def country_family(code: str) -> str:
    code = (code or "").upper()
    if code.startswith("GB"):
        return "GB"
    if code in {"ENG", "SCO", "WAL", "NIR"}:
        return "GB"
    return ISO3_TO_ISO2.get(code, code)


def candidate_key(name: str, country: str) -> tuple[str, str]:
    return fold(name), country_family(country)


def load_v1_assets(entity_type: str, mega: bool) -> list[dict]:
    rows = psql(
        None,
        "SELECT a.core_id, a.file_path, a.source_entity_id, coalesce(c.iso2, ''), "
        "coalesce(e.name, '') "
        "FROM image_assets a "
        f"JOIN {'teams' if entity_type == 'team' else 'tournaments'} e ON e.id = a.core_id "
        "LEFT JOIN countries c ON c.id = e.country_id "
        f"WHERE a.entity_type = '{entity_type}' "
        "ORDER BY a.core_id",
        db="football_app",
    )
    assets = []
    for core_id, file_path, source_entity_id, iso2, name in rows:
        path = V1_DIST / file_path
        if mega and entity_type == "team" and "/megapack/" not in file_path:
            continue
        if entity_type == "tournament" and "/league/" not in file_path:
            continue
        if not path.is_file():
            continue
        assets.append(
            {
                "core_id": int(core_id),
                "path": path,
                "source_entity_id": source_entity_id or "",
                "iso2": iso2,
                "name": name,
            }
        )
    return assets


def load_v2_entities(kind: str) -> dict[tuple[str, str], list[str]]:
    if kind == "team":
        sql = (
            "SELECT t.id::text, t.name, coalesce(c.code, '') FROM core.team t "
            "LEFT JOIN core.country c ON c.id = t.country_id"
        )
    else:
        sql = (
            "SELECT c.id::text, c.name, coalesce(co.code, '') FROM core.competition c "
            "LEFT JOIN core.country co ON co.id = c.country_id"
        )
    index: dict[tuple[str, str], list[str]] = {}
    for entity_id, name, code in psql(env("DATABASE_URL"), sql):
        index.setdefault(candidate_key(name, code), []).append(entity_id)
    return index


def tokens_match(a: str, b: str) -> bool:
    """Teljes vagy rövidített tokenegyezés (pl. `ath bilbao` ~ `athletic bilbao`)."""
    ta, tb = a.split(), b.split()
    if len(ta) != len(tb) or not ta:
        return False
    for x, y in zip(sorted(ta), sorted(tb)):
        if x == y:
            continue
        if min(len(x), len(y)) >= 3 and (x.startswith(y) or y.startswith(x)):
            continue
        return False
    return True


def tokens_subset(a: str, b: str) -> bool:
    """Rövidített név: a rövidebb tokenjei mind megtalálhatók a hosszabbban
    (teljes vagy prefix egyezéssel, legalább 3 karakter), és legfeljebb 2
    extra token van (pl. `Egnatia` ~ `Egnatia Rrogozhine`, `Ath Bilbao` ~
    `Athletic Bilbao`)."""
    ta, tb = sorted(a.split()), sorted(b.split())
    if not ta or not tb or len(ta) > len(tb):
        return False
    if len(tb) - len(ta) > 2:
        return False
    pool = list(tb)
    for x in ta:
        hit = None
        for i, y in enumerate(pool):
            if x == y or (min(len(x), len(y)) >= 3 and (x.startswith(y) or y.startswith(x))):
                hit = i
                break
        if hit is None:
            return False
        pool.pop(hit)
    return True


def resolve(
    index: dict[tuple[str, str], list[str]],
    name: str,
    iso2: str,
    fuzzy_index: list[tuple[str, str, list[str]]],
) -> tuple[list[str], str | None]:
    folded = fold(name)
    family = country_family(iso2)
    exact = index.get((folded, family))
    if exact:
        # Ugyanaz a név és ország-család több v2 entitáson is él (a research és
        # a v1-import külön sort hozott létre): ezek ugyanaz a klub, ezért
        # mindegyik kaphatja ugyanazt a logót.
        return list(exact), None
    same_name = [ids for (folded_name, _), ids in index.items() if folded_name == folded]
    if len(same_name) == 1 and len(same_name[0]) == 1:
        return list(same_name[0]), None
    if not same_name:
        # Rövidített tokenegyezés (Ath Bilbao ~ Athletic Bilbao): országon belül,
        # csak egyértelmű találatnál fogadjuk el.
        prefixed = [
            ids[0]
            for candidate, candidate_family, ids in fuzzy_index
            if candidate_family == family and len(ids) == 1 and tokens_match(folded, candidate)
        ]
        if prefixed:
            return sorted(set(prefixed)), None
        subset = [
            ids[0]
            for candidate, candidate_family, ids in fuzzy_index
            if candidate_family == family
            and len(ids) == 1
            and tokens_subset(folded, candidate)
        ]
        if subset:
            return sorted(set(subset)), None
        # Fuzzy fallback: azonos ország-családon belül, magas hasonlósággal,
        # csak egyértelmű legjobb találatot fogadunk el.
        best: list[tuple[float, str]] = []
        for candidate, candidate_family, ids in fuzzy_index:
            if candidate_family != family or len(ids) != 1:
                continue
            ratio = difflib.SequenceMatcher(None, folded, candidate).ratio()
            if ratio >= 0.86:
                best.append((ratio, ids[0]))
        if not best:
            return [], "nincs név-egyezés"
        best.sort(reverse=True)
        if len(best) == 1 or best[0][0] - best[1][0] >= 0.05:
            return [best[0][1]], None
        return [], "több hasonló jelölt"
    return [], "több országban is létezik, ország nélkül nem dönthető el"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if not args.apply:
        args.dry_run = True

    data_dir = Path(env("DATA_DIR")).resolve()
    db_url = env("DATABASE_URL")
    raw_assets = data_dir / "raw"

    filled: set[str] = set()
    for kind, mega in (("team", True), ("team", False), ("competition", True)):
        label = "csapat" if kind == "team" else "verseny"
        assets = load_v1_assets("tournament" if kind == "competition" else "team", mega)
        index = load_v2_entities(kind)
        fuzzy_index = [(key[0], key[1], ids) for key, ids in index.items()]
        matched = skipped = 0
        statements: list[str] = []
        copies: list[tuple[Path, Path]] = []
        reasons: dict[str, int] = {}
        for asset in assets:
            entity_ids, reason = resolve(index, asset["name"], asset["iso2"], fuzzy_index)
            entity_ids = [entity_id for entity_id in entity_ids if entity_id not in filled]
            if not entity_ids:
                skipped += 1
                key = reason or "már feltöltve"
                reasons[key] = reasons.get(key, 0) + 1
                continue
            digest = hashlib.sha256(asset["path"].read_bytes()).hexdigest()
            key = f"assets/{digest[:2]}/{digest[2:4]}/{digest}.webp"
            target = raw_assets / key
            copies.append((asset["path"], target))
            url = f"file://{asset['path']}"
            attribution = (
                f"v1 megapack (image_assets core_id={asset['core_id']}, "
                f"source_entity_id={asset['source_entity_id'] or '-'})"
            )
            for entity_id in entity_ids:
                statements.append(
                    "INSERT INTO ops.asset "
                    "(id, entity_kind, entity_id, kind, url, storage_key, source, licence, "
                    "licence_status, attribution, captured_at) VALUES "
                    f"('{uuid.uuid4()}', '{kind}', '{entity_id}', 'logo', "
                    f"'{escape(url)}', '{key}', 'v1-megapack', 'v1 megapack', 'unknown', "
                    f"'{escape(attribution)}', now()) "
                    "ON CONFLICT (entity_kind, entity_id, kind, url) DO NOTHING"
                )
                filled.add(entity_id)
                matched += 1

        print(f"{label} ({'megapack' if mega else 'legacy'}): {matched} illesztve, {skipped} kihagyva")
        for reason, count in sorted(reasons.items(), key=lambda item: -item[1])[:6]:
            print(f"  - {reason}: {count}")
        if args.dry_run:
            continue
        for source, target in copies:
            target.parent.mkdir(parents=True, exist_ok=True)
            if not target.exists():
                target.write_bytes(source.read_bytes())
        if statements:
            chunk = 200
            for start in range(0, len(statements), chunk):
                sql = "BEGIN;\n" + ";\n".join(statements[start : start + chunk]) + ";\nCOMMIT;"
                psql(db_url, sql)


def escape(value: str) -> str:
    return value.replace("'", "''")


if __name__ == "__main__":
    main()