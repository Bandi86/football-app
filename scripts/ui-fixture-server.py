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

    class Handler(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.1"

        def do_GET(self):  # noqa: N802 - http.server API
            if self.path.startswith("/v1/day/"):
                body, status = payload, 200
            elif self.path.startswith("/v1/match/"):
                match_id = self.path.removeprefix("/v1/match/").split("?", 1)[0].lower()
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
