# Verseny-identitás és verseny-merge (S81)

A versenyek ugyanazt a névváltozat-problémát hordozzák, mint a csapatok: ugyanaz
a liga több írásmóddal (szponzornév, ékezet, nyelvi változat) több
`core.competition` sort hozott létre, és a duplikátumok a napi feedben
duplikált meccseket termeltek.

## Find-or-create sorrend

Minden forrás-adapter és a `footballctl competitions` javító parancs az
`identity::competition_identity::find_or_create_competition` útvonalon megy:

1. **id_hint** — ha a forrás kanonikus UUID-t ad, azt a merge-láncon át a mai
   (nem merged) versenyre oldja fel;
2. **source_entity_ref** — pontos forrás-hivatkozás;
3. **fold alias** — `identity.fold_name(alias)` egyezés, ország-kontextussal;
4. **entitás kanonikus név** — `identity.fold_name(entity.canonical_name)`
   egyezés (S81 kiegészítés: a régi, alias nélküli sorokat is megtalálja);
5. **természetes kulcs** — fold-olt megjelenített név + ország;
6. csak ezután hoz létre új entitást, és ír aliast (`lang='und'`,
   `status='reviewed'`, `method='import'`) + source-refet.

Az ismeretlen ország (`XX`, `INT`, `UNK`, `ZZ`, üres) semleges wildcard: nem
szűkít országra. Ismert országnál viszont más ország versenyét nem hozza be.

## Merge parancs

```bash
footballctl identity merge-competition   --from <beolvadó-entitás-uuid> --into <túlélő-entitás-uuid>   [--note "indok"] [--apply] [--force]
```

Alapból **dry-run**: csak a hatásokat írja ki. Guardok (dry-run és apply
esetén is): nem önmaga, mindkettő `competition` entitás, egyik sincs már
merge-elve, a fold-olt entitásnév **vagy** a fold-olt versenynév egyezik, az
ország egyezik vagy valamelyik NULL, a túlélő meccsszáma nem kisebb. `--force`
csak a név-guardot lazítja, a többit nem.

Az apply egy tranzakcióban: archív sor (előtte/utána számokkal), ütköző
season-címkék meccseit a túlélő seasonjére mozgatja és az üres season sort
törli, átmozgatja a seasonöket/meccseket/aliasokat/source-refeket, beállítja a
`merged_into_id`-t, majd **aliasként felírja a túlélőre mindkét verseny
megjelenített nevét és mindkét entitás kanonikus nevét** — így a következő
ingest nem hozza létre újra a beolvadó versenyt. Végül audit
(`competition_merge`, actor `cli`). Ismételt hívás idempotens
(`AlreadyMerged`, nulla írás). A beolvadó `core.competition` sora megmarad
(0 meccsel), csak az entitás kap `merged_into_id`-t.

## Ellenőrzés

```bash
cargo test -p football-storage --test competition_identity --test competition_merge -- --test-threads=1
cargo test -p football-ingest --test live_materialize -- --test-threads=1
footballctl integrity   # merge után 0 duplikált verseny-csoport és 0 duplikált fixture
```

Merge után a napi dedupe-pass továbbra is lefut; a verseny-merge nem
helyettesíti a meccs-szintű dedupe-ot.
