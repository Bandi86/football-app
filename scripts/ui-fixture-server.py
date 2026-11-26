#!/usr/bin/env python3
"""Determinisztikus fixture HTTP-szerver az S30h automata UI-teszthez.

A tesztek nem indíthatnak saját socketet (a macOS UI-test runner sandboxja
EPERM-mel tiltja), ezért a fixture-adatot ez a folyamat szolgálja ki.

Használat: ui-fixture-server.py --port 18080 --fixture tests/fixtures/ui/loaded-day.json
"""
import argparse
import http.server
import json
import sys
import threading


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--fixture", required=True)
    parser.add_argument("--detail-fixture", help="opcionális meccs-részletező JSON")
    parser.add_argument("--detail-map-fixture", help="UUID szerinti meccs-részletező JSON-térkép")
    parser.add_argument("--search-fixture", help="opcionális globális keresési JSON")
    parser.add_argument("--player-fixture", help="opcionális játékosadatlap JSON")
    parser.add_argument(
        "--match-micro-fixture",
        help="opcionális meccs-mikro JSON (lineups/player_stats/shots kulcsokkal)",
    )
    parser.add_argument("--team-fixture", help="opcionális csapatadatlap JSON")
    parser.add_argument("--competition-fixture", help="opcionális versenyadatlap JSON")
    parser.add_argument("--ops-status-fixture", help="opcionális üzemeltetési állapot JSON")
    parser.add_argument("--issues-fixture", help="opcionális hiányjel-listája JSON")
    parser.add_argument("--history-fixture", help="opcionális meccs-idővonal JSON")
    parser.add_argument("--ops-summary-fixture", help="opcionális napi összefoglaló JSON")
    parser.add_argument("--ops-audit-fixture", help="opcionális globális audit JSON")
    args = parser.parse_args()

    with open(args.fixture, "rb") as handle:
        payload = handle.read()
    json.loads(payload)

    detail_payload = None
    if args.detail_fixture:
        with open(args.detail_fixture, "rb") as handle:
            detail_payload = handle.read()
        json.loads(detail_payload)

    detail_map = {}
    if args.detail_map_fixture:
        with open(args.detail_map_fixture, "rb") as handle:
            detail_map = json.load(handle)
        if not isinstance(detail_map, dict):
            raise ValueError("--detail-map-fixture must contain a JSON object")

    search_payload = None
    if args.search_fixture:
        with open(args.search_fixture, "rb") as handle:
            search_payload = handle.read()
        json.loads(search_payload)

    player_payload = None
    if args.player_fixture:
        with open(args.player_fixture, "rb") as handle:
            player_payload = handle.read()
        json.loads(player_payload)

    def read_fixture(path):
        if not path:
            return None
        with open(path, "rb") as handle:
            content = handle.read()
        json.loads(content)
        return content

    micro_payload = read_fixture(args.match_micro_fixture)
    ops_status_payload = read_fixture(args.ops_status_fixture)
    issues_payload = read_fixture(args.issues_fixture)
    history_payload = read_fixture(args.history_fixture)
    summary_payload = read_fixture(args.ops_summary_fixture)
    audit_payload = read_fixture(args.ops_audit_fixture)
    team_payload = read_fixture(args.team_fixture)
    competition_payload = read_fixture(args.competition_fixture)

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self):  # noqa: N802 - http.server API
            path = self.path.split("?", 1)[0]
            if path.startswith("/v1/day/"):
                body, status = payload, 200
            elif path == "/v1/ops/status" and ops_status_payload is not None:
                body, status = ops_status_payload, 200
            elif path == "/v1/issues" and issues_payload is not None:
                body, status = issues_payload, 200
            elif path == "/v1/ops/summary" and summary_payload is not None:
                body, status = summary_payload, 200
            elif path == "/v1/ops/audit" and audit_payload is not None:
                body, status = audit_payload, 200
            elif path.endswith("/history") and path.startswith("/v1/match/") and history_payload is not None:
                body, status = history_payload, 200
            elif micro_payload is not None and path.startswith("/v1/match/") and any(
                path.endswith(suffix) for suffix in ("/lineups", "/player-stats", "/shots")
            ):
                key = path.rsplit("/", 1)[-1]
                key = "player_stats" if key == "player-stats" else key
                document = json.loads(micro_payload)
                body = json.dumps(document[key], ensure_ascii=False).encode("utf-8")
                status = 200
            elif path.startswith("/v1/match/"):
                match_id = path.removeprefix("/v1/match/").lower()
                if match_id in detail_map:
                    body = json.dumps(detail_map[match_id], ensure_ascii=False).encode("utf-8")
                    status = 200
                elif detail_payload is not None:
                    body, status = detail_payload, 200
                else:
                    body, status = b'{"error":"not found"}', 404
            elif self.path.startswith("/v1/search") and search_payload is not None:
                body, status = search_payload, 200
            elif self.path.startswith("/v1/player/") and player_payload is not None:
                body, status = player_payload, 200
            elif path.startswith("/v1/team/") and team_payload is not None:
                body, status = team_payload, 200
            elif path.startswith("/v1/competition/") and competition_payload is not None:
                body, status = competition_payload, 200
            elif self.path == "/v1/health":
                body, status = b'{"status":"ok","service":"footballd"}', 200
            else:
                body, status = b'{"error":"not found"}', 404
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.send_header("Connection", "close")
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, fmt, *args):  # csendes napló
            pass

    server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    print(f"ui-fixture-server listening on 127.0.0.1:{args.port}", flush=True)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        threading.Event().wait()
    except KeyboardInterrupt:
        pass
    finally:
        server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
