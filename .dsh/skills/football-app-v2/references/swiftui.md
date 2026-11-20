# SwiftUI kliens — projekt-specifikus konvenciók

## Design és kód
- A vizuális alap: `apps/FootballApp/Theme.swift` — OLED-fekete háttér (`#000000`),
  elsődleges szöveg `#F5F5F7`, másodlagos `#8E8E93`, accent `#FFB020`,
  live-zöld `#30D158`, warning `#FF453A`, surface `#0E0E10`/`#16161A`, hairline 8% fehér.
  Predikciós színek is definiálva vannak, de a predikciós fázis még nem indult.
- 56 Swift fájl, ebből 15 `*Tests.swift`; a `FootballAppCore` tesztjei 26/26 zöldek.
- Nézetek: `HomeView`, `DayModel`/`MatchListView`/`MatchRowView`, `MatchDetailSheet`,
  `CompetitionsView`/`CompetitionSheet`, `TeamsView`/`TeamSheet`, `OperationsView`,
  `EmptyStateView`/`ErrorStateView`, `StatusBadge`. Az API réteg `DayService`/`HTTPDayService`,
  stubbing `StubURLProtocol`.
- A kliens nem ér adatbázist: minden művelet tokenes `127.0.0.1` API-n megy át.

## A spec által megkívánt megjelenés (§3.4–3.7)
- Napi lista: dátum, versenycsoport, csapatok, kickoff, állapot, eredmény, félidő, adatminőség, forrásfrissesség;
  rejtett nulla nincs. Meccsoldal: fejléc + Összefoglaló/Statisztika/Felállások/Forma-H2H/Elérhetőség/Odds/Adat.
- Adatőszinteségi címkék: `Megfigyelt`, `Számított`, `Hiányzó`, `Elavult`, `Konfliktusos`, `Karanténban` —
  a szín nem egyedüli hordozó (szöveg + ikon + tooltip kell).
- Világos/sötét mód, reduced motion, WCAG 2.1 AA, teljes billentyűzetes navigáció, VoiceOver-címkék,
  magyar `hu-HU` formázás, betöltés/üres/hiba/daemon-offline külön állapot.

## Ismert rések (2026-11-20, mérve)
- 2026-11-20 óta a VoiceOver-címkék közös, tesztelt forrásból jönnek: `FootballAppCore/AccessibilityLabels.swift`
  (`matchRowLabel`, `matchCardLabel`, `publishLabel`, `statusLabel`, `matchIdentifier`), 6 unit teszttel.
  A napi lista kártyája (`MatchItemRow`), a meccs-sor, a státuszbadge, a liga-fejléc és az üres állapot
  címkézett; Cmd+←/→/T/R billentyű-parancsok bevezetve. **A tényleges VoiceOver-felolvasás és Tab-ciklus
  emberi ellenőrzése még nincs rögzítve** — lásd a repo `docs/operations/ui-accessibility-checklist.md`-jét.
- Nincs rögzített manuális macOS UI-, billentyűzet- és VoiceOver-ellenőrzés a checkpointokban.
- Nincs OpenAPI↔Swift DTO kontrakt-teszt (a spec §11.1 követelménye).
- A keresés és a teljes játékosoldal még nincs kész (a spec első mérföldkő nézetei közül).

## Következő UI-szelet javaslat
1. VoiceOver-címkék + accessibilityIdentifier a listára, meccs-sorra, státuszbadge-re (teszttel).
2. Billentyűzetes navigáció (FocusState, gyors szűrők) és reduced-motion tisztelete.
3. Manuális ellenőrzési jegyzőkönyv: világos/sötét, VoiceOver, Tab-ciklus, üres/hiba/offline állapot —
   futtatott parancs/screenshot + magyar szövegek visszaolvasása a checkpointba.
4. OpenAPI-ból generált vagy kézzel karbantartott DTO-kontrakt teszt.
