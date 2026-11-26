# Terv — S4: egységes meccs-szerkesztő API (S173)

## Cél és határ

Új `PUT /v1/match/{id}/edit` végpont az S168/S170 döntések szerint. A régi
`PUT /v1/match/{id}/manual-result` **kompatibilitási okból változatlanul él**.
A kliens (`apps/**`) párhuzamosan dolgozik, ebben a szeletben nem érintjük.

Írási határ: `rust/**`, `db/migrations/**` (csak ha feltétlenül kell),
`tests/fixtures/**`, ez a dokumentum. Éles DB-be nem írunk, daemon nem lesz
újratelepítve, research job nem indul. Commit csak zöld kapu után.

## Szerződés

- **Scope:** ops token (`authorize(Scope::Ops)`); read token 401, hiányzó/rossz
  token 401. Magyar hibaüzenetek: 400 érvénytelen UUID / enum / hiányzó indoklás,
  404 ismeretlen meccs, 409 natural-key ütközés, 500 váratlan írási hiba.
- **Whitelist (küldhető mezők):** `kickoff_at`, `kickoff_precision`, `status`,
  `home_ft`, `away_ft`, `home_ht`, `away_ht`, `home_et`, `away_et`, `home_pens`,
  `away_pens`, `decided_by`, `note`, `idempotency_key`. Minden más mező
  figyelmen kívül marad (a szerializáló `deny_unknown_fields` nélkül is csak a
  felsorolt mezők kerülnek az editbe).
- **Régi értékek megőrzése:** a whitelistból kimaradó vagy `null` mező nem
  íródik felül; az eredmény-sor csak akkor cserélődik, ha érkezik eredmény-mező.
  A domain `validate_manual_edit` szabályai változatlanul érvényesek.
- **Bizonyíték:** egy tranzakcióban `raw.artifact` + `raw.capture` +
  `stage.observation` + `fact.provenance` (csak a ténylegesen változott
  mezőkre) + `ops.audit_event` (`before`/`after`, note, observation_id).
  Policy verzió az új úton `manual-full-1.0`; a régi út `manual-2.0` marad.
  A handler a meglévő `reverify_match` úttal frissíti a publish gate-et.
- **Idempotencia:** az opcionális `idempotency_key` (vagy a whitelist
  kanonikus lenyomata) stabil `request_key`-t ad; azonos kulcsú ismételt
  kérés új observation/audit helyett a meglévő azonosítókat adja vissza,
  `reused: true` jelzéssel. A meccs sor `FOR UPDATE` zárja sorosítja.

## TDD lépések (RED → GREEN)

1. `rust/crates/api/tests/match_edit.rs`: ops scope, 400/401/404, kickoff
  mellett a status megőrzése, provenance+audit, két azonos kulcsú hívás
  idempotenciája, kontrakt-fixture.
2. OpenAPI: új `#[utoipa::path]`, séma-regisztráció, útvonal-szám 22 → 23,
  `every_documented_path_is_routed` és a bearer-séma-teszt automatikusan fedi.
3. Kontrakt-fixture: `tests/fixtures/contract/match-edit-response.json`,
  mezőkészlet-egyeztetés a `MatchEditResponse` sémával.
4. Implementáció: `routes/edit.rs` + router + storage-opciók
   (`idempotency_key`, `policy_version`, `reused`).

## Kapu-lánc (ebben a sorrendben, mért számokkal)

```
cargo fmt --all
./scripts/db-reset.sh
./scripts/check-foundation.sh
cd rust && cargo test --workspace -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo build -p footballd -p footballctl --features obscura
cd .. && ./scripts/db-reset.sh
git diff --check
```

A kézi írású kód miatt a formázás az első lépés. A teszt-DB közös, ezért
`--test-threads=1`. A bináris build `--features obscura` nélkül nem megfelelő.

## Visszaállás

A szelet nem tartalmaz migrációt és nem ír éles DB-t. Visszaállás: a commit
előtt `git checkout -- <fájlok>`, illetve a commit `git revert <sha>`. A régi
végpont érintetlen, így a kliens kompatibilitása nem sérül.

## Nyitva maradhat

A teljes identitás-szerkesztés (csapat/verseny/szezon/forduló IDszerű
kiválasztása) és a logófeltöltés nem része ennek a szeletnek (S5, illetve a
spec 5. szakaszának későbbi bontása); a két eltolódott kickoff javítása csak
előkészített payload, alkalmazás nélkül.
