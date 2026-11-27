-- 2026-11-27 — külső forrással alátámasztott adatkorrekciók.
--
-- Visszajátszható, idempotens: minden UPDATE csak a konkrét régi értékre
-- illeszkedik, és minden megváltozott sorhoz ops.audit_eventet ír.
-- Források: docs/operations/2026-11-27-data-bug-audit.md (4. szakasz).
-- Alapértelmezés: futtatás tranzakcióban; hiba esetén ROLLBACK.

BEGIN;

-- 1) Paraguay: a 11-25-i Paraguayan Primera sorok elavultak. A három
--    fixture az ESPN szerint 11-27-re / 11-28-ra lett átütemezve, és a
--    megfelelő új sorok már megvannak Division Profesional: Clausura alatt.
WITH stale(id) AS (
    VALUES
        ('01a1fc05-2711-7451-9373-beac66fba917'::uuid),
        ('01a21bec-0c82-7128-9aeb-9f7b86db8f32'::uuid),
        ('01a1fc05-270f-737a-b4fd-1572bc47d002'::uuid),
        ('01a21bec-0c7f-7002-90b6-9e2debe3ddc4'::uuid),
        ('01a1fc05-2712-72d4-969a-0b74365ca212'::uuid),
        ('01a21bec-0c85-762a-bd2c-b9a2017d962e'::uuid)
), upd AS (
    UPDATE core.match m
       SET status = 'canceled', updated_at = now()
      FROM stale s
     WHERE m.id = s.id
       AND m.status = 'scheduled'
       AND m.kickoff_at = '2026-11-25 20:00:00+00'::timestamptz
    RETURNING m.id
)
INSERT INTO ops.audit_event
    (id, actor, action, subject_kind, subject_id, detail, created_at)
SELECT gen_random_uuid(), 'cli', 'external_source_correction', 'match', id,
       jsonb_build_object(
           'reason', 'ESPN: a fixture 11-27/11-28-ra átütemezve; a 11-25-i sor elavult',
           'old_status', 'scheduled',
           'new_status', 'canceled',
           'source', 'https://www.espn.com/soccer/match/_/gameId/401888557',
           'policy', 'S216-web-correction'
       ), now()
  FROM upd;

-- 2) Ciprus-kupa: a 11-25-i dátum a research feedben placeholder volt.
--    A CFA hivatalos programja szerint a nyolc meccs 12-09 és 2027-01-27
--    között van. A megadott idők Ciprus helyi ideje (EET, UTC+2), UTC-re
--    konvertálva. Minden sor régi időpontja 2026-11-25 14:00 UTC.
WITH corrected(id, new_kickoff) AS (
    VALUES
        ('01a1fc05-1cf0-72dc-ae2c-2ff8a821a9c8'::uuid, '2026-12-09 17:00:00+00'::timestamptz), -- AEL – EN Paralimni
        ('01a1fc05-1cf3-70ba-b514-157cb03e3613'::uuid, '2026-12-16 17:00:00+00'::timestamptz), -- Krasava – ASIL
        ('01a1fc05-1cf6-70e7-8c84-bc9419e35038'::uuid, '2027-01-06 17:00:00+00'::timestamptz), -- Omonia – Olympiakos
        ('01a1fc05-1cf1-7377-986c-65bd542b397f'::uuid, '2027-01-20 16:00:00+00'::timestamptz), -- APOEL – Aris
        ('01a1fc05-1cef-703a-ba38-cd2f09e82248'::uuid, '2027-01-20 17:00:00+00'::timestamptz), -- AEK – Anorthosis
        ('01a1fc05-1cf5-7638-ba8e-2109356f96ab'::uuid, '2027-01-27 15:00:00+00'::timestamptz), -- Omonia Arad. – Omonia 29M
        ('01a1fc05-1cf4-724e-85f3-b2b5bebf9fad'::uuid, '2027-01-27 16:00:00+00'::timestamptz), -- Nea Salamis – Pafos
        ('01a1fc05-1cf2-74bd-bf0c-86aabd7aeeaa'::uuid, '2027-01-27 16:00:00+00'::timestamptz)  -- Karmiotissa – Apollon
), upd AS (
    UPDATE core.match m
       SET kickoff_at = c.new_kickoff, updated_at = now()
      FROM corrected c
     WHERE m.id = c.id
       AND m.kickoff_at = '2026-11-25 14:00:00+00'::timestamptz
    RETURNING m.id, c.new_kickoff
)
INSERT INTO ops.audit_event
    (id, actor, action, subject_kind, subject_id, detail, created_at)
SELECT gen_random_uuid(), 'cli', 'external_source_correction', 'match', id,
       jsonb_build_object(
           'reason', 'CFA hivatalos programja: a kupa meccseit nem 11-25-én játszották',
           'old_kickoff', '2026-11-25T14:00:00Z',
           'new_kickoff', new_kickoff,
           'source', 'https://larnakaonline.com.cy/2026/11/23/kypello-stis-20-ianouariou-to-aek-anorthosi-olo-to-programma/',
           'policy', 'S216-web-correction'
       ), now()
  FROM upd;

-- 3) Holland Eerste Divisie, 17. forduló: a Voetbalkrant "Uitg" jelzése
--    szerint az Emmen – Helmond meccs el lett halasztva.
WITH upd AS (
    UPDATE core.match
       SET status = 'postponed', updated_at = now()
     WHERE id = '01a1fc05-915c-70d8-9682-cec1af5aeddc'
       AND status = 'scheduled'
       AND kickoff_at = '2026-11-28 15:30:00+00'::timestamptz
    RETURNING id
)
INSERT INTO ops.audit_event
    (id, actor, action, subject_kind, subject_id, detail, created_at)
SELECT gen_random_uuid(), 'cli', 'external_source_correction', 'match', id,
       jsonb_build_object(
           'reason', 'Voetbalkrant 17. forduló: Uitg (elhalasztva)',
           'old_status', 'scheduled',
           'new_status', 'postponed',
           'source', 'https://www.voetbalkrant.com/nederland/eerste-divisie/2026-2027/kalender/speeldag-17/fc-volendam/eindhoven',
           'policy', 'S216-web-correction'
       ), now()
  FROM upd;

COMMIT;

-- Ellenőrzés (read-only):
-- select count(*) from ops.audit_event where action='external_source_correction';
-- select m.kickoff_at::date, count(*) from core.match m where m.id in (...) group by 1;
