# Validációs előzmények retenciója

A `reverify` és a mindenképp lefutó ellenőrzési lánc minden futtatáskor új
`ops.validation_run` és `ops.validation_finding` sort ír. Ezeknek **nincs
API- vagy UI-fogyasztója**; a kanonikus, megjelenített tények auditja az
`ops.audit_event`-ben, illetve a `fact.provenance`-ban él. A validációs
előzmények így regenerálhatók, ezért nem kell őket korlátlanul tárolni.

## Mért indoklás (2026-11-22)

- `ops.validation_run`: 3 183 493 sor / 479 MB;
- `ops.validation_finding`: 4 829 228 sor / 1 000 MB;
- együtt **1,47 GB**, növekedés ~130 MB/nap;
- a sorok többsége 3 napnál régebbi (11-17: 1,5 M run, 11-19: 2,56 M run).

## Használat

```bash
# mérés, írás nélkül (alapértelmezett)
scripts/validation-retention.sh

# tényleges törlés: 3 napnál régebbi run-ek + cascade findingek
scripts/validation-retention.sh --apply

# más ablak, például 7 nap
scripts/validation-retention.sh --days=7 --apply
```

A script:

1. megszámolja a törlendő run/finding sorokat és a hozzájuk kapcsolódó
   `fact.verification.last_run_id` hivatkozásokat;
2. tranzakcióban `NULL`-ra állítja ezeket a hivatkozásokat (a FK nem cascade),
   törli a régi `validation_run` sorokat (a findingek `ON DELETE CASCADE`-vel
   mennek), és audit-eseményt ír (`validation_retention`, actor `ops-retention`);
3. tranzakción kívül `VACUUM (ANALYZE)` futtat a két táblán.

## Ellenőrzés és visszaállítás

```bash
psql "$DATABASE_URL" -c "SELECT count(*) FROM ops.validation_run;"
psql "$DATABASE_URL" -c "SELECT count(*) FROM ops.validation_finding;"
psql "$DATABASE_URL" -c "SELECT pg_size_pretty(pg_total_relation_size('ops.validation_run')+pg_total_relation_size('ops.validation_finding'));"
psql "$DATABASE_URL" -c "SELECT detail FROM ops.audit_event WHERE action='validation_retention' ORDER BY created_at DESC LIMIT 1;"
```

Visszaállítás: a törölt sorok nem kerülnek archívumba, de a megfelelő napok
`reverify --from … --to …` újra létrehozzák őket a megfigyelésekből. A
`fact.verification.last_run_id` értéke a következő verifikációkor újra kitöltődik.
A script csak a `DATABASE_URL` által megjelölt (dev) adatbázist érinti; a
tesztadatbázist nem.
