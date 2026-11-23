# 2026-11-23 — GB alrégió-versenyek egységesítése

## Döntés

A `GB-ENG` és `GB-SCT` alrégió-kódokat a verseny-entitásoknál **egységesítjük GB-ENG-re**, ha a verseny neve és a bizonyíték egyértelműen angol liga. A 0 meccses GB-SCT ghost sorokat az azonos nevű, GB-ENG túlélőbe olvasztjuk, miután az országkódot auditalt revízióval kijavítottuk.

Nem érintjük:
- a valóban skót versenyeket (pl. Scottish Premiership, Scottish Cup);
- a más országok azonos nevű versenyeit (pl. NZ „National League" — 9 meccsel külön entitás marad);
- a `core.country` GB-* kódjait (a forrás-kontextus továbbra is megőrződik).

## Indoklás (mért adatok)

- A `footballctl integrity` szoros definíciója (azonos fold név + azonos ország) mérve **0 duplikátum** — a korábbi CL/BO/CR klasztereket az S95/S97 már feloldotta.
- A GB ghostok a tipplap `("GB", "Scotland")` mapping-hibájából származnak (`rust/crates/ingest/src/tipplap.rs:192-211, 342-363`). Mind a 4 GB-SCT sor **0 meccsel**, legfeljebb 1 szezonnal rendelkezik; a GB-ENG párnak 9 453–18 824 meccse van.
- A `competition_merge.rs:227-232` ország-guard (`--force`-szel sem kerülhető) először ország-revíziót kíván — ez a döntés pontosan ezt az előfeltételt teljesíti.
- A `identity.normalize_country_code` már ma is GB-* → GB alapra normalizál; a verseny-szintű egységesítés ezzel összhangban van, és nem új szabály, hanem a meglévő normalizáció alkalmazása.

## Végrehajtás

1. `core.competition.country_id` revíziója a 4 GB-SCT soron GB-ENG-re, `ops.audit_event`-tel (action: `competition_country_revision`, payload: régi/új ország).
2. `footballctl identity merge-competition --from <GB-SCT entitás> --into <GB-ENG entitás> --note … --apply` — a meglévő, `ops.entity_merge_archive`-ba archivált, tranzakciós út.
3. Ellenőrzés: `integrity`, `reverify`, `coverage-day`, valamint hogy a GB-SCT verseny-sorok száma 0.

## Visszaállítás

- Az ország-revízió az `ops.audit_event.detail` payloadból visszaírható.
- A merge-ek az `ops.entity_merge_archive.payload`-jából visszaállíthatók (a repo `docs/operations/competition-merge.md` szakasza szerint; figyelem: a doc régi „sor megmarad" viselkedést ír, a kód S97 óta törli a beolvadó sort).

## Nyitva marad

- Az AT „Bundesliga" entitás (már `merged_into_id`-vel az „Austrian Bundesliga" felé) elavult `core.competition` sora 0 meccsel, 1 szezonnal: külön, auditalt revízió (a merge tranzakció S97 óta törli a sort, ez a régi merge előtti maradvány).
