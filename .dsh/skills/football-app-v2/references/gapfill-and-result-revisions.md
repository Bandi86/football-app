# Gap-fill és eredményrevíziók — eljárás

Akkor használd, amikor szezonok hiányoznak, vagy a `publish_gate`
`MATCH_RESULT_REQUIRED` / `MATERIAL_SOURCE_CONFLICT` blokkert tartalmaz. A
checkpoint számadatai elavulhatnak: a lépéseket a friss DB állapotából indítsd.

## 1. Réselemzés (read-only)

- A `footballctl coverage` alapból csak a legnagyobb szezonokat listázza;
  résekhez `--limit <nagy szám>` és/vagy `--competition <pontos név>` kell. A
  teljes DB száma mindig `SELECT count(*) FROM core.match` / `core.match_result`.
- Mérd végig a teljes idővonalat, ne csak a korai szezonokat:

```sql
-- v1 (read-only, psql -d football_app)
SELECT s.year_start, count(*) FROM matches m JOIN seasons s ON s.id=m.season_id
 WHERE s.year_start BETWEEN <start> AND <end> GROUP BY 1 ORDER BY 1;
-- v2
SELECT s.label, count(*) FROM core.match m
 JOIN core.season s ON s.id=m.season_id JOIN core.competition c ON c.id=m.competition_id
 WHERE c.name='<verseny>' GROUP BY 1 ORDER BY 1;
```

A v1-nek gyakran akkor is van adata, amikor a footballdata nem fedi a ligát.

## 2. Kétkörös import

1. `footballctl backfill v1-import --tournament <id> --from-year Y --to-year Y
   --target-competition <v2 uuid> --apply`. A cél UUID-t a `core.competition`-ból
   vedd (név + ország egyeztetés); nélküle új verseny-entitás jön létre.
2. `identity approve-create-all`, majd `--allow-unknown-country` az őszinte NULL-oknak.
3. Ugyanaz az import **újra** — az első kör csak az ismert csapatok meccseit
   anyagiasítja. A tartalékcsapatok („II”, U21, Jong, B) külön entitások maradnak.
4. `dedupe fixtures --apply`, majd a kockázatos párok kezelése (lent), reverify,
   integrity, coverage.

## 3. Kockázatos fixture-párok

- A `dedupe fixtures` dry-run `KOCKÁZAT [comp|season|home|away|date] … eredmény
  Some(a)-Some(b) / Some(c)-Some(d)` sora parse-olható. Az első szám a túlélőé.
- **Ne feltételezd, hogy a túlélő forrás a helyes**: a hiba iránya blokkonként
  változhat. Vegyél 2-3 mintát, és webes forrással állapítsd meg, melyik forrás
  téved; azután dönts minden párt külön.
- Tömeges ellenőrzés: indexeld a párokat (`index | comp | season | date | home |
  away | survivor | duplicate`), oszd 4-5 párhuzamos kutatónak 6-10 soronként, és
  kérj soronként ítéletet (`survivor`/`duplicate`/`UNKNOWN`) + két forrás-URL-t.
- Az `index → match_id` térkép alapján egy tranzakcióban javítsd a **túlélő**
  eredményét (ha a másik volt a jogos), és rögzíts `ops.audit_event`
  (`match_result_revision`). Utána `dedupe fixtures --apply`: a pár azonossá válik,
  biztonságosan merge-elődik, nem marad `MATERIAL_SOURCE_CONFLICT`.

## 4. Withheld-gate triage

Előbb bontsd fel, csak azután javíts:

```sql
SELECT g.blockers->0->>'rule_key' AS rule, m.status, count(*)
  FROM ops.publish_gate g JOIN core.match m ON m.id=g.match_id
 WHERE g.state='withheld' GROUP BY 1,2 ORDER BY 3 DESC;
```

- `MATCH_RESULT_REQUIRED` + `awarded`: technikai eredmény, jellemzően 3-0/0-3,
  de a jogcímet és a nyert felet két forrással igazold (szövetségi oldal, RSSSF,
  Wikipedia). A jogcím lehet visszalépés, bojkott, szabálytalan csere,
  jogosulatlan játékos, pályára lépés megtagadása — a pontszám iránya nem
  következtethető a klub nevéből.
- `MATCH_RESULT_REQUIRED` + `finished`: lehet elhalasztott (akkori hír alapján
  `status='postponed'`, eredményt ne találj ki), illetve fordított hazai/vendég:
  ilyenkor a `core.match.home_team_id/away_team_id` értéket is cseréld, és az
  eredményt az így javított orientációban írd be.
- `DUPLICATE_NATURAL_KEY`: a blocker a merge **előtti** pillanatkép. Ha a fixture
  már merge-elve van, `dedupe matches --date` nem talál duplikátumot — futtass
  `reverify --from <nap> --to <nap>` a gate frissítésére, ne a gate táblát írd.
- Az `awarded` meccseknél `home_ht/away_ht = NULL`; félidőt ne találj ki, különben
  `HT_GREATER_THAN_FT` blokkol.

## 5. Eredmények írása és konfliktusok lezárása

Az eredmény nélküli meccsnek gyakran **nincs `core.match_result` sora** — a sima
`UPDATE` 0 sort érint, és a gate továbbra is blokkol. Upsert kell:

```sql
BEGIN;
CREATE TEMP TABLE fix(mid uuid,hf int,af int,src text) ON COMMIT DROP;
INSERT INTO fix VALUES (…);
INSERT INTO core.match_result (match_id,home_ft,away_ft,validity,reason,decided_by,decided_at)
SELECT mid,hf,af,'valid',src,'ft',now() FROM fix
ON CONFLICT (match_id) DO UPDATE SET home_ft=EXCLUDED.home_ft,
  away_ft=EXCLUDED.away_ft,validity='valid',reason=EXCLUDED.reason,
  decided_by='ft',decided_at=now();
INSERT INTO ops.audit_event (id,actor,action,subject_kind,subject_id,detail)
SELECT gen_random_uuid(),'cli','match_result_revision','match',mid,
       jsonb_build_object('new_result',hf||'-'||af,'source',src) FROM fix;
COMMIT;
```

- A CLI-nek környezetben kell a `DATABASE_URL` (`set -a && source .env && set +a`),
  különben `resolve-conflict` „DATABASE_URL hiányzik” hibát ad.
- Minden `fact.conflict` sorhoz válassz győztes observationt a `fact.provenance`-ból:
  `resolve-conflict --match-id … --field … --winner <observation id> --reason …`.
  A `match` mező az egész anyagi összevetést maszkolja.
- A győztes kiválasztása: előbb a javított eredménnyel egyező observation, azután
  a megbízhatóbb forrás. Az azonos kanonikus csapatpárt tartalmazó, de más
  source-ID formátumú (FD slug vs v1 `v1t-…`) sorok álkonfliktusok.
- A dedupe/merge után elavult, provenance nélküli konfliktusok maradhatnak
  (`home_team_id`, `match`, régi `home_ht`). Ha a kanonikus érték web-igazolt,
  `status='resolved_manual'`, `winner_observation_id=NULL`, indoklás + audit.

## 6. Záró kapu

`reverify --from --to` (0 hiba), `integrity` (0 duplikátum/árva/review/withheld/failed
job/breaker), `conflicts` (0 unresolved), `coverage --limit <nagy>` — mindegyiket
futtatott kimenettel igazold, ne a kód szándékával.
