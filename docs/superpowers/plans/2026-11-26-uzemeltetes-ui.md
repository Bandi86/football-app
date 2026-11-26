# 2026-11-26 — App-oldalak bővítése, S2: Üzemeltetés — állapot, hiányjelek és idővonal

Spec: [`2026-11-25-app-oldalak-bovites-design.md`](../specs/2026-11-25-app-oldalak-bovites-design.md) §7.1.
Előzmény: S1 (`2026-11-26-app-oldalak-s1.md`, commit `1e41241`) — az 5 új API-végpont megvan.
Döntések: [`2026-11-26-app-oldalak-bovites.md`](../../decisions/2026-11-26-app-oldalak-bovites.md).
Branch: `main` (folyamatos felhatalmazás). **S2 után itt is megállok és jelzem.**

## Határ (mi most készül, mi nem)

Most: a meglévő `OperationsView` három szakasza — kattintható állapotkártyák,
szűrhető `ops.issue` lista, és issue-ból nyitott **meccs-szintű** idővonal
(`GET /v1/match/{id}/history`). Core DTO-k + kontrakt-teszt, magyar címkék,
`OpsModel` async állapot, UI-teszt fixture-szerverrel, valódi macOS AX-ellenőrzés.

**Nem most (külön döntés/szelet):** globális audit-feed (új `GET /v1/ops/audit`
végpont — a felhasználó 2026-11-26-on úgy döntött, hogy S3-ban jön); hiányok
`/v1/ops/summary` összefoglaló és javítási akciók (S3); jegyzetek panel a
meccsrészletezőben (S7); teljes meccs-szerkesztés (S4); logófeltöltés (S5);
AI-gomb (S8, alapból ki).

Az S1-ben a detektor még nem ír `ops.issue` sorokat — S2-ben ezért az üres
lista állapotot is külön kell megjeleníteni és tesztelni („Még nincs rögzített
hiányjel."), nem lehet üres képernyő.

## Szelet lépései

### S2a — Core DTO-k, formázás, akadálymentesség

- [ ] `apps/FootballAppCore/Sources/FootballAppCore/OpsIssueModels.swift` (új):
    `IssueRowDTO` (`id`, `subjectKind`, `subjectId`, `code`, `severity`,
    `detail`, `firstSeenAt`, `lastSeenAt`, `resolvedAt?`, `source`),
    `IssuesResponseDTO`, `MatchNoteDTO`, `HistoryEntryDTO`,
    `HistoryResponseDTO`. Konvenció: `Decodable, Equatable, Sendable`,
    internal `CodingKeys`, ismeretlen enum-érték → `.unknown`/`.other(raw)`
    (mint `PublishState`), dátumok `APIDecoding.makeDecoder()`-rel.
- [ ] `OpsFormatting.swift` (új, Core): magyar címke hiány-kódhoz
    (`missing_odds` → „Hiányzó odds"), súlyossághoz (`info` → „Tájékoztatás",
    `warning` → „Figyelmeztetés", `hard` → „Súlyos"), forráshoz
    (`daily-check` → „Napi ellenőrzés"), tárgyfajtához; hu_HU idő/dátum
    formátum az első/utolsó láttatáshoz. A szín itt sem egyedüli hordozó.
- [ ] `AccessibilityLabels.swift`: `issueLabel`, `issueIdentifier`,
    `issueSeverityLabel`, `historyEntryLabel`, `historyEntryIdentifier`,
    `issuesEmptyLabel/Identifier`, `issuesFilterIdentifier` — egyetlen
    tesztelt forrás; új unit tesztek `AccessibilityLabelsTests.swift`-ben.
- [ ] `IssuesContractTests.swift` (új): a közös
    `tests/fixtures/contract/issues-response.json` dekódolása és a kulcsok
    ellenőrzése (`#filePath`-ból induló keresési minta, mint `DayContractTests`).
- [ ] `OpsModelsTests.swift`: `IssuesResponseDTO`/`HistoryResponseDTO` inline
    JSON dekódolása, ismeretlen enum nem dob.

### S2b — Service és állapot

- [ ] `OpsService` protokoll bővítése: `issues(scope:date:code:severity:openOnly:)`
    és `history(matchID:limit:)`; `HTTPOpsService` URLQueryItem-ekkel,
    `APIDecoding.makeDecoder()`, meglévő `CancellationError`/`APIError`
    leképezéssel. (Az app a runtime ops tokenjét használja — az ops token a
    read scope-ot is kielégíti.)
- [ ] `OpsModel` bővítése: `issuesState` (idle/loading/loaded/error),
    `issuesFilter` (`scope`, `date`, `code`, `severity`, `openOnly`),
    `selectedIssue`, `issueHistoryState`, `loadIssues()`, `openIssue()`,
    generációs stale-result védelemmel és `CancellationError`-kezeléssel,
    ahogy a meglévő `load()`; a 30 mp-es `refreshLoop` az issues-t is frissíti.
- [ ] App unit teszt `OpsModelTests.swift` (új): stub service-szel betöltött,
    üres, hibás és szűrt lista; kiválasztott issue history-ja; stale result
    elvetése.

### S2c — Nézet

- [ ] `OperationsView.swift`: a meglévő állapotkártyák kattinthatóak
    (`issue` típusú kártya → a hiánylista ugyanarra a kódra szűrve); új
    **„Hiányjelek"** szekció: keresés/szűrő (kód, súlyosság, `openOnly`, nap),
    sorokon kód + magyar címke + súlyosság + első/utolsó láttatás + forrás +
    „megoldva" jelölés; üres/hiba/betöltés külön állapottal.
- [ ] Issue sorra kattintva (ha `subjectKind == "match"`) részletező sheet a
    `/v1/match/{id}/history` eseményeivel (origin, actor, action, before→after,
    forrás), stabil AX-azonosítókkal és magyar címkékkel; nem-meccs tárgynál
    „A részletes idővonal meccsekhez érhető el." magyar üzenet.
- [ ] A meglévő OLED-fekete téma és `AppCardStyle` használata; szín mellett
    szöveg + ikon; reduced motion; `EmptyStateView`/`ErrorStateView` újrahasználata.
- [ ] Magyar szövegek visszaolvasása a checkpointban.

### S2d — Kapu, telepítés, rögzítés

- [ ] `cd apps && xcodegen generate` (ha `project.yml` változott),
    `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer swift test --package-path apps/FootballAppCore`,
    `xcodebuild test -project apps/FootballApp.xcodeproj -scheme FootballApp
    -destination 'platform=macOS'` (csak `FootballAppTests`),
    `./scripts/run-ui-tests.sh` (**TEST SUCCEEDED**, új issues-tesztel),
    `./scripts/daily-check.sh 2026-11-25` (EXIT=0), `git diff --check`.
- [ ] **Daemon újratelepítése** (a felhasználó az S2 indításakor kifejezetten
      engedélyezte): `./scripts/install-footballd-agent.sh` közvetlenül,
      `launchctl print … | grep state|pid`, a log új `footballd elindult port=…`
      sora, `/v1/health` és egy `GET /v1/issues` kísérleti hívás a valódi
      porton. **A port minden induláskor változhat — a logból olvasom.**
- [ ] Kliens újratelepítése: `./scripts/install-football-app.sh` → BUILD
      SUCCEEDED; valódi AX ellenőrzés (`ax-press.swift … --list`,
      `app-window-shot.sh`) az Üzemeltetés fülön: kártyák, hiánylista (üres
      állapot), szűrő, issue-részletező.
- [ ] Checkpoint S169: mért kapuk, magyar szövegek, nem ellenőrzött részek,
      visszaállás. Commit a zöld kapu után. **S3-ba nem lépek tovább.**

## Kockázatok

- A `GET /v1/issues` jelenleg üres élesben (detektor S3) — a UI-nak ezt
  érthetően kell kimondania, nem lehet „semmi sem történt" képernyő.
- A régi daemon binárisa nem ismeri az új végpontokat: telepítés előtt a
  kliens 404/405-öt kaphat; a telepítés és a port-ellenőrzés kötelező.
- A `CODE_SIGNING_ALLOWED=NO` megöli a UI-test runnert; a fixture-szerver
  külön folyamat (sandbox `EPERM`), a `runtime.json`-t a runner írja.
- `SwiftUI Text` AX-értéke a `value`-ban van: UI-tesztben `value CONTAINS`.

## Visszaállás

- `git revert <S2 commit>`; a Core/app új fájljai törlődnek, a meglévők
  visszaállnak. A daemon és az app binárisa az S1-es commitból újraépíthető
  és telepíthető (`install-footballd-agent.sh`, `install-football-app.sh`).
- Adatot S2 nem módosít; a migráció (0028) változatlan marad.
