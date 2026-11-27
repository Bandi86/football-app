# DB-lekérdezések és séma-térkép (football_v2_dev)

Kapcsolódás (a repo gyökeréből, a .env-ből; a jelszó ne kerüljön naplóba/kimenetbe):

```bash
cd ~/Documents/Code/football-app-v2
set -a && source .env && set +a
psql "$DATABASE_URL" -P pager=off -c "…"
```

Dev = `football_v2_dev`, teszt = `football_v2_test`. A v1 `football_app` DB csak olvasható
referencia (crosswalk-seed, v1-import forrás).

## Oszlopnevek, amelyek eltérnek az intuíciótól

- `core.match`: **kickoff_at** (nem kickoff_ts); home_team_id/away_team_id; competition_id; status.
- Az ország FK: `core.team.country_id` / `core.competition.country_id` → `core.country.id`;
  az ISO a **core.country.code** (nem iso_code), a név a `core.country.name`.
- `core.team`-nek **nincs created_at** oszlopa — a létrehozási idő a `core.entity.created_at`-on
  él (azonos id).
- `ops.publish_gate`: **state** (nem status), publish_date, blockers jsonb; állapotok:
  verified_publish / provisional_publish / withheld / quarantined.
- `ops.fetch_ledger`: **outcome** (nem ok), attempted_at, http_status; outcome ∈ success /
  http_error / timeout / oversize / robots_blocked / quota_blocked / breaker_open /
  transport_error / parse_error.
- `ops.source`: **code** (nem slug), display_name. Az `ops.source_policy` viszont source_slug.
- `identity.review_queue`: `context` jsonb (country_code / branch / gender / age_group kulcsok);
  reason ∈ no_exact_match / multiple_candidates / alias_conflict / fuzzy_candidate / name_mismatch;
  candidate_entity_ids jsonb; unique pending index (source_id, entity_kind, source_entity_id, reason).
- `stage.observation`: entity_kind (team/match), aspect (team_identity/match_core),
  normalized_payload jsonb, source_record_key.
- `raw.capture`: minden fetch/import új sort hoz létre; `(source_id, request_key)` stabil
  kérésidentitás, nem egyedi capture-kulcs. A legfrissebb capture determinisztikus
  rendezése: `requested_at DESC, id DESC`. `raw.artifact`: `content_sha256` egyedi,
  ezért az azonos tartalom deduplikálódik; storage_key, compression, byte_size.
- Sémák: raw, stage, core, identity, fact, ops (+ public._sqlx_migrations).

## Bevált diagnosztikai lekérdezések (read-only)

```sql
-- Backfill/köteg állapot
SELECT status, count(*) FROM ops.job WHERE domain='backfill' GROUP BY 1;

-- Mai napi jobok + hibák
SELECT domain, status, count(*) FROM ops.job
 WHERE domain<>'backfill' AND created_at >= CURRENT_DATE GROUP BY 1,2;
SELECT id, source_slug, domain, time_window, attempts, left(last_error,200)
  FROM ops.job WHERE status='failed';

-- Pending review-k forrás/kind/ok szerint
SELECT s.code, q.entity_kind, q.reason, count(*)
  FROM identity.review_queue q JOIN ops.source s ON s.id=q.source_id
 WHERE q.status='pending' GROUP BY 1,2,3 ORDER BY 4 DESC;
-- ország-kontextus (jsonb kulcs: country_code)
SELECT context->>'country_code' AS orszag, count(*)
  FROM identity.review_queue WHERE status='pending' GROUP BY 1 ORDER BY 2 DESC;

-- Mai gate-állapotok
SELECT state, count(*) FROM ops.publish_gate WHERE publish_date = CURRENT_DATE GROUP BY 1;

-- Budgetek / breaker
SELECT source_slug, daily_budget, min_delay_ms, enabled FROM ops.source_policy;
SELECT source_slug, state, failure_count FROM ops.breaker_state;

-- Rossz országú entitások kár-felmérése (mapping-hiba után)
SELECT c.id, c.name, k.code FROM core.competition c
  LEFT JOIN core.country k ON k.id=c.country_id WHERE c.name ILIKE '%<gyanús név>%';
SELECT t.id, t.name, k.code FROM core.team t
  LEFT JOIN core.country k ON k.id=t.country_id WHERE t.name ILIKE '%<gyanús név>%';
```

**Coverage-összesítésnél COUNT(DISTINCT …) kell**: a `LEFT JOIN fact.odds_snapshot` megszorozza
a sorokat (több snapshot/meccs), és minden count-ot felfúj — a meccs/eredmény szám is rossz lesz.
Helyesen:

```sql
SELECT (SELECT count(*) FROM core.match) AS meccs,
       (SELECT count(*) FROM core.match_result) AS eredmeny,
       (SELECT count(DISTINCT match_id) FROM fact.odds_snapshot) AS odds_meccs;
```

## Duplikátum-napok keresése (team-merge után, dedupe-hoz)

```sql
SELECT DISTINCT kickoff_at::date FROM core.match
 GROUP BY LEAST(home_team_id,away_team_id), GREATEST(home_team_id,away_team_id), kickoff_at
HAVING count(*)>1 ORDER BY 1;
```

A LEAST/GREATEST kulcs a tükrözött (fordított home/away) párokat is csoportosítja. Minden napra:
`footballctl dedupe matches --date <nap> --apply` (dry-run alapból). A végén teljes DB-s ellenőrzés:

```sql
SELECT count(*) FROM (
  SELECT 1 FROM core.match
   GROUP BY LEAST(home_team_id,away_team_id), GREATEST(home_team_id,away_team_id), kickoff_at
  HAVING count(*)>1) x;
```

## Tudatos revízió minta (auditált adatjavítás, NEM migráció)

```sql
BEGIN;
UPDATE core.competition SET country_id=(SELECT id FROM core.country WHERE code='<új>')
 WHERE id='<entitás>' AND country_id=(SELECT id FROM core.country WHERE code='<régi>');
INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
VALUES (gen_random_uuid(), 'hermes-agent', '<action_név>', 'competition', '<entitás>',
        '{"old_country":"<régi>","new_country":"<új>","reason":"…","matches_at_revision":0}');
COMMIT;
```

Az `ops.audit_event` oszlopai: id, actor, action, subject_kind, subject_id, detail jsonb, created_at.
A merge/dedupe eszközök saját archívumot írnak (`ops.entity_merge_archive`, `ops.dedupe_archive` —
teljes előtte-állapot JSONB, kézi visszajátszás a visszaállítási út).

## Nyers artifact inspect (parser-debug recept)

A daemon objektumtára: `~/Library/Application Support/FootballAppV2/agent/data/raw/<storage_key>`.

```sql
SELECT c.request_key, c.received_at, a.storage_key, a.compression, a.byte_size
  FROM raw.capture c JOIN raw.artifact a ON a.id=c.artifact_id
  JOIN ops.source s ON s.id=c.source_id
 WHERE s.code='soccerway' ORDER BY c.received_at DESC LIMIT 5;
```

```bash
zstd -dc "<raw_dir>/<storage_key>" > scratch/render.html
```

- A payload **HTML**, akkor is ha a storage_key `.json.zst` — ne JSON-ként parse-old.
- Soccerway csoport-fejléc: `div[data-testid=wcl-headerLeague]` → `span.headerLeague__flag`
  `title` attribútum (ország) + `a.headerLeague__title` (slug + cím). A parser ebből a flag-title-ből
  képez országot — ha a megfigyelés országa rossz, előbb ezt a title-t ellenőrizd a nyers renderben.
- A megfigyelés visszakeresése: `stage.observation WHERE normalized_payload::text ILIKE '%<csapatnév vagy forrás-id>%'`
  (a match_core payload tartalmazza a competition country_code-ot is).

## Duplikált csapat-entitások (fold-csoport)

A v1 import és a footballdata ugyanazt a klubot két entitásként hozhatja létre
(kanonikus név vs csapatnév, ékezet, török betűk). A csoportosítás kulcsa a
**foldolt név + ország + nem + korosztály + ág**; a „Res.”/„II”/U19 csapatok
fold-névben eltérnek, ezért külön entitások maradnak.

```bash
set -a && source .env && set +a
python3 ~/.hermes/skills/football-data/football-app-v2/scripts/team_dup_groups.py          # dry-run
python3 ~/.hermes/skills/football-data/football-app-v2/scripts/team_dup_groups.py --apply  # merge
```

A script a `footballctl identity merge-team --from <duplikátum> --to <túlélő>
--note … --apply` parancsot hívja; a túlélő a legtöbb meccses entitás. Merge
után kötelező a `dedupe fixtures --apply` + `reverify` + `integrity`.

Nyers ellenőrzéshez a csoportosítás alapja:

```sql
SELECT t.name, coalesce(k.code,'') AS orszag, t.gender, t.age_group, t.branch,
       (SELECT count(*) FROM core.match m
         WHERE m.home_team_id=t.id OR m.away_team_id=t.id) AS meccs
  FROM core.team t LEFT JOIN core.country k ON k.id=t.country_id
 ORDER BY lower(t.name), orszag;
```

Az ékezet-független kulcsot alkalmazásoldalon kell képezni (`fold_name` a
Rustban, `unicodedata` a scriptben) — a Postgres `lower()` az ékezetet és a
török `İ`-t nem foldolja, a `unaccent` extension pedig nem garantált.

## Több utasításos psql-blokk (temp tábla, nem CTE)

A `WITH v AS (…) INSERT …; INSERT … FROM v;` **nem működik**: a CTE csak az
egyetlen utasítás élettartamára él, a második INSERT „relation v does not
exist” hibát ad. Több felhasználáshoz:

```sql
BEGIN;
CREATE TEMP TABLE fix(mid uuid, hf int, af int, src text) ON COMMIT DROP;
INSERT INTO fix VALUES (…);
UPDATE core.match_result r SET home_ft=f.hf, away_ft=f.af
  FROM fix f WHERE r.match_id=f.mid;
INSERT INTO ops.audit_event (id, actor, action, subject_kind, subject_id, detail)
SELECT gen_random_uuid(),'cli','match_result_revision','match',mid,
       jsonb_build_object('new_result',hf||'-'||af,'source',src) FROM fix;
COMMIT;
```

`psql -v ON_ERROR_STOP=1` esetén bármely hiba abortálja a tranzakciót (a
COMMIT nem hajtódik végre) — a hibát a blokk elején, külön próbafuttatással
izoláld, ne a végeredményből következtess.

## Coverage-mérés értelmezése

- A `footballctl coverage --from --to` alapból csak a **top-300 szezont**
  listázza meccsszám szerint; `--limit <nagy szám>` és `--competition <név>`
  opcióval a teljes DB is mérhető. A teljes állapotot mindig a
  `SELECT count(*) FROM core.match` / `core.match_result` adja; a kettőt ne
  keverd a riportban. Réselemzéshez mindig magas limit kell: a top-300 nem
  mutatja, ha egy kisebb liga több évadja hiányzik.
- A folyó szezon és a COVID miatt csonkolt 2019/20 szezonok alacsony
  eredmény-%-a normális, nem hiányzó adat; a 2004 előtti 0% odds szintén
  forrás-korlát (a v1 `match_odds` is csak 2004-től van).
