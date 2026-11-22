-- 0023 — retenció-támogató indexek
--
-- A validációs retenció (scripts/validation-retention.sh) régi ops.validation_run
-- sorokat töröl. A fact.verification.last_run_id FK (nincs ON DELETE CASCADE)
-- index nélkül soronként végigolvasta a verification táblát, ezért a törlés
-- gyakorlatilag nem haladt. Az ops.validation_run.created_at index a cutoff
-- keresést gyorsítja.
--
-- Visszaállítás: a telepítés után `DROP INDEX IF EXISTS ...` (az adatokat
-- nem érinti); a migráció maga nem ír üzleti adatot.

CREATE INDEX IF NOT EXISTS idx_verification_last_run_id
  ON fact.verification (last_run_id);

CREATE INDEX IF NOT EXISTS idx_validation_run_created_at
  ON ops.validation_run (created_at);
