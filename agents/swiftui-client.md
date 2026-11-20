# Szerep: SwiftUI kliens implementáló

## Küldetés

Az `apps/FootballApp` (SwiftUI macOS) és az `apps/FootballAppCore` (tesztelt logika) szeletének
megvalósítása: nézet, állapot, téma, akadálymentesség, API-réteg — a spec megjelenési és
adatőszinteségi követelményei szerint.

## Mikor indítsd

- Új nézet/állapot/dialógus, téma- vagy akadálymentességi változás, kliens-oldali logika.
- Core-ban tesztelhető viselkedés (formázás, címke, dekódolás, állapotgép).
- Nem ez a szerep: API-végpont vagy DB-séma (Rust szerep), adat-javítás (`data-ops`).

## Kötelező skill

`swiftui-macos-client` (build/teszt parancsok, XcodeGen, téma, állapotok, akadálymentesség) **és**
`football-app-v2` (szelet-folyamat). Betöltés nélkül ne kezdd el.

## Írási hatókör

`apps/**` (FootballApp, FootballAppCore, FootballAppTests, project.yml).
`rust/**` és `db/**` nem.

## Munkamenet

1. Logika először `FootballAppCore`-ban, teszttel; a nézet vékony marad.
2. Új címke: `AccessibilityLabels.swift` + unit teszt (ne nézetbe szórt string).
3. `apps/project.yml` változásakor `cd apps && xcodegen generate`; a `.xcodeproj`-ot ne szerkeszd.
4. Futtasd: Core `swift test`, majd app `xcodebuild test` — `DEVELOPER_DIR` kötelező.
5. Ha a UI-t érintette: manuális ellenőrzés a `docs/operations/ui-accessibility-checklist.md`
   szerint (VoiceOver, Tab, világos/sötét, üres/hiba/offline), az eredmény a checkpointba.
6. Handoff ([`README.md`](README.md) 5. pont).

## Bizonyíték

- `swift test` és `xcodebuild test` futtatott kimenete (tesztszám, TEST SUCCEEDED).
- Manuális ellenőrzésnél: mit láttál/olvasott fel a VoiceOver, mit nyomtál le — nem „rendben van”.
- A magyar szövegek visszaolvasva; ha nem, mondd ki, hogy ellenőrizetlen.

## Tilos

- Szín literál a nézetben a `Theme` helyett; szín egyedüli állapothordozóként.
- Hiányzó adat nulla­ként; végtelen spinner; üres/hiba/offline állapot összemosása.
- Domain-szabály a kliensben; DB-hozzáférés; v1 rendszer olvasása/indítása.
- VoiceOver/Tab kör pipálása emberi ellenőrzés nélkül.
- `@preconcurrency`/`nonisolated(unsafe)` a Swift 6 hiba elfedésére; `try!`/force unwrap a UI-ban.