# Identitás-réteg üzemeltetési útmutató (S2b-1)

**Dátum:** 2026-11-13
**Hatókör:** a fail-closed linker (`football-identity`), a crosswalk/alias/
review_queue táblák (0005), a `footballctl identity` review-parancsok és a
tick live ágának identitás-feloldása.
**Kapcsolódó dokumentumok:**
[S2b-1 spec](../superpowers/specs/2026-11-13-s2b1-identity-layer-design.md),
[S2b-1 terv](../superpowers/plans/2026-11-13-s2b1-identity-layer.md),
`db/migrations/0005_identity_layer.sql`,
[worker útmutató](worker.md).

Minden parancs a repo gyökeréből fut, a környezet betöltése után:

```bash
set -a; . ./.env; set +a
```

## 1. A linker feloldási sorrendje (fail-closed)

A `football_identity::linker::resolve` a forrás-entitást (forrás, kind,
external ID, javasolt név, kontextus) a kanonikus entitás-térbe oldja fel.
A sorrend szigorú; az első egzakt találat nyer, fuzzy soha nem ad Exact-et:

1. **Egzakt külső ID:** aktív `identity.source_entity_ref` a (forrás, kind,
   external ID) kulcsra → `Exact`. A ref method-ja szerint:
   `exact_external_id`/`fixture_seed` → `ExactExternalId`,
   `manual_review` → `ManualReview`.
2. **Crosswalk-lánc:** aktív strong/medium `identity.crosswalk` kötés →
   cél-forrás aktív ref-je → `Exact(CrosswalkChain)`, csak ha a lánc minden
   eleme aktív. A **weak rung teljesen kihagyott**: sem Exact-et, sem
   jelöltet nem ad (S2b-1 dokumentált egyszerűsítés).
3. **Review-olt alias:** normalizált név + kontextus-szűrő a `reviewed`
   aliasokon, pontosan 1 különböző entitás → `Exact(ReviewedAlias)`.
4. **Egzakt normalizált név:** `core.entity.canonical_name` normalizált
   egyezés + kontextus-szűrő, pontosan 1 találat → `Exact(ExactName)`.
   Több találat → jelöltek (`multiple_candidates`).
5. **Fuzzy jelöltek:** token-átfedés ≥ 0.6 (`FUZZY_THRESHOLD`),
   kontextus-szűrve → kizárólag `Candidates(fuzzy_candidate)`, soha Exact.
   A pontszám csak a review-sor tájékoztató rendezésére való.
6. **Semmi:** `NotFound(no_exact_match)`.

A `Candidates` és a `NotFound` **mindig** pending `identity.review_queue`
sort nyit (upsert: kulcsonként egy pending, a meglévő sor frissül, nem
duplikálódik), és visszaadja annak `queue_id`-ját.

**Kontextus-szűrő minden útvonalon:** ha az ország/nem/korosztály/csapatág
mindkét oldalon jelen van és eltér, a jelölt nem ajánlható. Ez zárja a
v1 Newcastle→ManU-osztályú hibákat.

**DB-hiba:** `IdentityError::Db` — a hívó (tick) fail-closed módon folytatja:
naplóz (`tracing::error`) és `false` identity-flaget ad, a meccs withheld
marad. Bizonytalan állapotban soha nem enged át.

## 2. Review menete (CLI)

A review-felület kizárólag CLI (`footballctl identity`); API/kliens felület
az S8 hatóköre. A CLI döntője minden döntésnél `cli`.

### `footballctl identity review [--status pending|approved|rejected]`

Kilistázza a review-sorokat (alapértelmezés: mind). Üres lista esetén:

```text
id | forrás | external ID | név | ok | jelöltek száma | létrehozva
(nincs felülvizsgálati sor)
```

Példa sorral (szintetikus példa):

```text
id | forrás | external ID | név | ok | jelöltek száma | létrehozva
019a2c34-5678-7abc-8def-0123456789ab | demo-synthetic | DEMO-FC | Demó FC | multiple_candidates | 2 | 2026-11-13 10:15:30 UTC
```

Érvénytelen `--status` értéknél magyar hiba:
`Érvénytelen státusz: <érték> (lehetséges: pending, approved, rejected)`.

### `footballctl identity approve <queue-id> --entity <uuid> [--note <szöveg>]`

Csak `pending` soron működik. Egy tranzakcióban: aktív `manual_review`
`source_entity_ref` írása + a sor `approved`-ra zárása + append-only audit.
Sikeres kimenet:

```text
jóváhagyva: 019a2c34-5678-7abc-8def-0123456789ab → 019a2c34-1111-7abc-8def-0123456789ab
```

Ellenőrzések és hibaüzenetek (magyar, cselekvő):

- nem létező sor → `A kért review-sor nem található.`
- már eldöntött sor → `A felülvizsgálati sor már el lett döntve.`
- nem létező entitás → `Érvénytelen entitásazonosító.`
- kontextus-ütközés → `A kontextus nem egyezik a jelölt entitás kontextusával.`
- forrás nélküli (NULL source) sor → `A review-sorhoz nem tartozik forrás,
  kézi döntés nem lehetséges.` (fail-closed: kézi ref kulcsához kell a forrás)

A következő resolve a kézi ref alapján `Exact(ManualReview)`-t ad, a gate
újranyit. Dokumentált korlát: az approve `source_entity_ref`-et ír, nem
aliast — az alias-import külön szeletben érkezik.

### `footballctl identity reject <queue-id> --note <szöveg>`

Csak `pending` soron, a jegyzet kötelező (üres → `A jegyzet nem lehet
üres.`). A döntés **végleges**: a `rejected` sor megmarad, újra nem
dönthető (`A felülvizsgálati sor már el lett döntve.`). Sikeres kimenet:

```text
elutasítva: 019a2c34-5678-7abc-8def-0123456789ab
```

Ugyanarra a párra új evidence/új jelölt új pending sort nyithat; a linker
a rejected sort nem éleszti újra.

### `footballctl identity list-aliases [--entity <uuid>]`

Aliasok státusszal; üres lista esetén:

```text
entitás | alias | nyelv | ország | státusz
(nincs alias)
```

## 3. Append-only audit és visszavonás

- Minden döntés az `ops.audit_event` táblába kerül: actor (`cli`),
  action (`identity_approve` / `identity_reject`), detail (queue_id,
  entity_id, note). A napló **append-only**: törlés és módosítás nélkül.
- A `review_queue` sor státusza változik (pending → approved/rejected),
  a döntési napló megmarad.
- **Visszavonás csak új döntéssel:** rossz jóváhagyás esetén a kézi ref
  státuszának tudatos revíziója és új review-döntés szükséges (superseded
  elv); a régi audit-esemény nem írható át. Automatikus fuzzy összevonás
  és csendes korrekció tilos.

## 4. Fail-closed garanciák (mindegyik tesztelt)

1. **DB-hiba → withheld:** a resolve hibája esetén a tick naplóz és
   `false` flaget ad (EXACT_IDENTITY blocker); soha nem enged át
   bizonytalan állapotban.
2. **Kontextus-ütközés → nem jelölt** minden útvonalon (alias, név, fuzzy).
3. **Több jelölt → soha nem választ automatikusan**; approve csak explicit
   `--entity`-vel történik.
4. **Weak rung nem ad Exact-et** (S2b-1-ben jelöltet sem); Exact-hez
   strong/medium + minden elemében aktív lánc kell.
5. **Rejected végleges**; új jelölt új sort nyit.
6. **Nincs néma automatizmus:** fuzzy küszöb felett sem Exact, csak
   jelölt + pending review-sor.

Karantén-kapcsolat: a linker NotFound-ja NEM karantén (a payload érvényes,
az identitás bizonytalan) — a meccs withheld, a nyers/observation megmarad,
review után a gate újranyit. A parse-karantén (S2a) érintetlen.

## 5. Ismert korlátok

- **Fixture re-seed:** a `Canonicalizer::materialize` minden tickben
  újraírja a `fixture_seed` ref-eket (`ON CONFLICT DO UPDATE
  status='active'`), ezért a fixture pipeline-ban a withheld → review →
  approve ciklus resolve-szinten bizonyított (`resolve_team_identity`
  közvetlen hívásával). Tick-szintű withheld állapot nem-fixture
  forrásoknál jelentkezik — az S3 fázisban záródik.
- **Fuzzy skála:** a jelölt-képzés az `all_team_entities` teljes listán fut
  (S2b-1 szintetikus skála); S3-ban szűkítendő (ország/verseny-előszűrő).
- **Valódi import nincs:** crosswalk-import (reep strong rung, ESPN-aliasok,
  v1 alias-táblák) és alias-import külön szelet (S3); a dev DB identity
  táblái üresek, a demo fixture_seed ref-ek érintetlenek.

## 6. Visszaállítási út

- **0005 migráció (additív):** csak az `identity` séma három új tábláját
  adja hozzá; a dev demoadat érintetlen. Eldobás fordított sorrendben
  (a hozzátartozó indexek a táblákkal együtt dobódnak):

  ```sql
  DROP TABLE identity.review_queue;
  DROP TABLE identity.alias;
  DROP TABLE identity.crosswalk;
  ```

  TesztDB-n a teljes sémaújraépítés a szokásos út: `./scripts/db-reset.sh`.
  Dev DB-n a drop csak kifejezett felhasználói engedéllyel futtatandó.
- **Tick:** a linker-csere visszaállítható az S2a `identity_exact=true`
  hardcode-ra git reverttel (a gate szabályai változatlanok, a demo
  pipeline érintetlen).
- **Kód:** a szelet a `feat/s2b1-identity-layer` ágon él; elvetés
  `git restore` csak kifejezett engedéllyel.
