# Football App v2 — élő cél- és állapottérkép

Frissítve: 2026-11-20. A **DB és a legutóbbi checkpoint az igazság**, ez a fájl csak navigáció.

## Források, amiket minden munka előtt olvasni kell
1. `AGENTS.md` (szerződés) → 2. `README.md` (aktuális állapot) → 3. `docs/superpowers/specs/2026-11-09-football-app-v2-design.md` →
4. a legutóbbi `docs/superpowers/plans/*` → 5. `docs/checkpoints/<legutóbbi>/checkpoint.md` → 6. `git status --short --branch`.

A `docs/superpowers/plans/2026-11-17-next-slices.md` **elavult** (S21–S25 ütemezés, 2026-11-17-i állapot);
a jelenlegi helyzetet a README + 2026-11-20 checkpoint írja le.

## Hová tartunk (a spec fázisterve)
| Fázis | Cél | Jelenleg (2026-11-20) |
|---|---|---|
| 0 | Alapozás, CI, migrációs fegyelem | Kód/migrációk kész; **CI (.github/workflows) HIÁNYZIK** |
| 1 | Identitás, gyűjtés, coverage, kapu | Üzemel: 4 forrás, fail-closed identitás, L0–L8 kapu, 287 664 meccs |
| 2 | Meccsközpont (Ma/verseny/meccs/csapat/játékos/keresés/Ops) | Nagy rész kész; **keresés, játékosoldal teljes adattartalma, VoiceOver/billentyűzet-ellenőrzés, OpenAPI↔Swift kontraktteszt nyitott** |
| 3 | Live eseményfolyam | Nem kezdődött |
| 4 | Predikció (1X2/OU2.5/BTTS, kalibráció) | Nem kezdődött; csak Elo-számítás van |
| 5 | Botok, papírszelvények | Nem kezdődött |
| 6 | Autonómia, önfejlesztés | Részben (worker/breaker/audit), teljes kép nem |

A fázisok nem ugorhatók át: a live nem helyettesíti az adatminőséget, a predikció nem indul mérési kapu nélkül (spec §12).

## Aktuális mérőszámok (dev DB, 2026-11-20)
- 287 664 kanonikus meccs; 2006–2026: 189 310 meccs, **96,7% eredmény / 86,1% odds**.
- `reverify 2006–2027`: 193 658 meccs, 0 hiba; `integrity`: 0 duplikátum, 0 árva, 0 review, 0 withheld, 0 failed job, 0 breaker, 0 feloldatlan konfliktus.
- Tesztek: Rust workspace 85 csoport / 0 failure (`--test-threads=1`), Swift 26/26, clippy `-D warnings`, fmt, build `--features obscura`.
- Kapu-lánc: `scripts/db-reset.sh` → `check-foundation.sh` → cargo test → clippy → fmt → build → db-reset.

## Kész / nincs kész (rövid)
**Kész:** Rust workspace + daemon + CLI, 4 élő forrás adapter, identitás/review/merge, dedupe, verifikáció és publish gate,
napi worker + launchd, audit/ledger, v1 szelektív import 1993–2026, Elo, opcionális API-integritás és SwiftUI Ops kártya, napi checkpointok.
**Nincs kész / nyitott:** CI, socerway JS dayPicker (múltbeli nap fetch; 2026-11-20-i probe: csak 622 KB-os héj),
futó daemon kézi frissítése az új API-mezőhöz (a gateway blokkolta a telepítőt), 2026/27 odds, VoiceOver/akadálymentességi
és billentyűzetes UI-ellenőrzés, OpenAPI↔Swift kontrakt, keresés és teljes játékosoldal, live/predikció/bot fázisok.

## Következő döntési pontok (prioritási sorrend)
1. **P0** — XCode-ban futtatható UI-ellenőrzés + VoiceOver/billentyűzet, mert a spec DoD-je szerint „kész” csak így lehet (lásd `swiftui.md`).
2. **P0** — CI a kapu-láncból (`.github/workflows`), különben a spec 0. fázisa formálisan nem zárt.
3. **P1** — OpenAPI↔Swift DTO-kontrakt teszt, hogy a kliens és az API ne térjen el csendben.
4. **P1** — A futó agent kézi frissítése (felhasználói lépés a gateway-blokk miatt).
5. **P2** — soccerway dayPicker újrapróbálása, ha az oldal ismét teljes DOM-ot ad; 2026/27 odds források.
6. **P2** — Fázis 3 (live) csak a fázis-2 kilépési feltétel után.

## Definition of Done
A spec §17 szerint: viselkedés megfelel a specnek, domain-invariánsok teszteltek, migráció+visszaállítás dokumentált,
API- és kliensszerződés egyezik, hiány/hiba/konfliktus kezelt, adat visszavezethető, teljesítmény kereten belül,
billentyűzetes+akadálymentes használat ellenőrzött, pontos build/teszt eredmény rögzített, korlátok kimondottak,
minden új adathiba mögött szabály/fixture/tudatos kivétel van.
