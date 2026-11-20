# Fejlesztési felkészültség és készség-/dokumentáció-audit (2026-11-20)

Ez a dokumentum arra válaszol: megvan-e a tudás, a dokumentáció és a tesztfegyver ahhoz, hogy a v2-t
a specifikáció szerint folytassuk. A tények élőlekérdezésből és a repó fájljaiból származnak.

## 1. Skill-fedettség
| Terület | Skill | Állapot |
|---|---|---|
| Projektvezetés, szeletek, kapuk, DB | `football-app-v2` (+ `references/data-ops.md`, `db-queries.md`, `gapfill-and-result-revisions.md`, `roadmap.md`, `rust-backend.md`, `swiftui.md`) | Megvan; 2026-11-20-ban 26 KB-ról 12,6 KB-re bontva |
| Adatkutatás, források, odds, charter 9 | `football-data-research`, `football-app-monitoring`, `obscura`, `blocked-page-recovery`, `open-data-source-audit`, `grounded-citations` | Megvan |
| Döntések nyomon követése | `memory-engineering` (ledger.py, L0001–L0026) | Megvan |
| Általános szoftverfejlesztés | `tdd`, `test-driven-development`, `systematic-debugging`, `code-review`, `requesting-code-review`, `simplify-code`, `codebase-design`, `writing-for-agents` | Megvan, de a napi szeleteknél nem mindegyik lett betöltve |
| SwiftUI specifikus | **repo-skill kész**: `.dsh/skills/swiftui-macos-client/SKILL.md` | Megvan (2026-11-20, S30f); a community `swiftui` bundle nem kell |
| Rust specifikus | **repo-skill kész**: `.dsh/skills/rust-workspace-backend/SKILL.md` | Megvan (2026-11-20, S30f); a community `rust` bundle nem kell |
| Projekt-skill DSH-ban | `.dsh/skills/football-app-v2/` (SKILL.md + 6 reference + script) | Megvan; a DSH katalógus élőben felveszi (system reminder igazolta) |

A skill-fedettség 2026-11-20-án (S30f) lezárva: a három skill a repóban él, a DSH automatikusan
katalógusba veszi, és két próba-ágens igazolta, hogy a kérdésekre a skillből, rövid ellenőrző
olvasással válaszolnak. A korábbi hiány oka az volt, hogy a skillek csak a hermes harness alatt
(`~/.hermes/skills/`) léteztek, a DSH profilnak nincs saját skills mappája.

## 2. Dokumentáció: mi van lefektetve
- `AGENTS.md`: kötelező olvasási sorrend, modulhatárok, DB/migrációs szabályok, tesztelési és git-szabályok, kommunikáció.
- `README.md`: friss állapot (2026-11-20), architektúra, alapelvek.
- Spec: `docs/superpowers/specs/2026-11-09-football-app-v2-design.md` — termékcélok, nem-célok, 4 munkamód,
  oldalak pontos tartalma, adatminőségi felületi szerződés, UX/akadálymentesség, API, adatmodell,
  forrás- és lefedettségi szabályok, L0–L8 kapu, 0–6 fázisterv, sikermérők, **Definition of Done (§17)**.
- Tervek: 18 megvalósítási terv a `docs/superpowers/plans/` alatt; a `2026-11-17-next-slices.md` **elavult**
  (S21–S25 ütemezés), az élő célok most a skill `references/roadmap.md`-ben vannak.
- Napi checkpointok: 2026-11-10 … 2026-11-20; üzemeltetési útmutatók 15 téma; döntések, research leltár.
- **Ellentmondás:** `AGENTS.md` §11 még azt írja, „korai alapozás, nincs valódi adat/UI, nincs auto-commit” —
  ez 2026-11-20-ra nem igaz; a/README és a checkpointok az igazság. Javítva: §11 most a README-re+checkpointra mutat.

## 3. Tesztelés: mi van és mi nincs
| Követelmény (spec §11 / AGENTS §8) | Jelenleg |
|---|---|
| domain unit + storage integráció + parser golden + API route | Megvan; Rust workspace 85 csoport / 0 failure (`--test-threads=1`) |
| Swift logika/állapot teszt | Megvan: 26/26 |
| Teljes kapu (`check-foundation`, clippy `-D warnings`, fmt, build `--features obscura`, DB reset) | Megvan, futott: EXIT 0 |
| Adatintegritás (`integrity`, `reverify`, audit, review) | Megvan: 0 hiba / 0 withheld / 0 konfliktus |
| **CI workflow** | **Nincs `.github/workflows`** — a spec 0. fázisa formálisan nem zárt (következő P0) |
| **OpenAPI↔Swift kontrakt** | **Nincs teszt** |
| **Manuális macOS UI + VoiceOver + billentyűzet** | Címkék és 4 billentyű-parancs bevezetve (6/6 Core + 24/24 app teszt); a **tényleges VoiceOver/Tab-futás még nincs rögzítve**, lásd `ui-accessibility-checklist.md` |
| **Backup-restore próba, p95 teljesítmény** | **Nincs mérés** |

## 4. Következtetés és javasolt sorrend
A tudás és a projekt-specifikus dokumentáció **elegendő** a backend és az adatmunka folytatásához; a SwiftUI-oldalon
van egy valós DoD-rés (akadálymentesség + kézi UI-ellenőrzés + kontrakt), és a minőségi kapu hiányos a CI miatt.

**Frissítés (S30f, 2026-11-20):** a hiányzó SwiftUI/Rust tudás repo-skillként pótolva
(`.dsh/skills/swiftui-macos-client`, `.dsh/skills/rust-workspace-backend`), a DSH katalógus élőben
látja őket, és két próba-ágens igazolta a használhatóságukat. Az ágens-csapat (`agents/`) felállítva:
hat szerep, builder ≠ verifier, diszjunkt írási hatókör. A lentmaradó P0/P1 sorrend változatlan.
1. P0 (kód kész, emberi ellenőrzés hátra): VoiceOver/Tab-futás a `ui-accessibility-checklist.md` szerint.
2. P0: CI a meglévő kapu-láncból.
3. P1: OpenAPI↔Swift DTO kontrakt teszt.
4. P1: futó daemon frissítése (felhasználói kézi lépés, a gateway blokkolta a telepítőt).
5. P2: soccerway dayPicker és 2026/27 odds, majd a spec 3. fázisa (live) csak a 2. fázis lezárása után.
