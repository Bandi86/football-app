# Hátralévő hézagok és nyitott kérdések

Állapot: 2026-11-25. A research backfill a célig (2026-08-01) lezárult: 108 nap,
0 hiba. Ez a dokumentum azokat a problémákat gyűjti, amik **nem blokkolják**
az adatfeldolgozást, de külön döntést vagy későbbi kutatást igényelnek.

## 1. Adatintegritás — ismert, nem javított

### Téves provenance-linkek (S140 — S149-ben rendezve)

**Állapot: megoldva.** A 70 téves provenance-sort (5 + 35 + 30, mind
`is_selected=false`) S149-ben töröltük, 3 audit eseménnyel. A megjelenített
adat változatlan maradt; integritás `OK`.

A research obszervációk néha tévesen kerülnek másik kanonikus meccshez:

| Research UID | Hova van linkelve | Valójában |
|---|---|---|
| `fm:1000021160` | felnőtt Real Madrid–PSV (Champions League, 5:1) | Real Madrid U19–PSV U19 (3:3) |
| `livescore:1872665` | Bandari–Kakamega Homeboyz | Bandari–Mathare United (1:0) |
| `fm:6015575` | Fleetwood–Charlton U21 (11-24) **és** Fleetwood–Leeds U21 (11-25) | két külön meccs |

A megjelenített adat ezeknél helyes (más forrásból / revízióval), csak az
audit-nyom téves. Tisztítás: a rossz `fact.provenance` sorok archiválása és
újra-linkelés — külön döntés, mert érintheti a field-level selectiont.

### `is_selected` mezőalapú frissítés (S140 — S149-ben javítva)

**Állapot: a kód javítva.** A canonicalize most a **legfrissebb** obszervációt
választja mezőnként (előbb kijelöli a régit az
`uq_provenance_selected_field` miatt, majd UPSERT-tel beállítja az újat).
Új teszt: `newer_observation_takes_over_selected_provenance`; Rust 591 zöld.
Valós ellenőrzés: 11-24 újra-ingestelése után a Shahin Bandar Ameri meccs
`kickoff_at` provenance-a a 07:39-as (14:00) obszervációra mutat, a régi
13:15-ös nem kiválasztott.

A kanonikus mezőket régen a **legrégebbi** obszerváció határozza meg
(`is_selected=true`), a frissebbek `false`-val kerülnek be. Emiatt a
státusz frissül (UPSERT), de a **kezdési idő nem**: két konkrét esetet
kellett kézi revízióval javítani (Shahin Bandar Ameri 13:15→14:00,
Al Sharkeyah 15:00→18:00). Ugyanez a mechanizmus okozta a 133 darab
elavult `scheduled` meccset is (S136). A selection-szabály megváltoztatása
kockázatos (dedupe-érzékeny), ezért külön szelet.

### Azonos csapatpár azonos napon

A teljes backfill-tartományban 16 ilyen figyelmeztetés. Átvizsgálva: többségük
jogos — finn Veikkausliiga fáziscsoportjai (TPS/SJK, HJK/Inter Turku) és
NCAA férfi/női meccsek, eltérő kezdési idővel és versennyel. Kettőt
cross-competition duplikátumként összeolvasztottunk (Fénix–Colón,
River Plate–Colón — S141). A maradékot nem olvasztjuk össze.

## 2. Forráslefedettség

### Sofascore-referenciából hiányzó ligák (S132 — S150 forráskutatás)

**Forráskutatás (2026-11-25)**: a research pipeline FotMob napi JSON-ja
(`fotmob-matches.json`) **nem tartalmazza** a hiányzó csapatokat. A
`FORRASOK-KATALOGUS.md` alapján a **BetExplorer** engedélyezett (robots üres,
nincs CF challenge): a `/football/results/?date=2026-11-24` oldal HTTP 200,
635 KB, valós eredményekkel. A 11 hiányzó meccsből **3-at** fed le:
`Ajman–Al Ettifaq 1:2`, `Al Duhail–Neom SC 3:2`, `Torreense U23–Vizela U23 2:1`.
A többi (Buru SC, Halcones, Vitesse U21, ASFA Dakar, Teranga, Koper U19,
Szaúd-U21 meccsek) nem szerepel — Maldives, Liga TDP, Senegal Ligue 2
egyáltalán nincs a feedben. A BetExplorer tehát **részleges** pótlás, új
adapter és külön forrás-engedély kellene hozzá.

**Megjegyzés**: a research pipeline jelenleg a FotMob `/api/data/matches`
végpontot használja, amit a saját forráskatalógusa kifejezetten tilt
(csak a renderelt `/matches?date=` oldal engedélyezett) — ez a research
projektben lévő, nem a v2-repo-beli kérdés, de érdemes külön napirendre tenni.

~11 meccs / 22 csapat nincs meg egyik engedélyezett forrásban sem
(Buru SC, Vitesse U21, Torreense U23, ASFA Dakar, Koper U19, Ajman,
Al Duhail, Halcones stb.). A research nyers adatában sem szerepelnek;
csak csapatidentitások vannak. A `footballdata` 22+15 rögzített ligát követ,
ezek nincsenek köztük. **Sofascore adapter kifejezetten tiltott** — új forrás
vagy ligakonfig csak külön döntéssel.

### Élő eredmények

A research API-ben nincs aktuális élő eredmény (csak félidős, `fotmob_events`
forrással), ezért az élő kártyák „–"-t mutatnak. A pontos játékidő
(`status.minute`) megvan és S134 óta eljut a kliensig. Új forrás nélkül az
élő eredmény nem pótolható.

### Egyforrású meccsek

Pl. `Neretva–Šibenik 3:0` (horvát kupa, 2026-09-08) csak FotMob-forrásból
van—webes forrás (Wikipedia, HRT) megerősítette (Šibenik visszalépett,
3:0 „bez borbe"), de sok hasonló meccsnél nincs független keresztellenőrzés.

## 3. Megjelenítés / szűrők

### S131 — öt percnél régebbi `scheduled` meccsek

6 meccs van úgy, hogy a kezdési ideje több mint 5 perce elmúlt, de a forrás
még `scheduled`-t mond: egyik státusz-szűrőben sincs (a `lejárt, nyitott`
3 órás küszöb alatt), csak a „Mind"-ben. Vagy a `open_past` küszöbét kell
leszállítani, vagy külön szűrőt adni — döntés kérdése.

### Placeholder kezdési idők (S148 — javítva)

843 meccsnek a research éjfél UTC-t adott (csak dátumot ismert). Ezek
`kickoff_precision='date_only'`-ra lettek állítva; a kliens helyettük
„időpont egyeztetés alatt"-ot mutat. A jövőbeli ingeseknél a kód is ezt
állítja be. Figyelni kell, hogy a 14:15-ös research-sync ne írja vissza
`exact`-re (a régi bináris ezt tette — az új daemon telepítve van).

## 4. Üzemeltetés

### Failed research job

Egy `refresh` job a 2026-11-22-i `bendel-insurance-vs-doma-united_fm6103183`
detail fájl hiányzó változata miatt `failed`. Nem mai adatot érint; a
research dokumentum hiányát külön kell pótolni, ha számít.

### Soccerway

A forrás `enabled=f` 2026-11-17 óta. Egy egyszeri szivárgás történt
11-24 07:31-kor (régi bináris), amiből 74 meccs került be — S133 óta
`editorial_hold`-ban vannak, audit eseménnyel. A tiltás utáni soccerway-only
publikált meccsek száma: **0**.

## 5. Szemantikai duplikátumok / identitás (S150 — feltárva, döntésre vár)

Példa: **Everton W – Brighton W** 2026-11-22 15:00, WSL:
- `01a1fc04-b694-…` — `Everton W – Brighton W`, `WSL` (GB-ENG), `finished`
- `01a202d6-60c0-…` — `Everton W – Brighton & Hove Albion`, `Women's Super League` (XX), `scheduled`, research forrás `espn:401902855`

Mindkét Brighton-entitás `female`, `senior`, azonos ország — tehát az
identitásfeloldás nem vette észre a teljes név és a rövidített „W" alak
egyezését. A `dedupe variants` és `dedupe aliases` dry-run **nem** találta
meg (1 névvariáns-csoport, 0 merge), ezért a két fixture a napi listában is
külön szerepel.

Hasonló, magas hasonlóságú női entitáspárok (csak **jelöltek**, nem
összevonhatók automatikusan): `UDG Tenerife W` / `UD Tenerife (W)`,
`Madrid CFF W` / `Madrid CF W`, `IFK Norrkoeping W` / `IFK Norrköping (W)`,
`IFK Goeteborg W` / `IFK Göteborg (W)`, `Djurgaarden W` / `Djurgården (W)`.
Ugyanakkor számos hasonló név **külön csapat** (pl. `Eastern Illinois` /
`Western Illinois`, `Kansas State` / `Arkansas State`) — ezért az AGENTS.md
által tiltott automatikus fuzzy merge helyett **kéntelen, forrás-alapú
döntés** kell páronként. A teljes átvizsgálás hátralévő munka.

## 6. Emberi ellenőrzésre vár

- Tényleges VoiceOver-felolvasás (emberi fül kell hozzá).
- Tab-kör kiterjesztése a natív menükre / naptár-popoverre / részletezőre.
- Alsóbb ligák webes keresztellenőrzése mintánként — eddig 138/138 minta
  egyezett a research forrással, és 7 webes forrás (BBC, ESPN, Guardian,
  premierleague.com, Wikipedia/HRT, Aston Villa, club sites) is megerősítette
  a mintát; a rendszeres webes audit még nem automatizált.
