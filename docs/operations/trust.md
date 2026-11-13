# Bizalmi réteg üzemeltetési útmutató (S2b-2)

**Dátum:** 2026-11-13
**Hatókör:** az `ops.source_trust` tábla karbantartása (0006), a demo és a
katalógus-források trust seedje, a `validation_rule` katalógus igazítása,
a provider- és odds-szabály, a 0006 visszaállítási útja.
**Kapcsolódó dokumentumok:**
[S2b-2 spec](../superpowers/specs/2026-11-13-s2b2-trust-gate-design.md),
[S2b-2 terv](../superpowers/plans/2026-11-13-s2b2-trust-gate.md),
`db/migrations/0006_trust_gate.sql`,
[forráskatalógus](../research/source-catalog.md) (bizalmi mátrix),
[identitás útmutató](identity.md).

Minden parancs a repo gyökeréből fut, a környezet betöltése után:

```bash
set -a; . ./.env; set +a
```

## 1. A trust tábla karbantartása

Az `ops.source_trust` kulcsa `(source_id, domain)`; egy forrás domainenként
egy trust osztályt kap: `high`, `medium` vagy `display_only`. A `provider`
oszlop NULL = önálló csoport; azonos provider = egy csoport (lásd 4. pont).
A `created_at` az első beszúráskor rögzül, upsertnél megmarad.

**Hiányzó trust sor nem hiba:** a verification context medium defaultot
alkalmaz (konzervatív). Trust-sor nélküli forrás egymagában provisional.

Karbantartó upsert (a repository `upsert_trust` függvényével azonos
szemantika: ütközéskor az osztály és a provider frissül, a `created_at`
marad):

```sql
INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
SELECT id, 'fixture/kickoff', 'medium', NULL
FROM ops.source
WHERE code = 'demo_day_a'
ON CONFLICT (source_id, domain) DO UPDATE
SET trust_class = EXCLUDED.trust_class,
    provider    = EXCLUDED.provider;
```

Törlés (a forrás medium default-ra esik vissza):

```sql
DELETE FROM ops.source_trust
WHERE source_id = (SELECT id FROM ops.source WHERE code = 'demo_day_a')
  AND domain = 'odds';
```

## 2. Seed — demo források

A demo források `medium` osztályt kapnak NULL providerrel, mindhárom demó
domainre (`fixture/kickoff`, `result`, `odds`). A két demo forrás NULL
providerrel két független csoport → a kétforrású demo meccsek
`verified_publish` állapotúak, az egzakt egyező odds párok `verified`
odds állapotot adnak.

A seed idempotens, és csak a már létező forrásokra szúr be sort. A dev
adatbázis demo forrásai: `fixture_a`, `fixture_b`, `demo_day_a`,
`demo_day_b`, `demo-synthetic`.

```sql
INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
SELECT s.id, d.domain, 'medium', NULL
FROM ops.source s
CROSS JOIN (VALUES ('fixture/kickoff'), ('result'), ('odds')) AS d(domain)
WHERE s.code IN ('fixture_a', 'fixture_b', 'demo_day_a', 'demo_day_b', 'demo-synthetic')
ON CONFLICT (source_id, domain) DO UPDATE
SET trust_class = EXCLUDED.trust_class,
    provider    = EXCLUDED.provider;
```

## 3. Seed — a katalógus igazolt forrásai

A bizalmi mátrix (forráskatalógus, jóváhagyva 2026-11-12) alapján. A
katalógus-forrásokat az S3 adapter-szeletek regisztrálják az `ops.source`
táblába; amíg a forrás nem létezik, a seed nem szúr be sort (a JOIN miatt
biztonságosan újrafuttatható). A provider mindenhol NULL: a mátrixban nincs
közös provider-csoport; az S3 felülvizsgálja, ha valódi közös adatszolgáltató
derül ki.

Osztályok: `footballdata` high fixture/kickoff + result (egymagában is
verified), `thefa` high fixture/kickoff (a results domain Opta-decoy miatt
halasztott, nincs seedelve); `espn`, `fotmob`, `openfoot`, `sky`, `tipplap`
medium. Odds: nincs egyforrásos high — a kétforrásos szabály két medium
oldala `tipplap` + `footballdata` (closing). A display-only domainek
(lineups/stats/logó/meta/Elo) `display_only` osztályt kapnak: nem
gate-tényezők, nem adnak verified-et és nem is rontanak.

```sql
INSERT INTO ops.source_trust (source_id, domain, trust_class, provider)
SELECT s.id, v.domain, v.trust_class, NULL
FROM ops.source s
JOIN (VALUES
    ('footballdata', 'fixture/kickoff', 'high'),
    ('footballdata', 'result',          'high'),
    ('footballdata', 'odds',            'medium'),
    ('thefa',        'fixture/kickoff', 'high'),
    ('espn',         'fixture/kickoff', 'medium'),
    ('espn',         'result',          'medium'),
    ('fotmob',       'fixture/kickoff', 'medium'),
    ('fotmob',       'result',          'medium'),
    ('fotmob',       'lineups',         'display_only'),
    ('fotmob',       'stats',           'display_only'),
    ('openfoot',     'fixture/kickoff', 'medium'),
    ('openfoot',     'result',          'medium'),
    ('sky',          'fixture/kickoff', 'medium'),
    ('sky',          'result',          'medium'),
    ('tipplap',      'fixture/kickoff', 'medium'),
    ('tipplap',      'odds',            'medium')
) AS v(code, domain, trust_class) ON v.code = s.code
ON CONFLICT (source_id, domain) DO UPDATE
SET trust_class = EXCLUDED.trust_class,
    provider    = EXCLUDED.provider;
```

Új display-only domain (pl. `logo`, `meta`, `elo`) regisztrálásakor ugyanígy
`display_only` sor kerül a táblába; a gate ezeket figyelmen kívül hagyja.

**A `validation_rule` katalógus igazítása (forward):** a migrációs lánc
(0001) még `SINGLE_SOURCE` szabálysort seedel, és a 0006 ezt nem nevezi át.
A production kód nem olvassa a `validation_rule` táblát (a gate a reportból
dolgozik, nem ebből a táblából), ezért ez nem funkcionális hiba — de a
migrációkból felépített friss DB katalógusa elavult marad. Az alábbi
idempotens SQL a katalógust a TRUST_LEVEL szabályhoz igazítja (AGENTS §6
miatt üzemeltetési lépés, nem új migráció):

```sql
UPDATE ops.validation_rule SET rule_key='TRUST_LEVEL', description='Bizalmi szint (egy független közepes forrás → provisional)' WHERE rule_key='SINGLE_SOURCE';
```

Az UPDATE újrafuttatása ártalmatlan: a második futásnál már nincs
`SINGLE_SOURCE` sor, így 0 sort érint. A 7. pont rollbackje esetén a sort
vissza kell nevezni `SINGLE_SOURCE`-ra.

## 4. Provider szabály

- **NULL provider = önálló csoport.** Két NULL providerű medium forrás két
  független csoport → nincs TRUST_LEVEL warning.
- **Azonos provider = 1 csoport** (a katalógus „3 felület = 1 forrás”
  szabálya): ugyanazon adatszolgáltató több felülete nem számít független
  megerősítésnek.
- A TRUST_LEVEL szabály domainenként (`fixture/kickoff`, `result`) értékeli
  a hozzájáruló forrásokat: bármely high → nincs lelet; egyébként a medium
  források független provider-csoportjai: ≥2 csoport → nincs lelet,
  1 csoport → warning („Egy független forrás (közepes bizalom) —
  provisional.”). A `display_only` hozzájárulás nem számít az anyagi
  domainekbe.
- Az odds-kiértékelés ugyanezt a csoportosítást használja (5. pont).

## 5. Odds szabály

Az `evaluate_odds_trust` (verification crate) mezőszintű, NEM Finding; a
meccs publish állapotát nem érinti. Eredménye a `fact.odds_snapshot`
`trust_state` oszlopába íródik (a meccs összes odds sorára ugyanaz).

- **Verified:** ≥2 független provider-csoport, és a csoportok képviselőinek
  mind a 7 mezője (home/draw/away/btts_yes/btts_no/over25/under25) egyezik
  ±tolerance-on belül.
- **Provisional:** 1 csoport, vagy ≥2 csoport eltérő értékekkel.
- **NoOdds:** nincs odds sor (nincs frissítés).
- A csoport képviselője a csoport első forrása `source_code` szerint
  rendezve (determinisztikus); a csoporton belüli eltérő értékek nem
  blokkolnak.
- **Tolerancia:** paraméter, default `ODDS_TOLERANCE_DEFAULT = 0.0`
  (egzakt egyezés). Az egyezés mikro-epszilont használ:
  `(a-b).abs() <= tolerance + 1e-9` — a két tizedes odds-értékek
  f64-reprezentációs zaját kezeli a decimális határokon (pl.
  abs(1.85−1.86) = 0.010000000000000009). Az 1e-9 az odds-pontosság
  mellett elhanyagolható. Az S3 éles adapterek állítják be a toleranciát
  mért eltérés alapján.
- A `fact.odds_snapshot` PK-ja `(match_id, source_id)`: forrásonként egy
  odds sor meccsenként. A `trust_state` CHECK: verified/provisional,
  default provisional.

## 6. Ellenőrzés

```bash
./scripts/db-check.sh                                   # dev 33, test 33/0/0
cd rust
cargo run -q -p footballctl -- day --date 2026-11-11    # 20 meccs verified_publish
```

A demo elvárt állapota: 20/20 `verified_publish` + `verified` odds
trust_state (a 20 odds pár egzakt egyező, mérve 2026-11-13). A `day`
kimenet meccsenként tartalmazza az `odds_trust_state` mezőt
(verified/provisional; odds nélküli meccsnél null).

## 7. Visszaállítási út (0006 rollback)

A 0006 tranzakciós migráció; a tesztDB-n `./scripts/db-reset.sh` bármikor
visszaállítja a teljes láncot. Dev DB-n a visszaállítás kézi, és a composite
odds sorok deduplikálásával jár (a második forrás sorai elvesznek — előtte
mentés szükséges). A teljes visszaállítás (1–4. lépés) egyetlen
BEGIN…COMMIT tranzakcióban fut: ha bármelyik lépés hibázik, az egész
rollback visszagördül, így nem maradhat olyan állapot, hogy a séma már
visszaállt, de a 0006 migrációs sora még megvan. Lépések:

```sql
BEGIN;

-- 1. Dedup: meccsenként egy odds sor marad (a legnagyobb
--    (captured_at, source_id) értékű sor; a PK miatt forrásonként egy sor).
DELETE FROM fact.odds_snapshot a
USING fact.odds_snapshot b
WHERE a.match_id = b.match_id
  AND (a.captured_at, a.source_id) < (b.captured_at, b.source_id);

-- 2. PK visszaállítása match_id-re.
ALTER TABLE fact.odds_snapshot DROP CONSTRAINT odds_snapshot_pkey;
ALTER TABLE fact.odds_snapshot ADD PRIMARY KEY (match_id);

-- 3. trust_state oszlop és source_trust tábla eldobása.
ALTER TABLE fact.odds_snapshot DROP COLUMN trust_state;
DROP TABLE ops.source_trust;

-- 4. A migrációs láncból a 0006 törlése (sqlx). A tranzakción belül fut:
--    a séma visszaállítása és a migrációs sor törlése együtt commitálódik
--    vagy együtt gördül vissza (atomikus rollback).
DELETE FROM public._sqlx_migrations WHERE version = 6;

COMMIT;
```

Hiba esetén a tranzakció ROLLBACK-kal zárható: ilyenkor egyetlen lépés sem
marad érvényben, a 0006 séma és a migrációs sor is a helyén marad.

Kód-oldalon a TRUST_LEVEL szabály a SINGLE_SOURCE-ra git reverttel
állítható vissza; ekkor az `ops.validation_rule` TRUST_LEVEL sorát is
vissza kell nevezni SINGLE_SOURCE-ra (rule_key, description). A demo
pipeline a rollback után ismét egyforrású odds sorokat használ.
