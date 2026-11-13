# S2b-1 — Fail-closed identitás-réteg: linker, crosswalk, alias, review-sor (design)

**Dátum:** 2026-11-13
**Státusz:** design jóváhagyva szekciónként a felhasználó által; spec review-kapu előtt
**Bemenetek:**
- [S1 forráskatalógus](../../research/source-catalog.md) — jóváhagyott bizalmi mátrix
  (Identitás-híd sor: reep strong rung egzakt = magas; minden más → review-sor;
  fuzzy összevonás tilos), 16. séma-rés tétel (external_id_map + team_aliases + liga-alias);
- [S1 záró review döntések](../../decisions/2026-11-12-s1-review-dontesek.md);
- [S1 kutatási spec](2026-11-11-real-data-sources-research-design.md) §1.1–1.2
  (fail-closed identitás: csak explicit crosswalk/alias vagy egzakt egyezés);
- [Rendszertervezési spec](2026-11-09-football-app-v2-design.md) §7.10.1
  (bizonytalan csapatnév soha nem oldódik fel automatikusan fuzzy egyezéssel;
  a fuzzy csak jelöltet adhat);
- [S2a spec](2026-11-12-s2a-ingest-framework-design.md) — a tick live ágának
  `identity_exact=true` hardcode-ja cserélődik a linkerre;
- AGENTS.md (crate-határok, migrációs fegyelem, tesztelés, publikus repó).

## 1. Hatókör

Az S2b két alszeletre bomlik (a felhasználó jóváhagyta):
1. **S2b-1 (ez a spec):** fail-closed identitás-réteg — linker, crosswalk/alias/
   review-sor táblák, CLI review, tick-integráció.
2. **S2b-2:** bizalmi szintes publish gate (a jóváhagyott mátrixszal, backward
   kompatibilis bővítés) — az identitás-rétegre épül, külön ciklus.

### 1.1 Határvonalak (jóváhagyott döntések)

- **Tiszta keret:** a linker mechanizmusát szintetikus golden fixture-ök és
  contract/integrációs tesztek bizonyítják. Valódi crosswalk-import (reep strong
  rung, ESPN-aliasok, v1 alias-táblák) NEM része — külön szeletben (S3
  adapterekkel vagy célzottan) érkezik. A reep CSV >5 MB és nem mért
  (streaming-döntés kell), a v1 aliasok auditot igényelnek.
- **Review-felület: csak CLI** (`footballctl identity`). API-endpoint és kliens
  az S8 hatóköre (YAGNI).
- **Nem változik:** a gate szabályai és a publish-állapotok (S2b-2), a meglévő
  fixture pipeline, a shadow ág, a kliens.

### 1.2 Nem célok (YAGNI)

- Nincs fuzzy automatikus összevonás (AGENTS + spec tiltja; a v1
  Atlético/Real és Newcastle/ManU tanulsága).
- Nincs valódi adatimport ebben a szeletben.
- Nincs API/kliens review-felület (S8).
- Nincs liga-alias tábla most: a 16. séma-rés tétel liga-alias része az S3
  adapterekkel jön, amikor a liga-azonosítás ténylegesen kell (a crosswalk
  tábla entity_kind-ot nem kap — a csapat/verseny crosswalkot a
  source_entity_ref entity_kind-ja fedi; ha az S3-ban mérhető rés lesz,
  külön bővítés).

## 2. Architektúra

```
worker tick (live ág)                footballctl identity review/approve/reject
        │ resolve(...)                        │
        ▼                                     ▼
   identity crate (ÚJ)  ──▶ storage::repositories::identity (0005 táblák)
        │                    + meglévő source_entity_ref
        ▼
   domain::identity (tiszta szabályok: normalizálás, egyeztetés, jelölt-képzés)
```

### 2.1 Crate-határok

- **Új `identity` crate** (`rust/crates/identity`): linker szolgáltatás —
  `resolve`, `propose_candidates`, review-döntések alkalmazása.
  Függ: `domain`, `storage`. A `worker` és a `footballctl` fogyasztja;
  az `ingest` nem függhet tőle (egyirányú függőség).
  Indoklás az új crate-re: önálló felelősség (identitás-feloldás), két
  fogyasztó (worker + CLI), az ingest forrásfeldolgozás, a domain tiszta —
  a jelenlegi szerkezet nem ad neki otthont.
- **`domain::identity`** (új modul): IO-mentes szabályok — névnormalizálás
  (kisbetű + whitespace, ékezetek megmaradnak), egzakt egyezés,
  kontextus-szűrés (ország/nem/korosztály/csapatág), fuzzy jelölt-pontszám
  (kizárólag jelöltlistához). Típusok: `Resolution`, `IdentityContext`,
  `Candidate`, `ResolveMethod`.
- **`storage::repositories::identity`**: a jelenlegi stub kibővül —
  crosswalk/alias/review_queue CRUD + source_entity_ref lekérdezések.
  Minden SQL itt (AGENTS §5).
- **`worker`**: a tick live ága a linkert hívja (2.3).
- **`footballctl`**: `identity` alparancsok (3.3).

### 2.2 A linker szerződése

```rust
// identity::linker
pub enum Resolution {
    Exact { entity_id: Uuid, method: ResolveMethod },
    Candidates { candidates: Vec<Candidate>, queue_id: Uuid },
    NotFound { queue_id: Uuid },
}

pub enum ResolveMethod {
    ExactExternalId,   // source_entity_ref (exact_external_id / fixture_seed)
    CrosswalkChain,    // strong/medium crosswalk + cél-ref, minden elem aktív
    ReviewedAlias,     // identity.alias status=reviewed, kontextus egyezik
    ExactName,         // normalizált canonical_name, egyértelmű + kontextus
    ManualReview,      // review-döntésből származó ref
}

pub struct IdentityContext {
    pub country_code: Option<String>,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub branch: Option<String>,
}

pub async fn resolve(
    pool: &PgPool,
    source_id: Uuid,
    entity_kind: &str,
    source_entity_id: &str,
    proposed_name: &str,
    ctx: &IdentityContext,
) -> Result<Resolution, IdentityError>;
```

A `Candidates` és `NotFound` minden esetben review_queue bejegyzést hoz létre
(vagy frissíti a meglévő pending sort), és a `queue_id`-t visszaadja.
A resolve DB-hibája `IdentityError::Db` — a hívó (tick) ezt NotFound-ként
kezeli: withheld + review-sor `no_exact_match` okkal (fail-closed, 4. fejezet).

**Terv-jegyzet (2026-11-13, writing-plans):** a `resolve` szignatúrája a
tervben `source_slug: &str` paraméterrel bővül (a crosswalk tábla szöveges
forrás-kulcsot használ, a source_entity_ref pedig uuid-t) — backward
kompatibilis kiegészítés, a viselkedés változatlan.

### 2.3 Tick-integráció (S2a csere)

A `tick.rs` live ágának `verify_and_publish`-ja meccsenként:
1. `resolve` a hazai és vendég csapatra (forrás + external ID + név +
   kontextus az observation adataiból).
2. `Exact` → `home/away_identity_exact = true`; `Candidates`/`NotFound`/
   `Db` → `false` (a review-sor bejegyzést a linker már létrehozta).
3. A `VerificationContext` többi része változatlan; az EXACT_IDENTITY
   blocker withholding-ot ad, amíg a review el nem dől.
4. Shadow ág érintetlen. A demo fixture_seed ref-ek Exact-ként oldódnak fel
   → a 20/20 verified_publish megmarad (regressziós teszt).

## 3. Adatmodell (0005 migráció)

Minden új tábla az `identity` sémában; a `core`/`fact`/`ops` sémák nem
változnak. Egyetlen tranzakciós migráció: `0005_identity_layer.sql`;
sorrend: tesztDB → zöld → devDB; visszaállítási út dokumentálva
(drop sorrend: review_queue, alias, crosswalk).

| Tábla | Kulcs / fontos mezők | Felelősség |
|---|---|---|
| `identity.crosswalk` | id uuid PK; `left_source` text, `left_external_id` text, `right_source` text, `right_external_id` text, `rung` text CHECK (strong/medium/weak), `method` text CHECK (exact_external_id/manual_review/import), `status` text CHECK (active/superseded/rejected), `evidence` text, `created_at`; UNIQUE (left_source, left_external_id, right_source, right_external_id) | forrás-független külső ID kötések; a reep strong rung fogadója; weak rung csak jelölt |
| `identity.alias` | id uuid PK; `entity_id` uuid FK core.entity, `alias` text, `lang` text, `country_code`/`gender`/`age_group`/`branch` text NULL, `status` text CHECK (pending/reviewed/rejected), `method` text CHECK (manual_review/import/fixture_seed), `evidence_observation_id` uuid NULL FK, `reviewed_by` text NULL, `reviewed_at` timestamptz NULL, `created_at`; UNIQUE (entity_id, alias, lang, country_code, gender, age_group, branch) NULLS NOT DISTINCT | kontextusos aliasok (HU↔EN, női/utánpótlás); csak reviewed alias ad Exact-et |
| `identity.review_queue` | id uuid PK; `source_id` uuid NULL FK ops.source, `entity_kind` text, `source_entity_id` text, `candidate_entity_ids` jsonb, `reason` text CHECK (no_exact_match/multiple_candidates/alias_conflict/fuzzy_candidate/name_mismatch), `proposed_name` text, `context` jsonb, `status` text CHECK (pending/approved/rejected), `created_at`, `decided_by` text NULL, `decided_at` timestamptz NULL, `decision_note` text NULL; részleges UNIQUE (source_id, entity_kind, source_entity_id, reason) WHERE status='pending' | bizonytalan párok első osztályú sora; egy pending kulcsonként; döntés append-only auditba |

A meglévő `identity.source_entity_ref` változatlan (forrásonkénti leképezés:
exact_external_id / manual_review / fixture_seed, confidence, status, evidence).

### 3.1 Feloldási sorrend (fail-closed)

1. **Egzakt külső ID:** aktív `source_entity_ref` (source, kind, external_id)
   → `Exact`, a ref method-ja szerint: `exact_external_id`/`fixture_seed` →
   `ExactExternalId`, `manual_review` → `ManualReview`.
2. **Crosswalk-lánc:** external ID → aktív strong/medium `crosswalk` →
   cél-forrás aktív ref-je → `Exact(CrosswalkChain)`, csak ha a lánc minden
   eleme egzakt és aktív; weak rung → jelölt.
3. **Review-olt alias:** normalizált név + kontextus egyezés `reviewed`
   aliasra → `Exact(ReviewedAlias)`.
4. **Egzakt normalizált név:** `core.entity.canonical_name` normalizált
   egyezés + kontextus, csak ha egyértelmű (1 jelölt) → `Exact(ExactName)`;
   több jelölt → `Candidates(multiple_candidates)`.
5. **Fuzzy hasonlóság:** token-átfedés/pontszám → kizárólag
   `Candidates(fuzzy_candidate)`, soha Exact; a pontszám csak a review-sor
   rendezésére való.
6. **Semmi:** `NotFound(no_exact_match)` → review-sor + gate withheld.

**Kontextus-szűrő kötelező minden úton:** ha az ország/nem/korosztály/
csapatág mindkét oldalon jelen van és eltér, a jelölt nem ajánlható
(alias- és fuzzy-útvonalon is). Ez zárja a v1 Newcastle→ManU-osztályú hibákat.

### 3.2 Review-életciklus

```
linker: Candidates/NotFound → review_queue (pending, reason, jelöltek, kontextus)
ember: footballctl identity review → lista (prioritás szerint rendezve)
ember: identity approve <id> --entity <uuid> → source_entity_ref/alias
       (manual_review) + queue approved + ops.audit_event
ember: identity reject <id> --note "..." → queue rejected + audit (végleges)
linker: következő ticknél a review-olt döntés Exact-et ad → gate újranyit
```

- Append-only döntési napló: `ops.audit_event` (actor, action=identity_approve/
  identity_reject, detail: queue_id, entity, ok, note). A queue sor státusza
  változik, a napló megmarad; visszavonás csak új review-döntéssel (superseded).
- Egy pending sor kulcsonként (részleges unique); a linker a meglévő pending
  sort frissíti (jelöltlista bővülhet), nem gyárt duplikátumot.
- Rejected döntés végleges: ugyanarra a párra nincs automatikus újraajánlás;
  új evidence/új jelölt új pending sort nyithat.

### 3.3 CLI (`footballctl identity`)

- `review [--status pending]` — lista: id, forrás, external ID, név, ok,
  jelöltek, létrehozva; rendezés kickoff-közelség + verseny-prioritás szerint.
- `approve <queue-id> --entity <uuid> [--note]` — csak pending soron; az
  entity létezik (FK), kontextus-ütközésnél hiba.
- `reject <queue-id> --note <szöveg>` — csak pending soron, note kötelező.
- `list-aliases [--entity <uuid>]` — aliasok státusszal.
- Magyar, cselekvő hibaüzenetek; hiányzó env/DB → értelmes hiba, nem pánik.

## 4. Hibakezelés és fail-closed garanciák

**Típusos hibák** (identity crate, thiserror): `IdentityError::{Db, NotFound,
AmbiguousDecision, ContextConflict, AlreadyDecided, InvalidEntity}`. A CLI
magyar üzenetet ad rájuk.

**Garanciák (mindegyik tesztelt):**
1. **DB-hiba → withheld:** a resolve lekérdezés hibája esetén a tick
   NotFound-ként kezel (EXACT_IDENTITY blocker), review-sor no_exact_match
   okkal + hiba-jelzés. Soha nem enged át bizonytalan állapotban.
2. **Kontextus-ütközés → nem jelölt** (minden útvonalon).
3. **Több jelölt → soha nem választ automatikusan**; approve csak explicit
   `--entity`-vel.
4. **Weak rung → csak jelölt**; Exact-hez strong/medium + aktív lánc kell.
5. **Rejected végleges**; új jelölt új sort nyit.
6. **Nincs néma automatizmus:** fuzzy küszöb felett sem Exact.

**Karantén-kapcsolat:** a linker NotFound-ja NEM karantén (a payload érvényes,
az identitás bizonytalan) — a meccs withheld, nyers/observation megmarad,
review után a gate újranyit. A parse-karantén (S2a) érintetlen.

**Újraindítás-biztonság:** minden állapot DB-ben él; a linker állapotmentes;
a tick idempotens (pending unique).

## 5. Tesztelés és ellenőrzés

1. **Domain unit tesztek** (`domain::identity`): normalizálás (ékezet,
   whitespace), egzakt egyezés, kontextus-szűrő minden ágon, fuzzy csak
   jelölt, küszöb nem ad Exact-et.
2. **Linker integrációs tesztek** (tesztDB, szintetikus fixture-ök): a 6
   feloldási út (exact ID; crosswalk strong/medium/weak; reviewed alias;
   egzakt név egyértelmű/több; fuzzy → review; NotFound → review); DB-hiba →
   withheld; rejected véglegesség; pending-dedup.
3. **Review-életciklus tesztek:** approve → manual_review ref + audit +
   következő resolve Exact; reject → végleges + audit; AlreadyDecided/
   ContextConflict/InvalidEntity; append-only napló.
4. **Tick E2E:** live ág linkerrel — exact fixture → verified; bizonytalan
   identitás → withheld + review-sor; approve után újraértékelés → verified.
   Shadow ág érintetlen (regresszió).
5. **CLI tesztek:** review/approve/reject/list-aliases kimenetek + magyar
   hibaüzenetek (visszaolvasás).
6. **Regresszió:** demo pipeline 20/20 verified_publish; teljes workspace
   suite zöld.

**Kötelező ellenőrzések a szelet végén:** `./scripts/check.sh`,
`db-reset + db-check` (tesztDB 0 üzleti sor), magyar visszaolvasás +
glyph-scan, `footballctl identity review` kézi bizonyíték a checkpointban.

## 6. Sikerkritériumok (Definition of Done)

- Minden teszt futott és zöld; parancsok + kimenetek a checkpointban.
- Élő hálózati kérés 0; nyers payload a repóban 0 (szintetikus fixture-ök).
- Demo 20/20 verified_publish; a gate szabályai változatlanok (S2b-2).
- 0005 migráció tesztDB → zöld → devDB; visszaállítási út dokumentálva.
- Operations doc: `docs/operations/identity.md` (review menete, CLI,
  visszaállítás).
- Spec + terv + checkpoint magyarul, visszaolvasva.

## 7. Kockázatok és mitigáció

| Kockázat | Mitigáció |
|---|---|
| A linker valódi adat nélkül készül, az S3 importoknál derül ki illesztési hiba | a contract-tesztek a katalógusban mért crosswalk-alakokat kódolják (reep rung, ESPN alias, Tipplap HU↔EN); az S3 első importja a legismertebb forrás |
| A fuzzy jelölt-képzés túl sok review-t generál | a pontszám-küszöb (domain-konstans, alapértelmezett 0.6 token-átfedés) és a kontextus-szűrő csökkenti a zajt; a review-sor prioritás-rendezett; a küszöb később adatvezéreltté tehető |
| A review-sor eltömődik (nincs ember) | a CLI prioritás-rendezett; a withheld meccsek láthatók az Üzemeltetésben (S8); a queue növekedése doctor-mérőszám |
| Új crate + 3 tábla komplexitás | minden egység egy felelősségű, külön tesztelhető; a domain-pure szabályok IO nélkül futnak |

## 8. Visszaállítási út

- Kód: feature branch (`feat/s2b1-identity-layer`), commit csak felhasználói
  kérésre; elvetés `git restore` csak kifejezett engedéllyel.
- DB: a 0005 tranzakciós; drop sorrend: review_queue, alias, crosswalk
  (tesztDB-n gyakorolva); a dev demoadata érintetlen (a migráció csak identity
  táblákat ad hozzá).
- Tick: a linker-csere visszaállítható az S2a hardcode-ra git reverttel
  (a gate változatlan, a demo pipeline érintetlen).

## 9. Következő lépés

A spec felhasználói review-ja után a writing-plans skill következik
(megvalósítási terv: `docs/superpowers/plans/2026-11-13-s2b1-identity-layer.md`).
Implementáció csak a terv jóváhagyása után, szeletenkénti stop-pontokkal.
Az S2b-2 (bizalmi gate) külön brainstorming ciklusban indul ez után.
