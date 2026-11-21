# Kézi meccsszerkesztés a kliensből (S70 → S80)

A rendszer alapelve a forrásvezérelt, ellenőrzött adat. Előfordul azonban, hogy
egy eredmény vagy egy meccs-paraméter egyik elérhető forrásból sem jön meg
(kis liga, elavult forrás, forrás-oldali hiba), vagy a forrás hibásan adja.
Ilyenkor a felhasználó a **meccs-részletezőben** maga is szerkesztheti a
meccset — de nem „csak úgy": az írás ugyanolyan bizonyíték- és auditnyomot hagy,
mint egy forrásból származó revízió, és a publikálási kapu újraszámolódik.

## Használat

1. Nyisd meg a meccs részletezőjét a napi listáról.
2. Kattints a **„Meccs szerkesztése"** gombra (`manual.editor`).
3. A lap három szekcióból áll:
   - **Alapadatok**: státusz (9 érték), kickoff-pontosság (`Pontos idő` /
     `Csak dátum`), kezdés dátuma és ideje. `Csak dátum` választásakor az idő
     letiltódik, és a kezdés a 12:00:00Z helyőrzőre normalizálódik. A
     DatePicker a verseny időzónájában jelenik meg (a részletező válasz
     `competition_timezone` mezője alapján), a payload mindig UTC `…Z`.
   - **Eredmény**: FT, félidő (HT), hosszabbítás (ET), tizenegyesek (pens) és
     `decided_by` (`ft`/`et`/`pens`). A szekciók feltételesen jelennek meg:
     HT csak lezárt (nem awarded) meccsnél; ET csak döntetlen FT után; pens
     csak döntetlen ET után; awardednél HT/ET/pens nem adható meg. Leállított
     státusznál (scheduled…abandoned) az eredmény-szekció rejtett.
   - **Indoklás**: kötelező, nem üres szöveg (pl. „SofaScore 20/11: 2–1").
4. A **Mentés** (`manual.submit`) után a részletező automatikusan újratölti a
   kanonikus adatot; a kapuállapotot az újraellenőrzés számolja ki. Hiba esetén
   a szerver magyar üzenete a `manual.error` helyen jelenik meg.

## API-szerződés

- **Végpont:** `PUT /v1/match/{id}/manual-result`, csak **ops token**.
- Minden mező opcionális a kötelező `note` kivételével:
  `kickoff_at` (RFC 3339, UTC), `kickoff_precision` (`exact|date_only`),
  `status` (`scheduled|confirmed|live|halftime|finished|postponed|canceled|abandoned|awarded`),
  `home_ft/away_ft/home_ht/away_ht/home_et/away_et/home_pens/away_pens`
  (egész, >= 0), `decided_by` (`ft|et|pens`).
- **Jelenlét-szemantika:** ha bármelyik eredmény-mező (vagy `decided_by`)
  jelen van a payloadban, a teljes `core.match_result` sor **cserélődik** — a
  hiányzó pontszám `NULL` lesz. A `kickoff_precision` `date_only`-ra váltásához
  a kliens a `kickoff_at`-ot is küldi (a UI mindig küldi).
- **Státusz-szabályok:** eredmény + nem-eredmény státusz → 400; eredmény
  státusz nélkül → a meccs `finished` lesz; `canceled`/`postponed` eredmény
  nélkül → a meglévő eredmény törlődik; csak kezdés/pontosság szerkesztés
  esetén az eredmény érintetlen.
- **Natural key ütközés:** ha a módosított
  `(competition, season, round, home, away, kickoff)` kulcs egy másik meccsre
  illeszkedik, a válasz **409**, a `conflict_match_id` mezőben a másik meccs
  azonosítójával.
- **Hibák:** 400 mezőnkénti magyar üzenet, 401 nincs jogosultság, 404 ismeretlen
  meccs, 500 váratlan hiba.

Példa (teljes eredmény hosszabbítás után):

```json
{
  "status": "finished",
  "home_ft": 1, "away_ft": 1,
  "home_ht": 1, "away_ht": 0,
  "home_et": 2, "away_et": 1,
  "decided_by": "et",
  "note": "Kupameccs, hosszabbítás után 2–1"
}
```

## Validáció (domain)

A `football_domain::manual_edit::validate_manual_edit` a HTTP-handlertől
függetlenül, tesztelten dönt. Fő szabályok: nemnegatív gólszámok; FT mindkét
oldalon kötelező, ha eredmény-blokk jön; HT <= FT; ET csak döntetlen FT után és
ET >= FT; pens csak döntetlen ET után és nem döntetlen; `decided_by` konzisztens
az eredmény alakjával; awardednél HT/ET/pens tilos; `date_only` esetén a kezdés
12:00:00Z.

## Technikai háttér

- **Forrás:** `ops.source.code = 'manual-ui'`.
- **Bizonyíték:** nyers JSON artifact (content-addressed) → `raw.capture` →
  `stage.observation` (aspect `match_core`, a meccs teljes új állapotával).
- **Provenance:** csak a ténylegesen változott mezők kapnak új selected sort
  (`policy = 'manual-2.0'`); a korábbi selected sorok `is_selected=false`-re
  kerülnek, a megfigyelések nem törlődnek.
- **Kanonikus írás:** `core.match` (kickoff, precision, status) és
  `core.match_result` (teljes sor: FT/HT/ET/pens, `decided_by`,
  `winner_method`, `validity`) egy tranzakcióban; a régi gate sorok a
  kickoff-dátum változása után törlődnek (derivált adat).
- **Audit:** `ops.audit_event`, actor `ui`, action `manual_match_edit`,
  részletben `before`, `after`, `note`, `observation_id`.
- **Publikálási kapu:** a handler a `reverify_match` úton újraellenőrzi a
  meccset — a kézi út **nem kerüli meg** a verification láncot, csak új
  bizonyítékot ad hozzá.

## Korlátok és visszaállítás

- Ha csak a pontosságot váltod `date_only`-ra, a kezdés normalizálásához a
  kliens a `kickoff_at`-ot is küldi; API-szinten a precision önmagában nem
  normalizálja a meglévő kezdést.
- Hiányos FT (pl. csak `home_ft`) 400-at ad — szándékosan szigorúbb, mint a
  „hiányzó = NULL" sorcsere.
- Versenyhelyzetben a natural key egyedi indexe 500-at adhat 409 helyett; az
  adat nem sérül.
- Visszaállítás = új, ellenkező értékű kézi szerkesztés (az auditban ott a
  `before` állapot), vagy a forrás-oldali revízió újrafuttatása.

## Ellenőrzés

```bash
cargo test -p football-api --test manual_result -- --test-threads=1
cargo test -p football-domain --lib -- --test-threads=1
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path apps/FootballAppCore
```