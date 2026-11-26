# Modern Labdarúgás Predikciós Rendszer — Kutatási és Tervezési Vázlat (v2)

**Dátum:** 2026-11-23  
**Státusz:** Tervezet / Kutatás (RFC)  
**Cél:** A Football App v2 adatbázisára és infrastruktúrájára (Rust backend, PostgreSQL, macOS SwiftUI kliens) épülő, hosszú távon pontos, kalibrált és szigorúan as-of predikciós architektúra specifikálása. Nem tartalmaz kódot, kizárólag a modern módszertant, piac-specifikus modellezést, adatigényt, metrikákat és rendszertervet rögzíti.

---

## 1. Vezérelvek és Filozófia

1. **Nem „eredményt tippelünk”, hanem valószínűségi eloszlást becslünk:**
   A labdarúgás alacsony gólszámú, magas zajtartalmú sport. Egy jó predikciós rendszer nem azt mondja, hogy *„2-1 lesz”*, hanem kiszámítja a pontos gólvalószínűségeket $P(G_{hazai}=x, G_{vendeg}=y)$, amiből származtatható az 1X2 valószínűség, Ázsiai Hendikep, Over/Under, és az elméleti tisztességes odds (fair odds).
2. **Szigorú As-Of Garancia (Nincs Adatszivárgás / Look-ahead Bias):**
   Egyetlen olyan metrika vagy adat sem vehető figyelembe a $T$ meccs időpontjában, ami $T$ után keletkezett vagy módosult. A v2 as-of elve alapján minden feature-számítás a meccs kickoff előtti állapotára fagyasztott.
3. **Piac-relatív pontosság (Beat the Market / Calibration over Accuracy):**
   A sportfogadási piacok (különösen a Pinnacle és a betfair tőzsde záró oddsa, a *Closing Line*) rendkívül hatékonyak. A cél nem az, hogy a modell „okosabb legyen a piacnál” minden ligában, hanem:
   - Saját, független valószínűségi modell felállítása fundamentális adatokból (Bottom-up).
   - A piaci oddsok felhasználása mint rendkívül erős előzetes eloszlás (Bayesian Prior / Wisdom of Crowds).
   - A piac által hagyott rések (market inefficiencies) megtalálása kisebb/közepes ligákban, korai oddsoknál vagy speciális prop piacokon (szögletek, lapok).
   - Valószínűségi kalibráció: ha a modell 100 meccsre 70% esélyt ad, abból pontosan 70-nek kell bekövetkeznie.

---

## 2. Piac-specifikus Módszertan és Statisztikai Modellek

A labdarúgásban a különböző fogadási piacok alapvetően eltérő matematikai eloszlást és feltételes összefüggéseket követnek. Nem lehet ugyanazt a modellt ráerőltetni az 1X2-re, mint a szögletekre vagy a büntetőlapokra.

### 2.1. Gólpiacok: 1X2, Over/Under, Ázsiai Hendikep, BTTS (Mindkét csapat szerez gólt)

* **Eloszlási modell:** **Bivariate Poisson / Dixon-Coles modell Frank- vagy Clayton-Copulával**
  - A sima Poisson-eloszlás feltételezi, hogy a hazai és vendég gólok száma független egymástól. A valóságban ez nem igaz: a játék dinamikája (ha az egyik csapat vezet, a másik kockáztat; alacsony gólszámnál megváltozik a játéktempó) függőséget teremt.
  - **Dixon-Coles korrekció ($\tau$ paraméter):** Korrigálja az alacsony gólszámú kimenetelek (0-0, 1-0, 0-1, 1-1) torzítását:
    $$\tau(x, y; \lambda, \mu, \rho)$$
  - **BTTS (Both Teams To Score):**
    $$P(\text{BTTS Yes}) = 1 - P(G_H = 0) - P(G_A = 0) + P(G_H = 0, G_A = 0)$$
    A Bivariate/Copula modellből közvetlenül és pontosan számítható, elkerülve a függetlenségi feltételezés hibáit.
  - **Over/Under (pl. 2.5 gól):**
    $$P(\text{Over } 2.5) = \sum_{x+y \ge 3} P(G_H = x, G_A = y)$$
  - **Game State & Várható játéktempó:**
    A csapatok tempója megváltozik, ha vezetéshez vagy hátrányhoz jutnak (Game state bias). A modern xG modellek mérik, mekkora az xG termelés döntetlennél, vezetésnél és hátrányban.

### 2.2. Szögletpiacok (Total Corners, Corner Handicap, Race to X Corners)

* **Eloszlási modell:** **Negatív Binomiális (Negative Binomial) eloszlás vagy Conway-Maxwell-Poisson**
  - **Miért nem Poisson?** A szögleteknél erős **túlszóródás (overdispersion)** figyelhető meg: a variancia jelentősen nagyobb, mint a várható érték ($Var(X) > E[X]$). A szögletek gyakran „csomósodnak” (egy szöglet után blokkolt lövés, azonnal újabb szöglet).
* **Kulcsfontosságú vezérlő változók:**
  - **Támadási stílus:** Szélső játék dominanciája, beadások (crosses) száma 90 percenként, blokkolt lövések aránya.
  - **Védelmi struktúra:** Mélyen ülő védelem (low-block) sokkal több szögletet enged a domináns csapatnak, mint egy magas letámadás.
  - **Field Tilt és Területi dominancia:** Labdabirtoklás az ellenfél támadóharmadában.
  - **Game State hatás:** Ha a favorit hátrányba kerül a 60. percben, a szögletgyakorisága drasztikusan megugrik. A pre-match szögletmodell a favorit vezetési valószínűségének súlyozott átlagát használja.

### 2.3. Sárga lapok és Büntetőpontok (Cards & Booking Points)

* **Eloszlási modell:** **Poisson / Negatív Binomiális + Zero-Inflated Piros Lap valószínűség**
* **A hármas interakciós mátrix:**
  A lapok modellezése egyedi, mert 3 független entitás szorzataként alakul ki:
  $$\text{Expected Cards} \propto f(\text{Csapat agresszivitás}) \times g(\text{Ellenfél cselezési/szabálytalanság-kikényszerítési rátája}) \times h(\text{Játékvezető szigora})$$
* **Kulcsfontosságú tényezők:**
  1. **Játékvezetői faktor (Referee Bias & Strictness):**
     - Átlagos sárga lap/meccs és szabálytalanság/lap arány (Fouls per Card). Egyes bírók 25 szabálytalanságra adnak 2 lapot, mások 15-re 6-ot.
     - Hazai pálya bírói torzítása (Home referee bias: a közönség nyomása miatt a vendégcsapat statisztikailag több lapot kap azonos szabálytalanságszám mellett).
  2. **Derby / Rivalizálási és Tét faktor:**
     - Regionális rangadók, kiesési rangadók, kupa-visszavágók esetén a szabálytalanságok és lapok várható száma 20–35%-kal megnő.
  3. **Taktikai szabálytalanságok és kontratörések:**
     - Mennyit kontrázik az ellenfél? Ha egy gyorsan kontrázó csapat játszik egy magasan letámadó, de lassú védelmű csapattal, a taktikai sárgák száma törvényszerűen nő.

---

## 3. Piac, Odds-integráció és Value Detection (Értékkeresés)

A sportfogadásban nem az nyer, aki "eltalálja a meccseket", hanem aki **olyan kimenetelekre fogad, ahol a felkínált szorzó nagyobb, mint a kimenetel valódi valószínűségének reciproka**:
$$P_{\text{valódi}} > \frac{1}{\text{Odds}_{\text{piaci}}}$$

### 3.1. Az Odds mint Bemeneti Feature (Wisdom of Crowds)
A piac záró oddsa (Closing Line) a világ legnagyobb konszenzusos információsűrítménye. Hibát követ el az a rendszer, amely teljesen figyelmen kívül hagyja a piaci oddsot.
* **Bayesian Shrinkage (Zsugorítás a piachoz):**
  A modell saját fundamentális becslését ($P_{\text{model}}$) kombináljuk a piac marginmentesített valószínűségével ($P_{\text{market}}$):
  $$P_{\text{final}} = w \cdot P_{\text{model}} + (1 - w) \cdot P_{\text{market}}$$
  - **Likvid piacokon (Premier League, BL, 1X2):** $w$ kisebb (a piac nagyon hatékony, a modell csak apró korrekciókat tehet).
  - **Illikvid piacokon (kisebb ligák, szöglet, lapok, korai oddsok):** $w$ jóval nagyobb (a piac lassabb és pontatlanabb, a saját modell dominál).

### 3.2. Marginmentesítés (Overround eltávolítása)
A bukmékerek beépítik a hasznukat (pl. 105%-os kifizetési ráta). A valódi piaci valószínűség kinyeréséhez nem elég az egyszerű normalizálás, mert a bukmékerek nem arányosan terhelik a szorzókat (**Favorite-Longshot Bias**: a longshotokat aránytalanul jobban leárazzák).
* **Shin-módszer:** Feltételezi, hogy a piacon jelen van egy belső információval rendelkező réteg (insiders), és a bukméker ellenük védekezve alakítja ki a spreadet. Egzakt, zárt formulával vagy numerikus optimalizálással adja vissza a valós valószínűségeket.
* **Logit Margin módszer:** Logisztikai transzformációval igazítja a szorzókat, megszüntetve a favorit-underdog torzítást.

### 3.3. Expected Value (Várható Érték) és Edge
Egy adott $k$ kimenetelre:
$$\text{Edge} = (P_{\text{final}} \times \text{Odds}) - 1$$
* Ha $\text{Edge} > \text{küszöbérték}$ (pl. > 3% likvid ligákban, > 5% prop piacokon), a fogadás pozitív várható értékű (**+EV / Value Bet**).

### 3.4. Piacmozgások (Odds History & Steam Moves)
* **Opening vs. Current vs. Closing:**
  - Nyitó odds vs. Jelenlegi odds eltérése jelzi, merre mozdult az "okos pénz" (Smart Money / Syndicate betting).
  - Ha a modell korán jelez értéket, és utána a piac beomlik az adott irányba (Steam Move), az igazolja a modell minőségét még a meccs lefújása előtt.

---

## 4. Részletes Csapat- és Játékosstatisztikák

A pontos predikcióhoz elengedhetetlen, hogy mélyebbre ássunk a tabellánál és a lőtt/kapott góloknál.

### 4.1. Csapatstatisztikák (Taktikai és Teljesítmény-mutatók)
1. **Minőségi Helyzetmutatók:**
   - **npxG / npxGA:** Tizenegyesek nélküli várható gólok és kapott várható gólok 90 percenként.
   - **xG per Shot:** Helyzetek átlagos minősége (a csapat távolról lövöldöz, vagy tiszta ziccereket alakít ki a 16-oson belül).
   - **Post-Shot xG (PSxG) Differential:** A befejezés és a kapusvédések minőségének szétválasztása.
2. **Játékstílus és Tempó (Style of Play):**
   - **Directness Index:** Milyen sebességgel halad előre a labda az ellenfél kapuja felé méter/másodpercben (possession-alapú vs. direkt kontracsapat).
   - **Pass per Sequence:** Hány passzból áll egy tipikus támadásépítés.
   - **PPDA (Passes Per Defensive Action):** Mennyire agresszív a csapat labda elleni letámadása.
   - **Defensive Line Height:** Milyen magasan szerel és avatkozik közbe a csapat védelme (sérülékenység a gyors kiugratásokkal szemben).
3. **Zóna- és Területuralom:**
   - **Deep Completions:** Sikeres passzok a 16-oson belülre (kivéve beadások).
   - **Field Tilt %:** A támadóharmadban végrehajtott akciók aránya.
4. **Pontrúgások (Set-Pieces):**
   - Set-Piece xG termelés és engedett xG (kiemelten fontos a gólok ~25-30%-ában és a szöglet/lap piacon).

### 4.2. Játékosstatisztikák (Egyéni hozzájárulás és hiányzók súlyozása)
1. **Támadók és Középpályások:**
   - **npxG + xA per 90:** Gól- és gólpassz-várhatóság játékpercekre vetítve.
   - **Progressive Carries & Passes:** Mennyire képes egy játékos megbontani a védelmi vonalakat és területet nyerni.
   - **Fouls Drawn per 90:** Kikényszerített szabálytalanságok (lapok előrejelzésénél döntő).
2. **Védők és Védekező Középpályások:**
   - **Tackles + Interceptions per 90:** Védelmi közbeavatkozások.
   - **Aerial Duel Win %:** Fejpárbaj-hatékonyság (különösen szögleteknél és pontrúgásoknál).
   - **Fouls Committed & Card Rate per 90:** Lapérzékenység.
3. **Kapusok:**
   - **PSxG - Goals Conceded:** Védési hatékonyság a kapura tartó lövések minőségéhez viszonyítva. Egy világklasszis formában lévő kapus képes egymaga felülírni a várható gólokat meccseken keresztül.
4. **Hiányzók Kezelése (Lineup-Adjusted Team Strength):**
   - Amikor a kezdőcsapat hivatalossá válik (kickoff - 60 perc): a csapat alaperősségét az adott 11 játékos egyéni ratingje (RAPM / Plus-Minus) alapján azonnal újraszámoljuk.

### 4.3. Játékvezetői (Bírói) Profil
- Név, tapasztalat, büntetési stílus.
- Átlagos sárga lap / meccs, piros lap gyakoriság, megítélt tizenegyesek aránya (Penalties awarded per 90).
- Statisztikai Z-score a ligájának átlagához képest (pl. +1.4 szórásnyival szigorúbb az átlagnál).

---

## 5. Szükséges Adatok és Források Kategorizálása

| Adatréteg | Tartalom és Metrikák | Szükségesség | Potenciális Források |
| :--- | :--- | :--- | :--- |
| **Alap meccsadatok (Meglévő v2)** | Eredmények, kezdési időpontok, kanonikus identitások, liga-táblázatok | **Kritikus (Megvan)** | Kanonikus DB, forrás-observációk |
| **Piac & Odds (Részben megvan)** | 1X2, AH, O/U, BTTS nyitó, jelenlegi és záró oddsok, mozgástörténet | **Kritikus** | Betman / Tipplap, Football-Data.co.uk, OddsPortal, Pinnacle API |
| **Részletes meccsstatisztikák** | Lövések (kapura, mellé, blokkolt), szögletek, szabálytalanságok, lapok | **Kritikus** | Football-Data.co.uk archívum, SofaScore / FlashScore API |
| **Haladó eseményadatok (xG)** | npxG, xGA, xA, lövéstérképek, PPDA, passzhálózat | **Erősen Ajánlott** | Understat, FBref (StatsBomb feed), Wyscout / Opta |
| **Játékvezetői adatok** | Játékvezető azonosító, bírói lap- és büntetőstatisztikák | **Fontos (Lapok/Büntetők)** | Football-Data.co.uk, Transfermarkt, WorldReferee |
| **Felállások és Hiányzók** | Hivatalos kezdő 11, cserék, sérültek, eltiltottak, pozíciók | **Közép- és hosszútáv** | SofaScore, Transfermarkt, hivatalos ligafedések |
| **Kontextuális adatok (Meglévő)** | Időjárás (hőmérséklet, szélsebesség, csapadék), stadion koordináták | **Bónusz (Időjárás kész)** | v2 Open-Meteo cache |

---

## 6. Validációs és Teljesítménymérési Stratégia

1. **Brier Score és Log-Loss (Multi-class Cross-Entropy):**
   Méri a valószínűségi becslések pontosságát kimenetelenként.
2. **Kalibrációs Görbék (Reliability Diagrams):**
   Annak vizsgálata, hogy a prediktált 60%-os események valójában 60%-ban következnek-e be (U-alakú vagy S-alakú torzítások felderítése).
3. **Closing Line Value (CLV) Yield & Beat Rate:**
   $$CLV = \frac{\text{Odds}_{\text{vett}}}{\text{Odds}_{\text{záró}}} - 1$$
   Milyen arányban veri a modell a záró szorzót? Ha a CLV Beat Rate > 55-60%, a modell elméletileg hosszú távon profitábilis.
4. **Frakcionális Kelly Szimuláció (Paper Portfolio):**
   Virtuális tétrakási szimuláció as-of adatokon a maximális visszaesés (Max Drawdown) és a Sharpe-ráta követésére.

---

## 7. Integráció a Football App v2 Architektúrájába

A v2 szigorú elveinek megfelelően:

1. **`db/migrations` (Adatbázis sémabővítés):**
   - `analytics.match_features`: as-of előszámolt feature vektorok (rolling xG, stílus-mutatók, pihenőidő, bírói mutatók).
   - `analytics.market_lines`: nyitó, mozgás és záró oddsok, számított implicit piaci valószínűségek (Shin-módszerrel).
   - `analytics.predictions`: Verziózott előrejelzések piacokra bontva (1X2, O/U, BTTS, szöglet, lap), valószínűségek, fair odds, becsült Edge %.
2. **`rust/crates/prediction` (Új crate a workspace-ben):**
   - **Math & Statistical Solvers:** Dixon-Coles Bivariate Poisson solver, Negative Binomial corner-engine, Shin-margin stripper (tiszta, gyors Rust kód).
   - **Feature Builder:** As-of SQL aggregációk, rolling window számítások.
   - **Inference Engine:** Gradiensfa modellek kiértékelése (ONNX Runtime vagy `tract`/`smartcore`).
3. **`rust/crates/api` (Predikciós végpontok):**
   - `GET /v1/matches/{id}/prediction`: Teljes piaci bontás (1X2, gólok, szögletek, lapok), valószínűségi eloszlás, fair odds vs piaci odds, és value jelzések.
4. **`apps/FootballApp` (macOS SwiftUI UI megjelenítés):**
   - Meccs-részletező nézet bővítése:
     - **Odds & Value Kártya:** Piaci odds vs Fair odds kiemelés, Edge % badge-ek (+EV jelzés).
     - **Piacválasztó fülek:** Gólok (1X2, O/U, BTTS), Szögletek, Lapok / Büntetőpontok.
     - **Gólelrendezési Mátrix:** 0-0-tól 4-4-ig terjedő pontos eredmény hőtérkép.
     - **Bírói Kártya:** Játékvezetői szigorúság és várható lap-indikátor.

---

## 8. Javasolt Lépésenkénti Megvalósítási Ütemterv (Roadmap)

* **Fázis 1: Piac-mentesítés (Shin) & Alap Gólmodellek**
  - A v2 meglévő odds-tárának marginmentesítése (Shin-algoritmus a valós piaci valószínűségekhez).
  - Tiszta Dixon-Coles Poisson gólmodell (1X2, Over/Under, BTTS) implementálása Rustban.
  - Fair odds vs Piaci odds összehasonlító motor és Value Edge kalkulátor.
* **Fázis 2: Részletes Statisztikák (Lövések, Szögletek, Lapok) & Bírói Adatok**
  - Football-Data.co.uk meccsszintű statisztikáinak és játékvezetőinek bekötése.
  - Negatív Binomiális szögletmodell és bíró-súlyozott lapmodell kialakítása.
* **Fázis 3: xG & Eseményadatok Integrációja**
  - Nyílt xG források (Understat, FBref) illesztése a kanonikus meccsekhez.
  - xG-alapú csapat rating és rolling momentum metrikák.
* **Fázis 4: Kezdőcsapatok és Dinamikus Lineup-Adjusted Értékelés**
  - Meccs előtti (kickoff - 60 perc) kezdőcsapat-frissítések feldolgozása, kulcshiányzók miatti azonnali oddskorrekció.
* **Fázis 5: SwiftUI UI Felület és Virtuális Papír-Portfólió**
  - Teljes predikciós és piaci érték panel beépítése a macOS kliensbe.
  - Automatizált CLV és modellellenőrző háttérjobok.
