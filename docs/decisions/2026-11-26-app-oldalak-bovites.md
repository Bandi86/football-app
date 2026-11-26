# App-oldalak bővítésének döntései (2026-11-26)

**Helyzet:** a `2026-11-25-app-oldalak-bovites-design.md` vázlat 10. szakasza
hat nyitott döntést tartalmazott. A felhasználó 2026-11-26-on mind a hat
ajánlott alapértelmezést elfogadta. A döntések az S1-től érvényesek.

## Döntések

1. **Írási jogosultság:** a teljes meccs-szerkesztés és a logófeltöltés is
   **ops tokenes** marad. A read-tokenes kliens nem írhat; minden írás
   auditált (`ops.audit_event`).
2. **`manual-result` és teljes szerkesztés:** új, egységes
   `PUT /v1/match/{id}/edit` végpont jön az S4-ben, a régi
   `PUT /v1/match/{id}/manual-result` kompatibilitási okból megmarad.
3. **`ops.issue` generálása:** a `daily-check` / `integrity` a jeleket
   **perzisztensen az adatbázisba írja** (nem a UI számolja élőben), mert az
   idővonalhoz (`first_seen`, `last_seen`, `resolved`) kell a megőrzés.
   A detektor bekötése az S3-ban történik; az S1 az írási repository-utat
   adja meg és teszteli.
4. **Jegyzet-láthatóság:** a read-tokenes kliens **olvashatja** a jegyzeteket
   külön végponton, de a jegyzetek **nem kerülnek a nyilvános napi feedbe**
   (`/v1/day/{date}` szerződése változatlan).
5. **Logóformátum:** az első szeletben **csak WebP/PNG**. Az SVG külön
   biztonsági review (szkript-mentesítés) után jöhet szóba.
6. **AI-gomb:** **alapból kikapcsolt**. A helyi modell és a SearXNG
   elérhetőségét külön üzemeltetési döntésként kell rögzíteni; a gomb
   javaslat-only marad, kanonikus adatot nem ír.

## Következmények

- Az S1 API-jában az olvasó végpontok (`/v1/issues`, notes list, history)
  read scope-ot, az író végpontok ops scope-ot használnak.
- Az OpenAPI-dokumentum új útvonalai mind `security(("bearer" = []))`-vel
  készülnek.
- A döntések nem tartalmaznak adat- vagy kódmódosítást; a megvalósítás
  szeletenként a `docs/superpowers/plans/2026-11-26-app-oldalak-s1.md` és
  az azt követő tervek szerint történik.
