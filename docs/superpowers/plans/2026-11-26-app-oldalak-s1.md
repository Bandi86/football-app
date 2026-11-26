# 2026-11-26 — App-oldalak bővítése, S1: adatmodell + API alap — megvalósítási terv

Spec: [`2026-11-25-app-oldalak-bovites-design.md`](../specs/2026-11-25-app-oldalak-bovites-design.md).
Döntések: [`2026-11-26-app-oldalak-bovites.md`](../../decisions/2026-11-26-app-oldalak-bovites.md).
Branch: `main` (a felhasználó folyamatos felhatalmazása mellett; commit csak a zöld kapu után).
**S1 után megállok és jelzem** — S2-re nem lépek tovább újabb döntés nélkül.

## Határ (mi most készül, mi nem)

Most: két új ops tábla + `ops.asset` bővítés, domain-enumok és szabály,
storage-repozitóriumok, 5 új API-végpont, OpenAPI-szinkron, kontrakt- és
integrációs tesztek, migráció a teszt- és dev DB-re.

**Nem most:** a hiányjelek detektorának bekötése a `daily-check`/
`integrity` futásához (a döntés szerint az adatbázisba ír, de a detektor
S3-ban kerül be — S1-ben a repository `upsert_seen`/`resolve` írási útja
van meg és van tesztelve, így a későbbi bekötés nem igényel új sémát);
logófeltöltés API/UI (S5), bármely SwiftUI-változás (S2+), AI-gomb (S8).

A `GET /v1/issues` S1-ben még üres listát ad élesben, mert a detektor még
nem ír — ezt a zárójelentésben expliciten kimondom, nem „kész"-ként
tüntetem fel.

## Döntések alkalmazása

1. Minden írás (jegyzet, később logó) **ops tokenes**; olvasás read tokenes.
2. A `PUT /v1/match/{id}/manual-result` változatlan marad; az egységes
   `PUT /v1/match/{id}/edit` az S4-ben jön.
3. Az `ops.issue` perzisztens, a forrás (`daily-check`, `integrity`, `manual`,
   `agent`) által upsertelt életciklus-tábla.
4. A jegyzeteket read tokennel lehet olvasni külön végponton, de **nem**
   kerülnek a napi feedbe (`/v1/day/{date}` változatlan).
5. Első szeletben csak WebP/PNG; az `ops.asset`új oszlopai még nem járnak
   feltöltési végponttal.
6. AI-gomb alapból ki; S8 külön döntés.

## Séma indoklása — miért nem `ops.validation_finding`

A dev DB-ben már van `ops.validation_run` / `ops.validation_finding`
(szabálykód, severity, field_key, üzenet, observation_id, futási idő).
Az `ops.issue` **nem** ezek duplikálása: futásokon átívelő, natural-key
szerint upsertelt állapotot hordoz (`first_seen_at`, `last_seen_at`,
`resolved_at`, `source`, nyitott sorra részleges unique index). A nyers
szabályeredmény továbbra is a `validation_finding`-ben él; az `issue`
csak a „mióta látszik / megoldódott-e” kérdést válaszolja meg.

## Szelet lépései

### S1a — Migráció és infrastruktúra

- [ ] `db/migrations/0028_ops_issues_notes.sql` (`BEGIN`…`COMMIT`, magyar
      visszaállítási megjegyzéssel):
  - `ops.issue`: `id uuid PK`, `subject_kind text CHECK (match|team|competition)`,
    `subject_id uuid NOT NULL` (polimorf, mint az `audit_event`), `code text CHECK`
    (a spec 7 kódja), `severity text CHECK (info|warning|hard)`, `detail jsonb NOT NULL
    DEFAULT '{}'`, `first_seen_at`, `last_seen_at`, `resolved_at`, `source text CHECK
    (daily-check|integrity|manual|agent)`, partial UNIQUE
    `(subject_kind, subject_id, code, source) WHERE resolved_at IS NULL`, indexek
    `(subject_kind, subject_id)` és `(last_seen_at DESC) WHERE resolved_at IS NULL`.
  - `ops.match_note`: `id uuid PK`, `match_id uuid FK core.match ON DELETE CASCADE`,
    `author text NOT NULL`, `body text NOT NULL` (trimelt nem üres CHECK), `kind text
    CHECK (info|problem|fix|question|resolved)`, `created_at`, `updated_at`,
    `resolved_at`, index `(match_id, created_at DESC)`.
  - `ops.asset`: `origin text NOT NULL DEFAULT 'import' CHECK (import|user_upload)`,
    `uploaded_by`, `original_filename`, `mime_type` (mind nullable; adatot nem módosít).
- [ ] `scripts/db-check.sh`: `expected_tables` **54 → 56**; a teszt-DB üresség-
      ellenőrzésébe az `ops.issue` és `ops.match_note` sorok is bekerülnek.
- [ ] `rust/crates/storage/src/test_support.rs`: TRUNCATE-lista bővítése a két
      új táblával (migráció automatikus a `sqlx::migrate!`-ből).
- [ ] Migráció futtatása előbb a teszt-, majd a dev DB-re; rollback dokumentálva.

### S1b — Domain és storage

- [ ] `rust/crates/domain/src/ops.rs` (új modul, `lib.rs`-ben regisztrálva):
    `IssueSeverity`, `IssueSubjectKind`, `IssueCode`, `IssueSource`, `NoteKind`
    (`serde(snake_case)` + `as_db_str()`/`parse()`, `utoipa::ToSchema` mint a
    `verification::Severity`), és `validate_note_body()` tipizált magyar
    `NoteError`-rel. Az API-handler nem tartalmaz üzleti szabályt.
- [ ] `rust/crates/storage/src/repositories/issue.rs` (új):
    `IssueRow` (`FromRow`, `Serialize`, `ToSchema`), `IssueFilter`
    (`scope: Day(NaiveDate)|All`, `code`, `severity`, `open_only`), `list`,
    `get`, `upsert_seen` (partial unique-on-conflict, `first_seen_at` megőrzése,
    `last_seen_at=now()`), `resolve`. Day-scope:
    `(m.kickoff_at AT TIME ZONE $2)::date = $1` a `day.rs` mintájára
    (`Europe/Budapest` paraméter).
- [ ] `rust/crates/storage/src/repositories/note.rs` (új): `MatchNoteRow`,
    `list_for_match`, `get`, `insert`, `update` (body/kind/resolved_at).
- [ ] `rust/crates/storage/src/repositories/history.rs` (új): `HistoryEntry`,
    `list_for_match` — `ops.audit_event` (before/after a `detail` jsonb-ben) és
    a kiválasztott `fact.provenance` sorok (`field_key`, `valid_from`,
    `stage.observation → ops.source.code`) egységes, időrendi feedje.
- [ ] `repositories/mod.rs` alphabetikus bővítése.
- [ ] Storage integrációs teszt `rust/crates/storage/tests/issues_notes.rs`:
    upsert-idempotencia, nyitott sor unique, first/last_seen viselkedés,
    resolve, day/all/code/severity szűrés, notes CRUD, history két forrásból,
    üres eredmény.

### S1c — API

- [ ] `routes/issues.rs`: `GET /v1/issues?scope=day&date=…&code=…&severity=…&open_only=…`
      — **Read** scope, `IssuesResponse { issues, count }`; magyar 400 a rossz
      dátumhoz / scope-hoz / enum-hoz.
- [ ] `routes/notes.rs`: `GET /v1/match/{id}/notes` (Read),
      `POST /v1/match/{id}/notes` (Ops, 201 + `note_added` audit),
      `PATCH /v1/note/{id}` (Ops, `note_edited` audit, 404 ismeretlen ID-ra).
- [ ] `routes/history.rs`: `GET /v1/match/{id}/history` (Read, 404 ismeretlen
      meccsre).
- [ ] `routes/mod.rs`, `lib.rs`: 5 új route regisztrációja.
- [ ] `openapi.rs`: 5 új `#[utoipa::path]`, új `ToSchema` típusok a
      `components(schemas(...))`-ban, új tag (`jegyzet` / `hiány`), a beégetett
      **15 → 20** útvonal-szám; `tests/openapi.rs` ugyanazon száma **15 → 20**,
      `sample_method` POST/PATCH body-val bővül.
- [ ] Kontrakt: `tests/fixtures/contract/issues-response.json` + új
      `rust/crates/api/tests/issues_contract.rs` (teljes szerializációs
      egyenlőség, mint a `day_contract.rs`).
- [ ] API integrációs teszt `rust/crates/api/tests/issues_notes.rs`:
      read/ops auth-mátrix (read írásra 401), 400/404 esetek, audit-esemény
      (`note_added`, `note_edited`) ellenőrzése SQL-lel.

### S1d — Kapu és rögzítés

- [ ] `cargo fmt --all`, `./scripts/db-reset.sh`, `./scripts/check-foundation.sh`,
      `cargo test --workspace -- --test-threads=1`,
      `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo build -p footballd -p footballctl --features obscura`,
      `./scripts/daily-check.sh 2026-11-25` (EXIT=0), `footballctl integrity`
      a két érintett napra, `git diff --check`.
- [ ] A futó daemon **nem** lesz újratelepítve: S1-ben nincs UI, a telepített
      binárisnak nem kell az 5 új route; ezt a zárójelentésben kimondom.
      (S2-ben, a kliens bekötésekor kötelező lesz az `install-footballd-agent.sh`.)
- [ ] A napi checkpoint S168 szakasza: változott fájlok, futtatott parancsok
      mért eredménnyel, nem ellenőrzött részek, kockázat, visszaállás.
- [ ] Commit a zöld kapu után (általános felhatalmazás); **S2-re nem lépek tovább**.

## Kockázatok

- A partial unique index miatt az `ON CONFLICT` záradéknak pontosan ugyanazt a
  predikátumot kell használnia — teszt nélkül csendben duplikálódhatna.
- A `db-check.sh` táblaszámát és a TRUNCATE-listát elfelejteni könnyű; a
  `check-foundation.sh` ezt elsőkapuként ellenőrzi.
- Az `ops.asset` oszlopai későbbi S5-ös feltöltési végpontot feltételeznek;
  most csak séma, nem használja semmi (a default `'import'` reprodukálja
  a meglévő sorok értékét).
- A `GET /v1/issues` üres marad a detektor bekötéséig — ezt nem szabad
  „működő hiányjelzésként" kommunikálni.

## Visszaállás

- Minden szelet külön commit → `git revert <commit>`.
- Migráció: `DROP TABLE ops.match_note; DROP TABLE ops.issue;` és
  `ALTER TABLE ops.asset DROP COLUMN origin, uploaded_by, original_filename, mime_type;`
  majd a `_sqlx_migrations` 0028-as sorának törlése — külön, egyeztetett lépés.
- Tesztadatok: `./scripts/db-reset.sh` (csak `football_v2_test`).
- A két új tábla nem tartalmaz éles adatot; jegyzet/issue sorokat a
  repository `resolve`/törlés útján, audit eseménnyel lehet visszaállítani.
