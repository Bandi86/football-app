---
name: swiftui-macos-client
description: "Use when changing, testing, or reviewing the SwiftUI macOS client of football-app-v2 (apps/), running Core or Xcode tests, or touching theme, accessibility and UI state behaviour."
whenToUse: "Any work under apps/, xcodebuild/swift test runs, VoiceOver/keyboard/theme changes, or a UI slice of the football-app-v2 plan."
---

# SwiftUI macOS kliens — football-app-v2

A kliens natív SwiftUI macOS app (`apps/FootballApp`) egy tesztelt SwiftPM mag fölött
(`apps/FootballAppCore`). Minden adat tokenes `127.0.0.1` API-n jön; a kliens nem ér adatbázist,
nem tartalmaz domain-szabályt és nem indít/olvas semmit a v1 rendszerből.

Ez a skill a **kliens-szelet végrehajtásának** menetét és a mérési pontjait hordozza. A projekt
teljes munkafolyamata (szeletek, kapuk, checkpoint) a `football-app-v2` skillben van — azt is töltsd be.

## 1. Ellenőrzött build- és tesztparancsok

Ez a gép Xcode 27.2-t futtat, de az aktív developer dir a CommandLineTools, ezért **minden**
Xcode-parancs elé kell a `DEVELOPER_DIR`:

```bash
export DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer
```

Enélkül a hiba szó szerint:
`xcode-select: error: tool 'xcodebuild' requires Xcode, but active developer directory '/Library/Developer/CommandLineTools' is a command line tools instance`.

Core (SwiftPM, gyors, ezt futtasd először):

```bash
cd apps/FootballAppCore && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test
```

App (Xcode scheme — csak a `FootballAppTests`-t futtatja, a Core-t NEM):

```bash
cd /Users/susleczandras/Documents/Code/football-app-v2
DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer \
  xcodebuild test -project apps/FootballApp.xcodeproj -scheme FootballApp \
  -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```

Szelet-kapu: mindkettő zöld + a manuális ellenőrzés rögzítve. A `swift test --filter <Suite>`
egy suite-ra szűkít (pl. `--filter AccessibilityLabelsTests`).

## 2. Projekt és generálás

| Tény | Érték |
|---|---|
| XcodeGen forrás | `apps/project.yml` — ez az igazság |
| Generált, NEM commitolt | `apps/FootballApp.xcodeproj/` (`.gitignore`), `apps/FootballApp/Info.plist` |
| Deployment target | macOS 14.0 |
| Swift nyelvi mód | `SWIFT_VERSION: "6.0"` (strict concurrency) |
| App target | `apps/FootballApp/**` |
| Teszt target | `apps/FootballAppTests/**` |
| Core | SwiftPM package: `apps/FootballAppCore` (a project.yml `packages:` alatt) |
| Bundle id | `hu.bandi86.FootballApp` |

`apps/project.yml` módosítása után: `cd apps && xcodegen generate`. **Soha ne szerkeszd kézzel a
`.xcodeproj`-ot** — a következő generálás felülírja. A Core saját `Package.swift`-ja külön él;
új Core-fájlhoz nem kell XcodeGen, új app-fájlhoz viszont igen (target sources mappa, ezért a legtöbb
esetben automatikus).

## 3. Téma (egyetlen forrás: `apps/FootballApp/Theme.swift`)

OLED-fekete alap kötelező: háttér `#000000`, elsődleges szöveg `#F5F5F7`, másodlagos `#8E8E93`,
accent `#FFB020`, live `#30D158`, warning `#FF453A`, surface `#0E0E10`, elevated `#16161A`,
hairline fehér 8%, predikciós 1X2: `#34C759`/`#8E8E93`/`#5E9EFF`.

- Minden szín a `Theme`-ből jön; nézetben nincs literál `Color(...)`/hex.
- A szín **soha nem egyedüli hordozó**: mellé szöveg + ikon/SF Symbol + tooltip kell.
- Világos/sötét mód és reduced motion tisztelete kötelező; a tiszta fekete háttér sötét módban marad.

## 4. Nézet- és rétegkonvenciók

- Napi lista: `DayModel` (állapot) + `MatchListView` (`MatchItemRow`/`MatchRowView`) + `StatusBadge`.
- Részletek: `MatchDetailSheet`; versenyek: `CompetitionsView`/`CompetitionSheet`; csapatok:
  `TeamsView`/`TeamSheet`; üzemeltetés: `OperationsView`; állapotok: `EmptyStateView`,
  `ErrorStateView`.
- Hálózat **csak** service-protokollon át: `DayService`/`HTTPDayService`, `CompetitionService`,
  `TeamService`, `MatchDetailService`, `OpsService`; DTO dekódolás `APIDecoding.swift`,
  hiba `APIError.swift`. Tesztben `StubURLProtocol` stubol, nem valódi hálózat.
- A nézet ne végezzen üzleti számítást: a számított/lezárt érték a szerverről jön, a kliens formáz.
- Minden új logika a `FootballAppCore`-ba kerül, ahol `swift test` méri — a nézet vékony marad.

## 5. Kötelező UI-állapotok

Minden adatképernyő **megkülönbözteti**: betöltés, üres, hiba, daemon-offline, részleges adat —
végtelen spinnert tilos hagyni. Adatőszinteségi címkék: `Megfigyelt`, `Számított`, `Hiányzó`,
`Elavult`, `Konfliktusos`, `Karanténban`. Hiányzó adat nem jelenik meg nullaként; magyar `hu-HU`
formázás (dátum, szám, odds) és a magyar szövegek visszaolvasása a szelet végén.

## 6. Akadálymentesség

- Címkék egyetlen, tesztelt forrásból: `apps/FootballAppCore/Sources/FootballAppCore/AccessibilityLabels.swift`
  (`matchRowLabel`, `matchCardLabel`, `publishLabel`, `statusLabel`, `matchIdentifier`). Új interaktív
  elemhez ide kerül a címke + unit teszt, ne a nézetbe szórt string.
- Minden interaktív elem kap `accessibilityLabel`-t és stabil `accessibilityIdentifier`-t.
- Billentyűzet: teljes Tab-ciklus, dialógusban focus trap, gyorsparancsok `Cmd+←/→/T/R`.
- A gép által futtatható rész (tesztek) nem bizonyítja a felolvasást: a VoiceOver/Tab/világos-sötét
  kör emberi, és csak akkor pipálható, ha a `docs/operations/ui-accessibility-checklist.md` szerint
  lefutott és a checkpointba bekerült a futtatott lépés + eredmény.

## 7. Swift 6 konvenciók

- View model / UI állapot `@MainActor`; hálózati réteg `async` és `Sendable` DTO-kkal.
- Új osztálynál `@Observable` (macOS 14+) a `ObservableObject` helyett, ha a nézet csak olvassa.
- `Task`/`.task` a nézetben, `onAppear`-ban ne indíts duplikált betöltést; megszakítást kezeld.
- Ne használj `@preconcurrency`/`nonisolated(unsafe)` szűkítést a fordítási hiba elfedésére — a
  strict concurrency hiba valódi adatverseny-jelzés.
- Force unwrap / `try!` a UI-kódban tilos; hiba → `ErrorStateView` értelmes magyar üzenettel.

## 8. Szelet-lezárás bizonyítékai

1. `swift test` (Core) és `xcodebuild test` (app) futtatott kimenete, számmal.
2. Új/ módosított címke esetén a hozzá tartozó unit teszt neve.
3. Manuális ellenőrzés (ha UI-t érint): indított app, mit láttál, mit olvasott fel a VoiceOver,
   Tab-ciklus, világos/sötét, üres/hiba/offline állapot — rögzítve a checkpointban.
4. Ha a kapu nem futott le, a szelet nem „kész”: mondd ki, mi maradt ellenőrizetlen.

## 9. Csapdák

- `xcodebuild` `DEVELOPER_DIR` nélkül elbukik (lásd 1. pont).
- A scheme **nem** futtatja a Core teszteket → a két parancs külön kell.
- A `.xcodeproj` és az `Info.plist` generált: kézi szerkesztés elveszik.
- A `CODE_SIGNING_ALLOWED=NO` nélkül a teszt-futtatás aláírás miatt akadhat el CI-szerű környezetben.
- A kliens soha nem kap DB-hozzáférést és nem tartalmaz domain-szabályt (az `api` crate kivétel:
  ott is tilos üzleti szabály a handlerben).