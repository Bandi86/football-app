# Döntés: csapatlogók forrása és licenc-állapota (S40)

## Probléma

A kliens eddig csak monogramot tudott rajzolni, mert a v2 `core.team` táblának
nincs logó-oszlopa: a képek az `ops.asset` tárban élnek. A v1-ben viszont
voltak csapatlogók (WebP a Wikimedia Commonsból + URL-ek a
`luukhopman/football-logos` repóból), és a felhasználó kérte ezt a képességet.

## Döntés

1. A v2 ugyanazt a két forrást használja, mint a v1, de **az `ops.asset`
   táblán keresztül**, nem új kolumnával a `core.team`-en.
2. Helyi WebP-t csak a v1-ből átvett, Wikimedia Commons-származású készletből
   másolunk (a v2 saját adatkönyvtárába, tartalom-címzett kulccsal); a GitHub
   repóból **csak URL-referenciát** írunk be, képet nem másolunk.
3. Minden sor `licence_status = 'unknown'`, mert sem a v1 manifest, sem a
   GitHub repó nem hordoz soronkénti licencet; az `attribution` megnevezi a
   forrást és annak licenc-helyzetét.
4. A kliens a nap-feed `home_logo`/`away_logo` mezőjét használja; ha nincs kép
   (vagy nem tölthető be), monogram-fallback rajzolódik.

## Elvetett alternatívák

- **Új `logo_url` oszlop a `core.team`-en** — elvetve: párhuzamos tár lenne az
  `ops.asset` mellett, és elveszítené a licence/attribution mezőket.
- **Képek bemásolása a GitHub repóból a repóba** — elvetve: a repó publikus,
  a címerek védjegyek, licence nélküli terjesztésük nem vállalható.
- **Generált címer** — elvetve: kitalált címer nem a csapat címere, félrevezet.

## Korlátok

- A lefedettség (~430 egyező csapat a repóból) a név-egyezésen múlik; az
  ismeretlen nevű csapatoknál monogram marad.
- A `licence_status = 'unknown'` őszinte állapot; üzleti felhasználás előtt
  a licence-tételes ellenőrzés (v1 mintájára) továbbra is szükséges.
