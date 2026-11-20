# Csapatlogók (S40)

A kliens minden csapatnál először címer-képet próbál betölteni, és csak ha nincs
(elérhetetlen vagy nem létezik), akkor rajzol monogramot (pl. `FER`, `DEB`).
A kép forrása kizárólag a v2 saját asset-tára (`ops.asset`), amit a nap-feed
`home_logo` / `away_logo` mezőjében ad át az API.

## Honnan jönnek a képek?

Két import-script tölti fel az `ops.asset` táblát; mindkettő idempotens
(`ON CONFLICT DO NOTHING`), és csak akkor ír, ha a csapat már létezik a
kanonikus törzsben.

1. `scripts/import-v1-team-logos.py` — a v1 `assets-manifest/manifest.json`
   WebP-fájljait másolja a v2 adatkönyvtárába tartalom-címzett kulccsal
   (`assets/<aa>/<bb>/<sha256>.webp`), majd `storage_key`-jelöléssel írja be.
   A v1-ben képpel rendelkező csapatokat a `v1t-<id>` forrás-hidakon keresztül
   köti a v2 csapataihoz. A manifestben nem szereplő, de `teams.logo_url`-lel
   bíró v1-csapatok URL-referenciát kapnak.
2. `scripts/import-luukhopman-logos.py` — a `luukhopman/football-logos` GitHub
   repó fájljaihoz rendel pontos (ékezet nélküli) név-egyezéssel, majd
   rövidítés-tűrő második körrel (pl. „Ath Bilbao” ↔ „Athletic Bilbao”).
   Csak egyértelmű találatot fogad el; URL-referenciát ír be, képet nem másol.

Futtatás (a `DATABASE_URL` a `.env`-ből):

```bash
set -a && . ./.env && set +a
python3 scripts/import-v1-team-logos.py
python3 scripts/import-luukhopman-logos.py
```

## Licenc

A klubcímerek védjegyek; a `luukhopman/football-logos` repóban nincs
licencfájl. Ezért a repóra mutató sorok `licence_status = 'unknown'`, és az
`attribution` megnevezi a forrást. A v1-ből átvett WebP-k a Wikimedia
Commonsból származnak (CC0/CC-BY), de a manifest soronként nem hordoz licencet,
így ezek is `unknown` állapotúak. A naplózott döntés:
`docs/decisions/2026-11-20-csapatlogok.md`.

## Mért lefedettség (2026-11-20)

- `ops.asset` team-logo sorok: **566** (v1 blob, v1 URL, luukhopman URL).
- A napi feedben (2026-11-20, 104 meccs): **20 meccsnél van legalább az egyik**
  csapatnak logója, **7 meccsnél mindkettő**.
- A lista a csapat-identitás pontosságán múlik: ahol a forrás neve és a
  kanonikus név eltér, ott monogram marad. Ez az S37d/S37f identity-szeletek
  feladata.

## Korlátok

- Új csapat (research create-on-miss) csak akkor kap logót, ha a következő
  importfuttatás név-egyezést talál.
- A logó nem befolyásolja a publish-gate-et; a hiányzó kép nem adathiba.
