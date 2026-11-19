# S26 — Kereszt-forrás kickoff-rekonciliáció (megvalósítási terv)

Dátum: 2026-11-19. Spec: `../specs/2026-11-19-s26-kickoff-reconciliation-design.md`.
Feltétel: a spec felhasználói jóváhagyása (új merge-szabály, ~4 600 kanonikus sor).

## Lépések (commit-per-green)

1. **S26a — storage kulcs + őr**: `duplicate_fixture_groups` (competition +
   season + LEAST/GREATEST csapat-pár + UTC-nap), `is_same_fixture`
   (orientáció-helyes eredmény-egyezés vagy egyik hiányzik; eltérő → false).
   Tesztek: azonos nap/különböző kickoff egyező eredménnyel → csoport;
   eltérő eredmény → őr elutasít; tükrözött orientáció normalizálva.
2. **S26b — merge-út**: a meglévő `merge_duplicate` tranzakció újrahasználva
   (archívum + provenance + additív odds/stats + gate + audit); a payload
   tartalmazza mindkét kickoffot és a forrás-prioritás döntését. Túlélő:
   footballdata-provenance, tie-break legkorábbi created_at. Teszt: archívum
   payload, gate-állapot, árva provenance 0.
3. **S26c — CLI**: `dedupe fixtures [--date] [--competition] [--apply]`
   (dry-run alap) + `--report` (a kockázatos párok tételesen: csapatok, nap,
   kickoffok, eredmények, források). Teszt: dry-run nem ír.
4. **S26d — élő futtatás**: dry-run riport → ellenőrzés → `--apply` az egész
   DB-re (4 615 biztonságos pár); a 13+1 kockázatos pár tételes döntése
   (withhold / forrás-prioritás / kézi javítás, auditálva).
5. **S26e — csapatnév-változat audit**: exact-name + reviewed-alias alapú
   párok listája (pl. Man United vs Manchester United); merge csak a meglévő
   `identity merge-team` eszközzel (azonos név + kontextus) vagy review-alias
   jóváhagyással — fuzzy tilos. Ami nem oldható meg egzaktul: backlog.
6. **S26f — zárás**: `reverify --from 2015-01-01 --to 2027-12-31`, coverage
   újramérés (PL 2015/16 380, Eliteserien valós szezonméret), 0 fixture-csoport
   (review-listán kívül), teljes kapu-lánc, checkpoint, commit.

## Kapuk minden lépésnél

`cargo fmt --all` → `cargo test --workspace -- --test-threads=1` →
`cargo clippy --workspace --all-targets -- -D warnings` → `cargo fmt --all --check`;
adat-művelet után SQL-ellenőrzés (csoportok, árva provenance, gate).

## Visszaállítás

`ops.dedupe_archive` (teljes payload, mindkét kickoff) → kézi visszajátszás;
a kód `git revert`; a review-listás párokhoz nincs írás.