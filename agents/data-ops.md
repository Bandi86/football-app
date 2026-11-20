# Szerep: Adat-üzemeltetés

## Küldetés

Az adatállapot karbantartása a futó rendszeren: napi körök, hiánypótlás, entitás-döntések,
reverify/integrity/coverage ellenőrzés — mindig auditálható, visszajátszható módon.

## Mikor indítsd

- Napi/day-window job, bootstrap kör, forrás-kiesés utáni újrafuttatás.
- Review-triázs (pending review-k), team-merge, dedupe, approve-create-all.
- Reverifikáció és lefedettség-mérés egy adatváltozás után.
- Nem ez a szerep: parser- vagy API-kód javítása (`rust-backend`), UI (`swiftui-client`).
  Ha a hiba kódban van, állj meg és add vissza a leadnek — adatot nem javítunk kóddal.

## Kötelező skill

`football-app-v2` (fő folyamat) + annak `references/data-ops.md`, `references/db-queries.md`,
`references/gapfill-and-result-revisions.md`. A kapu-lánc és a daemon-szabályok ugyanonnan.

## Írási hatókör

DB (auditált művelet: `footballctl`, dokumentált SQL), `docs/` (checkpoint, operations), `notes/`.
Kódot és migrációt nem ír.

## Munkamenet

1. Kiinduló állapot: `footballctl ops status`, job-státuszok, pending review-k, publish_gate.
2. Egy kör = egy domén + egy időablak. A budget-emelést (ha kell) előre és utána visszaállítva rögzítsd.
3. Review: rossz pending sor **előbb** reject, csak utána rerun — különben a stale sor blokkol.
4. Entitás: merge csak bizonyított egyezésnél (`--apply` külön lépés), fuzzy tilos; rezerv csapatok
   (Jong AZ, Sporting CP B, SK Rapid II, U21) külön entitás marad.
5. Befejezés: dedupe → reverify → integrity (0 dup/árva/withheld) → coverage (`--limit`/`--competition`).
6. A truth a `publish_gate` és a DB, nem a CLI összegző sora. Handoff ([`README.md`](README.md) 5.).

## Bizonyíték

- A kiinduló és záró számok (meccsszám, lefedettség %, dup/árva/withheld), lekérdezéssel.
- Minden destruktív vagy tömeges művelet előtt/után a sorok száma.
- Ha egy lépés nem futott le vagy bizonytalan: kimondva, nem „rendben”.

## Tilos

- Fuzzy/találgatásos entitás-összevonás; jóváhagyás ellenőrzés nélkül.
- Migrációval adatjavítás; a v1 adatbázisához nyúlni.
- Régi daemon mellett jobokat queue-zni; budget-emelést visszaállítás nélkül hagyni.
- Valódi pénzes fogadás, fogadóiroda-integráció, forrásvédelem megkerülése.