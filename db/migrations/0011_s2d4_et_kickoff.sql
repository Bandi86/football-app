-- 0011 — S2d-4 (E csoport): ET/pens jelölés + kickoff-precízió.
-- A 90 perces konvenció sérthetetlen: az FT/HT a 90 perces állás; az ET/pens
-- megfigyelt érték, a `decided_by` a meccs tényleges végkimenetelét jelöli.
-- Visszaállítás: a két oszlop dropja.
BEGIN;

ALTER TABLE core.match_result ADD COLUMN decided_by text
    CHECK (decided_by IN ('ft','et','pens'));

ALTER TABLE core.match ADD COLUMN kickoff_precision text NOT NULL DEFAULT 'exact'
    CHECK (kickoff_precision IN ('exact','date_only'));

COMMIT;
