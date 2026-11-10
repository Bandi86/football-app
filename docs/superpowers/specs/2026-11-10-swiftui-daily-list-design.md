# SwiftUI „Ma" kliens — Design

**Dátum:** 2026-11-10
**Állapot:** jóváhagyott design (a megvalósítási terv külön dokumentum)
**Kapcsolódó terv:** [SwiftUI „Ma" kliens — megvalósítási terv](../plans/2026-11-10-swiftui-daily-list.md)
**Kapcsolódó spec:** [Football App v2 rendszertervezés](2026-11-09-football-app-v2-design.md)

## 1. Cél

A macOS SwiftUI kliens első szelete: a futó `footballd` helyi API-ról megjelenít egy „Ma" képernyőt, amely a kiválasztott nap ellenőrzött meccslistáját mutatja. A kliens kizárólag olvas: nem indít daemont, nem ír adatbázist, nem hív ops-végpontot, és nem használ valós forrásadatot.

Ez a szelet a terv „Programterv bontása" (#2 SwiftUI kliens) első, önállóan szállítható darabja, a spec Phase 2 (meccsközpont) belépője.

## 2. Hatókör

- **Dátumválasztó**: alapból a mai nap (a felhasználó helyi, `Europe/Budapest` szerinti dátuma); a dátum módosítható.
- **Meccslista**: a kiválasztott nap meccsei a `GET /v1/day/{yyyy-MM-dd}` végpontról (alapértelmezett, csak `verified_publish`).
- **Soronként**: hazai–vendég csapatnév, kickoff helyi időben, eredmény (ha van) vagy státusz, publikálási állapot badge.
- **Állapotok**: betöltő, üres („Nincs meccs ezen a napon."), hiba (magyar üzenet + „Újra" gomb).
- **Kapcsolat**: az API portja és a read token a futásidejű `runtime.json`-ból.

## 3. Nem része (kizárások)

- meccsrészlet-, verseny-, csapat-, játékosoldal, keresés, ops nézetek, live;
- predikció, papírportfólió, ügynökök;
- írási műveletek, a daemon indítása/leállítása a kliensből;
- valós forrásadatok (a kliens csak azt mutatja, amit a daemon szolgál);
- az ops (`/v1/ops/verification/gates`) végpont használata — ebben a szeletben a kliens csak a read tokent használja a napi listához.

## 4. Architektúra

Két egység, tiszta felelősséggel, a mag UI- és Xcode-mentes, így parancssorból (`swift test`) tesztelhető.

### 4.1 `apps/FootballAppCore/` — SwiftPM library (Xcode nélkül tesztelhető)

| Komponens | Felelősség |
|---|---|
| `RuntimeInfo` (Codable) | `port` (Int), `read_token` (String); opcionálisan `ops_token` (ez a szelet nem használja). |
| `RuntimeInfoLoader` | A `~/Library/Application Support/FootballAppV2/runtime.json` beolvasása és dekódolása; hiányzó fájl → `APIError.runtimeMissing`. Injektálható útvonal a teszthez. |
| `DayResponse` (Decodable) | `date` (String), `timezone` (String), `matches` ([`DayMatch`]). |
| `DayMatch` (Decodable) | `id` (UUID/String), `kickoff_at` (ISO-8601 String), `status` (String), `home` (String), `away` (String), `home_score` (Int?), `away_score` (Int?), `publish_state` (String), `blockers` ([`Blocker`]). |
| `Blocker` (Decodable) | `rule_key` (String), `severity` (String), `field` (String?), `message` (String). |
| `PublishState` (enum) | `verifiedPublish`, `provisionalPublish`, `withheld`, `quarantined`, `unknown`; a JSON string → enum leképezés, ismeretlen → `.unknown`. |
| `DayService` (protocol) | `func day(for date: Date) async throws -> [DayMatch]`. |
| `HTTPDayService` | `DayService` implementáció: async/await `URLSession`, `GET {base}/v1/day/{yyyy-MM-dd}`, `Authorization: Bearer <read_token>`, JSON-dekódolás. Injektálható session és dátumformázó. |
| `APIError` (enum) | `runtimeMissing`, `unauthorized`, `badRequest`, `server`, `network`, `decoding`; magyar, felhasználónak szóló üzenettel. |

**Dekódolási szabályok:** snake_case kulcsok (`convertFromSnakeCase` vagy explicit `CodingKeys`); `kickoff_at` ISO-8601 String → `Date` (frakcionális másodpereceket is elfogadva); a hiányzó eredmény `null` → `nil` (soha nem 0).

### 4.2 `apps/FootballApp/` — Xcode app (`.app`, vékony SwiftUI)

| Komponens | Felelősség |
|---|---|
| `FootballAppApp` (`@main`) | App-belépés, a `DayModel` létrehozása. |
| `DayModel` (`@Observable`, `@MainActor`) | `selectedDate` (alapból ma), `state`: `idle`/`loading`/`loaded([DayMatch])`/`empty`/`error(APIError)`; `load()` async. A `DayService`-t injektálva kapja. |
| `DayView` | `DatePicker` + lista; állapot szerinti megjelenítés; dátumváltás → újratöltés. |
| `MatchRowView` | Hazai–vendég, kickoff helyi időben, eredmény vagy státusz, publikálási badge. |
| `StatusBadge` | A `PublishState` vizuális jelzése. |
| `ErrorStateView` | Magyar hibaüzenet + „Újra" gomb. |
| `EmptyStateView` | „Nincs meccs ezen a napon." |

### 4.3 `apps/project.yml` — XcodeGen

- `FootballApp` macOS app target, függőség a helyi `FootballAppCore` SwiftPM csomagra.
- Bundle azonosító: `hu.bandi86.FootballApp`.
- Deployment target: **macOS 14.0** (az `@Observable`/Observation minimuma).
- `Info.plist`: `NSAppTransportSecurity → NSAllowsLocalNetworking = YES` (a `http://127.0.0.1` engedélyezéséhez); `LSMinimumSystemVersion` a deployment target.
- **App Sandbox: kikapcsolva** (helyi fejlesztői app, nem App Store). A kliens a daemon `runtime.json`-ját a megosztott `Application Support` helyről olvassa; sandbox esetén ezt külön (group container / bookmark) kellene kezelni, ami ebben a szeletben nem cél.
- A `.xcodeproj` generált (`xcodegen generate`), nem kézzel írt.

## 5. Adatfolyam

1. App-indítás → `DayModel.load()`.
2. `RuntimeInfoLoader` beolvassa a `runtime.json`-t → `RuntimeInfo` (port, read token). Hiba → `state = .error(.runtimeMissing)`.
3. `HTTPDayService.day(for: selectedDate)` → `GET http://127.0.0.1:{port}/v1/day/{yyyy-MM-dd}` Bearer read tokennel → `DayResponse`.
4. `matches` üres → `.empty`; egyébként `.loaded(matches)`.
5. Hiba → `.error(APIError)`; a „Újra" gomb újrahívja a `load()`-ot.
6. Dátumváltás → `load()` az új dátumra.

A daemon külön folyamat; a kliens nem indítja el. Ha a `runtime.json` hiányzik vagy a kapcsolat megszakad, a hibaállapot egyértelműen jelzi, hogy a daemon indítása szükséges.

## 6. Hibakezelés

| Eset | `APIError` | Magyar üzenet |
|---|---|---|
| `runtime.json` hiányzik / nem olvasható | `runtimeMissing` | „A footballd daemon nem fut (runtime.json nem található). Indítsd el a daemont." |
| HTTP 401 | `unauthorized` | „Érvénytelen vagy hiányzó hozzáférési token." |
| HTTP 400 | `badRequest` | „Hibás dátum." |
| HTTP 5xx | `server` | „A szerver hibát adott. Próbáld újra." |
| Hálózati hiba | `network` | „Nem érhető el a helyi API. Fut a daemon?" |
| Értelmezhetetlen válasz | `decoding` | „Váratlan válaszformátum." |
| Üres lista | (nem hiba) | „Nincs meccs ezen a napon." |

A 400 a `DatePicker` miatt gyakorlatilag nem fordul elő, de a teljesség kedvéért kezelt.

## 7. Tesztelés és verifikáció

### 7.1 Core — parancssorból (`swift test`, Xcode nélkül)

- **DTO-dekódolás**: valós `day` válasz mintájára készült JSON fixture-ökből (`date`/`timezone`/`matches`, snake_case, ISO-8601 `kickoff_at`, `blockers`, `home_score: null`).
- **`RuntimeInfoLoader`**: ideiglenes `runtime.json` → parse; hiányzó fájl → `runtimeMissing`; hibás JSON → `decoding`.
- **`HTTPDayService`** injektált `URLSession`/stubbal:
  - helyes URL (`/v1/day/2026-11-09`) és `Authorization: Bearer <token>` fejléc;
  - 200 → `[DayMatch]`; 401 → `unauthorized`; 400 → `badRequest`; 500 → `server`; rossz JSON → `decoding`; hálózati hiba → `network`.
- A Core `Package.swift`-ban külön library + test target; `swift test` a CLI tools-szal fut.

### 7.2 App — Xcode után (`xcodebuild test`)

- `DayModel` állapotátmenetek stub `DayService`-szel (idle→loading→loaded/empty/error; „Újra").
- UI szemrevétel / screenshot Xcode-dal.

### 7.3 Verifikációs kapu

- **Most, Xcode nélkül**: a Core teljesen implementálható és `swift test`-tel bizonyítható.
- **Teljes Xcode telepítése után**: `xcodegen generate`, `xcodebuild test`, az app futtatása és vizuális ellenőrzése. Addig az app build/teszt/futtatás nem futtatható ezen a gépen.

## 8. Előfeltételek

- **Teljes Xcode** (az `.app` buildjéhez, `xcodebuild`-hez, SwiftUI-preview-höz) — a gépen jelenleg **nincs** telepítve, csak Command Line Tools.
- **XcodeGen** (`brew install xcodegen`) — a `project.yml`-ból `.xcodeproj`-ot generál.
- Swift 6 / SwiftPM (jelen van: Swift 6.4.2) — a Core `swift test`-jéhez elég a CLI tools is (a SwiftUI/AppKit framework a SDK-ban elérhető).

## 9. Biztonság és korlátok

- A kliens csak `127.0.0.1`-hez kapcsolódik; a tokeneket a `runtime.json`-ból olvassa, nem naplózza és nem tárolja tartósan.
- A read token csak a napi lista olvasásához kell; az ops tokent ez a szelet nem használja.
- A kliens nem ír adatbázist és nem indít daemont.
- App Sandbox kikapcsolva — helyi fejlesztői app; a repó továbbra is publikus, titok nem kerül bele.

## 10. Megállási pont

A design és a hozzá tartozó megvalósítási terv elkészül. A **Core implementációja** külön jóváhagyással indul és `swift test`-tel bizonyítható. Az **app (UI) implementációja és verifikációja** csak teljes Xcode telepítése után lehetséges — addig a terv kész, de az app-réteg nem futtatható.
