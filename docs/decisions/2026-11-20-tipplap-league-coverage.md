# Tipplap liga-lefedettség bővítése (S36a, 2026-11-20)

**Helyzet:** a felhasználó eldöntötte, hogy a v2 napi odds-lefedettségét a v1-ben már
bejáratott Tipplap-mechanizmus átemelésével kell javítani („az app v1-ben ott a tipplap
jol be van kotve es ott nap mint nap rengeteg meccset oddsot tolt le, csak azt kell at
emelni azt a mechanikat es ahol kell javitani”). A v2 Tipplap-adapter eddig csak
**16 ligacímet** ismert (`LEAGUES`), ezért a napi oldal sorainak túlnyomó része
kimaradt: 2026-11-20-án a daemon logja `rows=118 skipped=114`, azaz **4 meccs** került
be, és a nap 124 v2-meccséből is csak 4 kapott oddsot.

## Döntés

1. **Adatvezérelt bővítés, nem a v1 alias-tábla vak átemelése.** A v1
   `tipplap_league_alias` (434 reviewed sor) csak azt mondja meg, hogy a v1 melyik
   versenyéhez tartozik egy cím; a v2-ben viszont más a verseny-univerzum (326 verseny,
   176 meccsel). Ezért a v1 Tipplap-eseményeket (2026-11-06..21, 2204 esemény) a v2
   meccseihez (1042) **egzakt, ékezet nélküli csapatpár-egyeztetéssel** kötöttem, és a
   találatokból a v2-versenyre szavaztam. Csak azt a címet vettem fel, amelyre legalább
   **2 szavazat** és **≥80% többség** van — így a bekerülő cím mögött már létező,
   valódi v2-meccsek állnak, és nem hozunk létre új versenyt egy ismeretlen ligának.
2. **Két tábla:** a nemzetközi/top-ligás `LEAGUES` marad (prefix-illesztés,
   forduló-címkével), mellé `GENERAL_LEAGUES` került (26 új cím, pontos egyezés
   ékezet/kisbetű toleranciával, forduló-címke nélkül). Ismeretlen cím továbbra is
   kimarad — nincs találgatás, nincs automatikus verseny-létrehozás.
3. **Ismert ország-adathiba kezelése:** a v2-ben az angol ligák a `GB` ország alatt
   vannak, amelynek a neve tévesen „Scotland”, és a skót ligák is `GB` alatt vannak.
   Az új GB-sorok ezért a meglévő `("GB", "Scotland")` párt használják, hogy ne
   módosítsuk a `core.country` sort ebben a szeletben; a valódi javítás (ENG/GB
   szétválasztás) külön döntés.
4. **Nincs deployment ebben a szeletben:** a futó launchd-daemon a régi binárist
   futtatja; az új lefedettség élesben akkor jelenik meg, ha a daemont újraépítjük és
   újratelepítjük (`scripts/install-footballd-agent.sh`, külön döntés).

## Bizonyíték

- Tesztek: `cargo test -p football-ingest --lib tipplap` → 2 zöld;
  `cargo test -p football-ingest --test tipplap` → 5 zöld (új:
  `parses_extended_league_from_general_table` — League One/GB, 1 odds-observáció).
- Élő oldal (a daemon által 2026-11-20 14:12-kor mentett, `zstd`-vel kicsomagolt
  `/tmp/tipplap-1120.html`, 629 961 byte, 60 ligaszakasz, 354 `data-event` sor) az
  **új** kóddal: `match_core=44, odds=44, competitions=19`; a régi kóddal ugyanazon a
  napon `rows=118 skipped=114` → **4 meccs**. Parse-szintű lefedettség: 4 → 44.
- A 44 parsolt meccsből **24** csapatpár egyezik a nap 124 v2-meccsével (egzakt,
  ékezet nélküli összevetés), tehát a várható nyereség a mai napon 4 → legfeljebb 24
  oddsos meccs, a csapat-identitás feloldásától függően.
- Nem ellenőrzött: a tényleges betöltés (kanonikus materializáció) az új binárison még
  nem futott, mert a daemon a régi kódot futtatja; a „legfeljebb 24” felső becslés, a
  valós szám a deployment után mérhető.

## Következmények

- A liga-lefedettség már nem a fő szűkítő tényező: a 44 parsolt meccs 19 versenyből jön,
  a maradék kimaradó címek többségéhez nincs (vagy csak 1 szavazatnyi) v2-meccs.
- A következő szűkítő a **csapat-identitás** (v2-nek nincs verseny-szintű source-ref-je,
  a v1t- csapatrefek csak a nevek egy részét fedik) és a **napon belüli frissítés**
  (minden policy csak napi egyszeri `discovery`, `interval_minutes=1440`).
- A deployment után a lefedettséget ugyanezzel a méréssel kell ellenőrizni (napi
  oddsos meccsek száma + `identity.review_queue` nyitott sorai).