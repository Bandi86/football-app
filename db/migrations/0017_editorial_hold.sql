-- 0017 — szerkesztői visszatartás (S50).
--
-- A forrásadatok között előfordul, hogy ugyanaz a csapat egy napon két,
-- egymással ellentmondó fixture-ben szerepel (a research JSON is így adja).
-- Ilyenkor a publish gate nem tud dönteni, a reverify pedig felülírja a kézi
-- withheld állapotot. Ezért külön, tartós tábla jelöli az ember által
-- ellenőrzött, a napi listáról kizárandó meccseket. A napi lekérdezés
-- (day.rs) és a hiány-mutatók is kiszűrik ezeket.
--
-- Visszaállítás: DROP TABLE ops.editorial_hold; a napi lekérdezésből a
-- NOT EXISTS szűrő eltávolítása.

CREATE TABLE IF NOT EXISTS ops.editorial_hold (
    match_id uuid PRIMARY KEY REFERENCES core.match(id) ON DELETE CASCADE,
    reason text NOT NULL,
    note text,
    created_at timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE ops.editorial_hold IS
    'S50: ember által ellenőrzött, a napi listáról kizárt meccsek (pl. ellentmondó forrás-fixture).';