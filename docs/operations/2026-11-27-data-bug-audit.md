# 2026-11-27 — Adatminőségi hibák: gyökérokok és kapu

Ez a dokumentum a felhasználó által 2026-11-27-en jelzett három esetet
vizsgálja ki. Minden szám a dev adatbázis read-only lekéréséből származik.

## 1. Paraguayan Primera — duplikátum és hiányzó eredmény (2026-11-25)

**Tünet:** a három meccs kétszer-háromszor szerepelt a listában, eredmény nélkül.

**Gyökérok:** a research napi JSON ugyanazt a három fixture-t két külön
bejegyzésként hozta:
- `fm:1000017473` / `fm:...472` / `fm:...476` — **date_only** kezdés (00:00 UTC
  placeholder), verseny: `Division Profesional: Clausura`, ország PY;
- `espn:401888557` / `...558` / `...643` — **pontos** 20:00 UTC kezdés,
  verseny: `Paraguayan Primera`, ország XX.

A kanonikus csapat-ID-k **megegyeznek** (pl. `Luqueño` és `Olimpia` ugyanazok
az entitások), de a `canonicalize.rs::find_cross_source_fixture` csak akkor
köt hidat, ha a verseny országa nem null, a kezdések 15 percen belül vannak,
és a verseny azonos. Itt az ország XX volt és 20 óra a különbség, tehát három
külön `core.match` sor keletkezett.

**Eredmény:** a research mind a hat obszervációja máig `status=scheduled`,
`result=null` — a forrás nem adott eredményt. Nem lehetett kitalálni.

**Javítás (S215):**
- az integritási riport új mutatója, `unresolved_duplicate_fixture_groups`,
  **kemény hibaként** fail-el (ID-alapú, tehát nem fuzzy név-egyezés; az
  azonos nevű férfi/női NCAA-csapatok nem esnek bele);
- a napi API (`fetch_day`) determinisztikusan egy sorra szűri a csoportot:
  a pontos kezdésű sort tartja meg, a `date_only` placeholder-eket elrejti;
  a hiány-számlálók (`fetch_day_gaps`) ugyanazt a szűrőt használják.

**Korrekció a helyszíni ellenőrzés során:** páronként nem két placeholder,
hanem **egy `date_only` + két `exact`** sor volt (a második pontos sor a XX
országkódú verseny alatt, saját source-refel). Ezért az S216 adattisztítás
**csak a 3 `date_only` sort** állította `canceled`re; a két pontos sor közül
egyik sem lett törölve vagy leállítva. Az API a két pontos sor közül is
determinisztikusan egyet jelenít meg (azonos kanonikus csapatpár + azonos
kickoff-pillanat), ezért a felhasználó három sort lát.

**Mérés:** a javítás előtt a riport `FELOLDATLAN cross-source duplikátum: 3`,
`verdict: FAIL`. Az API 6 helyett **3** paraguayi meccset ad vissza, mind
pontos 20:00 UTC kezdéssel; `missing_kickoff` 7-ről 4-re csökkent.
Az S216 rendezés után: dry-run 3 → apply 3 sor → újra 0; a riport
`duplikátum: 0`, `verdict: OK — de ADATHIÁNY`, `daily-check` **ZÖLD**.
A 3 audit event az `ops.audit_event` táblában `duplicate_fixture_cancel`
akcióval visszakereshető.

## 2. Ciprus-kupa — nincs eredmény (2026-11-25)

**Tünet:** 8 meccs, mind `scheduled`, eredmény nélkül.

**Gyökérok:** a research napi JSON és az összes rögzített obszerváció ezeket
is `scheduled`/`result=null` értékkel tartalmazza még 11-27-i frissítésben is.
Más forrás (`fotmob`, `soccerway`, `footballdata`, `sofascore`) egyáltalán nem
rögzített hozzájuk obszervációt. **Nincs mit visszajátszani** — az adat a
forrásban hiányzik.

**Kezelés:** a kártyán a korábbi S214 óta `VÁRHATÓ` jelzés jelenik meg a lejárt,
de le nem zárt meccseknél; az integritási riport mostantól külön számolja:
`lejárt, scheduled, eredmény nélkül: 36` a 11-25…11-27 ablakban (globálisan
135). Ez adathiány-jelzés, nem kemény integritási hiba: eredményt nem
találunk ki (AGENTS.md §6).

## 3. Algéria — két kézi meccs elhalasztva (2026-11-25)

**Tünet:** a felhasználó által beírt `CR Belouizdad – CS Constantine` és
`USM Alger – MC Alger` meccsek `postponed` állapotban vannak.

**Gyökérok:** **nem rendszerhiba.** Az `ops.audit_event` tanúsága szerint a
felhasználó 2026-11-27 16:58-kor maga szerkesztette át mindkettőt
`scheduled` → `postponed` állapotra, `el lett halasztva` indoklással. A
manual-edit út observationt, provenance-t és audit eventet írt — ez a
kívánt, visszakövethető viselkedés. Ha a szándék most az eredmény rögzítése,
az újabb kézi szerkesztéssel vagy forrásadattal lehet megtenni.

## Miért nem derült ki korábban

A `footballctl integrity` már mérte az `azonos csapatpár azonos napon`
mutatót (11-25-re: 5), de a `has_hard_failures()` nem vette figyelembe, így a
verdict `OK` maradt. A `daily-check.sh 2026-11-26` ezért volt zöld. Az S215
óta a determinisztikus, ID-alapú duplikátumkemény hibának számít, a lejárt
eredményhiány pedig külön, látható mutató.

## Újra futtatható ellenőrzés

```bash
./rust/target/debug/footballctl integrity --from 2026-11-25 --to 2026-11-27
curl -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:$PORT/v1/day/2026-11-25"
```

## Visszaállás

- A kód `git revert`-tel visszaállítható; az API-szűrés nem ír adatot.
- Az S216 által leállított 3 `core.match` sor `status` mezője `canceled`ről
  `scheduled`re állítható vissza; a pontos értékek az audit event `detail`
  mezőjében és a fenti dry-run listában megvannak. Törlés nem történt.
- A kézi algériai szerkesztések audit eventben visszakereshetők; nem lettek
  felülírva.

## 4. Külső forrás-ellenőrzés (2026-11-27, web)

A felhasználó kérésére külső oldalakon is ellenőriztem a három esetet.
Ezek a leletek **felülírják** a korábbi „a forrásban nincs eredmény"
megállapítást: a meccsek nem lettek elhalasztva úgy, hogy nyomuk veszett
volna — **át lettek ütemezve, illetve a dátumuk placeholder volt**.

### 4.1 Paraguay — átütemezve (ESPN)

Az ESPN meccslapjai a `401888557`, `401888643`, `401888558` játékID-khez
(amelyek a mi adatbázisunkban is szerepelnek) ma ezeket a dátumokat mutatják:

| Meccs | Eredeti (nálunk) | ESPN ma |
|---|---|---|
| Luqueño – Olimpia | 11-25 20:00 UTC | **11-27 23:00** |
| Nacional – Rubio Ñú | 11-25 20:00 UTC | **11-27 20:30** |
| Trinidense – Recoleta | 11-25 20:00 UTC | **11-28 23:00** |

Források:
- https://www.espn.com/soccer/match/_/gameId/401888557
- https://www.espn.com/soccer/match/_/gameId/401888643
- https://www.espn.com/soccer/match/_/gameId/401888558

A mi DB-nkben a három 11-25-i `Paraguayan Primera` sor **scheduled** maradt,
közben ugyanezek a fixture-ek `Division Profesional: Clausura` alatt már
11-27-nél, illetve 11-28-nál is megjelennek (kettő közülük). A harmadik
(Trinidense–Recoleta) új dátuma még nincs benne az adatbázisban. Tehát nem
eredményhiány, hanem **elavult dátum + párhuzamos kanonikus sor**.

### 4.2 Ciprus-kupa — a 11-25-i dátum nem valós (CFA hivatalos program)

A Ciprusi Labdarúgó-szövetség 2026-11-23-i hivatalos programja szerint
a nyolc nyolcaddöntő meccs **egyikét sem** 11-25-én játszották:

- 2026-12-09: AEL – EN Paralimni
- 2026-12-16: Krasava ENY – ASIL
- 2027-01-06: Omonia – Olympiakos
- 2027-01-20: APOEL – Aris; AEK Larnaca – Anorthosis
- 2027-01-27: Omonia Aradippou – Omonia 29M; Nea Salamis – Pafos; Karmiotissa – Apollon

Forrás: https://larnakaonline.com.cy/2026/11/23/kypello-stis-20-ianouariou-to-aek-anorthosi-olo-to-programma/
A sorsolásról szóló hír (goal.philenews.com) szintén azt írta, hogy a meccsek
„legkorábban 2026. november 25-én kezdődhetnek" — a research feed ezt a
**lehetséges legkorábbi dátumot** rögzítette tényleges kickoffként.

### 4.3 Holland Eerste Divisie, 17. forduló — nincs hiányzó eredmény

A Voetbalkrant 2026/2027-es 17. forduló programja (és az azt mutató
OneFootball-meccslap) szerint a forduló 10 meccse:

- elhalasztva (`Uitg`): Utrecht–Almere, Dordrecht–Vitesse, Jong AZ–Oss,
  VVV–RKC, Emmen–Helmond;
- lejátszva: NAC **3-0** Den Bosch, MVV **0-2** Jong PSV,
  Jong Ajax **1-3** Heracles;
- 11-29: Volendam–Eindhoven 12:15, Roda–De Graafschap 16:45.

Forrás: https://www.voetbalkrant.com/nederland/eerste-divisie/2026-2027/kalender/speeldag-17/fc-volendam/eindhoven

A mi adatbázisunkban pontosan ezek vannak, **kivéve a 11-29-i két meccset**:
a research napi JSON 11-29-re egyáltalán nem létezik, ezért nem lett
letöltve. Ez nem megjelenítési hiba, hanem **feed-lefedettségi hiány**.
Egy eltérés: az Emmen–Helmond meccset a külső forrás elhalasztottnak
mutatja, nálunk `scheduled` (11-28 16:30) — ezt külön kell egyeztetni.

## 5. Élő percek — gyökérok és javítás (2026-11-27 este)

**Tünet:** a Győri ETO–Debrecen kártyája 42'-t mutatott, miközben a meccs
későbbi fázisban járt (a felhasználó 54'-et látott); a német meccseknél
pontosnak tűnt az óra.

**Gyökérok:** nem a kliens számolása, hanem a betöltési lánc állt meg.
A `feat/ops-triage-console` ág 0032-es migrációja **eltörölte** a
`raw.capture (source_id, request_key)` UNIQUE kényszert, de a worker és a
fixture-adapter kód még `ON CONFLICT (source_id, request_key) DO NOTHING`-et
használt. PostgreSQL ezért `there is no unique or exclusion constraint
matching the ON CONFLICT specification` hibát adott, és minden research
discovery job a `capture beszúrás` lépésben failed. Az élő adat ezért a
20:24-i snapshoton ragadt (Győr: 42'), miközben a research JSON már 88'-t
tartalmazott a 21:29-i revízióban.

**Javítás (S219):**
- a capture INSERT többé nem hivatkozik a törölt kényszerre;
- a capture azonosítót `ORDER BY received_at DESC, id DESC LIMIT 1`
  lekérdezéssel választjuk (a 0032-es migráció szándékának megfelelően a
  legfrissebb fetch nyer);
- ugyanez a javítás a fixture-adapterben;
- a storage-teszt frissítve az új, nem-idempotens capture-számára.

**Mérés:**
- a sikertelen job újraütemezése után a Győr–Debrecen perc **42 → 88**, az
  API most `minute=88`, a DB legfrissebb obszervációja 21:29:12 (88');
- a 11-27-i napi API **47 élő meccséből 0 perceltérés, 0 státuszeltérés,
  0 eredményeltérés** a research JSON-hoz képest;
- külső ellenőrzés: a Sofascore/FotMob meccslapjai 83' eseményt, illetve
  90+1-ig terjedő eseménylistát mutatnak — a 88' ebbe belefér; a korábbi
  42' valóban elavult érték volt.
- `ops.job`: 11-27-re `done=71`, failed=0; workspace teszt 0 failed,
  clippy 0, fmt 0, build OK; daemon újratelepítve, `/v1/health` OK.

**Kapu-hiány:** a `daily-check 2026-11-26` zöld maradt, pedig a 11-27-i
research job failed volt (a napi ellenőrzés csak az előző nap ablakát nézte).
Javaslat: a failed job-ok ellenőrzése minden napra vonatkozzon, ne csak a
vizsgált dátumra.
