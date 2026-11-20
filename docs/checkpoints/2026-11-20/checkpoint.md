# 2026-11-20 — napi checkpoint

## Induló állapot (read-only ellenőrzés)
- 255 749 meccs, publish_gate: 208 377 provisional / 47 372 verified, withheld 0.
- **555 függő review** (tegnap 0): 399 soccerway + 156 openfoot, 07:31–07:33 között,
  `no_exact_match` 548 / `fuzzy_candidate` 7. Integritás egyébként OK.
- launchd daemon fut (pid 962), napi jobok 16 done.

## S30a — review bootstrap
- [x] Triage: a 7 fuzzy jelöltből 3 egyértelmű alias (Almere City FC → Almere City,
      FC Den Bosch → Den Bosch, Roda JC Kerkrade → Roda JC) kézi `identity approve`;
      a tartalékcsapatok (SK Rapid II, Jong AZ Alkmaar, Sporting CP B, Shakhtar
      Donetsk) külön entitások maradtak, mert a jelölt nem azonos klub.
- [x] `identity approve-create-all` 536 + `--allow-unknown-country` 16 sor; pending 0.
      Összes csapat: 2 702. Az új csapatok főleg Indonézia, Ausztrália, Vietnam,
      Irán és Wales új ligáiból jöttek — valódi új entitások, nem mapping-hiba.
- [x] A mai 4 day-job újrafuttatása (budget módosítás nélkül): footballdata,
      openfoot, tipplap done; soccerway 3× timeout (45 s) → `cancelled`, auditálva
      (az eredeti 07:31-i fetch sikeres volt, további retry nem indokolt).
- [x] Végállapot: **255 829 meccs** (+80), ma 118 meccs, pending review 0,
      withheld 0, integritás OK.

## S30b — integritás az API-ban és az Üzemeltetés nézetben
- [x] `GET /v1/ops/status` most `integrity` blokkot ad: verdict (OK/FAIL),
      duplikátum kickoff/fixture, árva provenance, összevont entitásra mutató
      aktív ref, pending review, withheld gate, failed job, nyitott breaker.
      Read-only, a meglévő `IntegrityReport` alapján.
- [x] Swift `OpsIntegrity` modell + Üzemeltetés nézet integritás-kártya
      (verdict színnel, 4 fő számláló); régi API-válasz esetén opcionális.
- [x] Tesztek: API `ops_status` 9/9 (új integritás-assertel),
      `FootballAppCore` 26/26. Kapu-lánc: db-reset → check-foundation →
      `cargo test --workspace -- --test-threads=1` → clippy `-D warnings` →
      fmt check → build `--features obscura` → db-reset: **EXIT 0**, 118 zöld
      tesztcsoport, 0 hiba.

## Nyitott / döntési pont
- A gateway blokkolta az `install-footballd-agent.sh` közvetlen futtatását
  („cannot restart … the gateway from inside the gateway process”), ezért a futó
  daemon még a régi binárist használja: az `integrity` mező a élő API-ban csak
  a következő kézi agent-telepítés után jelenik meg. A blokkot nem kerültem meg.
- Következő szelet: soccerway JS dayPicker (múltbeli nap fetch) vagy teljes
  (top-300-nál szélesebb) coverage-riport.

## S30c — 2006–2026 os gap-fill + eredményrevíziók

- [x] A `coverage --limit 5 / --competition` feltárta, hogy a korábbi gap-fill
  csak 1993–2005-öt pótolt: a v1-ben 2006–2026-ra ~40 ezer meccs volt, a v2-ben
  szinte semmi (pl. NB I 2020+ 3 meccs). A CLI most `--limit` (alap 300) és
  `--competition` szűrőt kapott, így a top-300 riport nem rejti el a réseket.
- [x] 11 verseny 2006–2026-os importja két körben (`--target-competition`-
  vel): NB I, Eliteserien, Ekstraklasa, Superliga, 2. Liga, Persha Liga,
  2. Bundesliga, Süper Lig, Primeira Liga, Serie B, Scottish Premiership.
  110 új csapat jóváhagyva (`approve-create-all`), 2 tartalékcsapat külön
  entitás maradt. Nettó **+31 835 meccs**, összesen **287 664**.
- [x] `dedupe fixtures`: 2 851 biztonságos merge + **32 kockázatos pár**,
  mindegyiket 4 párhuzamos kutató 2-2 forrással ellenőrizte — mind a 32-ben a
  **v1 eredmény volt a helyes**, az FD sor tévesen volt hozzárendelve. A
  kanonikus eredményt auditált revízióval javítottam, így a párok biztonságosan
  merge-elve lettek.
- [x] A reverify **115 withheld gate**-et talált (113 hiányzó eredmény, 4
  natural-key duplikátum, 2 anyagi konfliktus). 5 párhuzamos kutató ~9 perc
  alatt mind a 115-öt két forrással ellenőrizte: techinikai/walkower eredmények
  (Ankaraspor 34× 0-3, Obolon/Stal Alchevsk/Persha, FC Sopron, Reus Deportiu,
  Brescia, Salernitana, Magdeburg), 2 elhalasztott meccs (Chernihiv–Ahrobiznes,
  Hibernian–Dundee) `postponed`, 2 fordított hazai/vendég pár javítva.
  Minden módosítás `ops.audit_event`-tel rögzítve.
- [x] 266 konfliktus `resolved_manual` (web-igazolt vagy elavult, kanonikus
  érték megmaradt): 0 feloldatlan. A 43+43 orientációs konfliktus álhiba volt
  (a FD slug és a v1 `v1t-…` ID ugyanarra a kanonikus csapatra képez).
- [x] Végső kapu: `reverify 2006–2027` → **193 658 meccs, 0 hiba**;
  `integrity` → **0 duplikátum, 0 árva, 0 review, 0 withheld, 0 failed job,
  0 breaker, verdict OK**. `coverage 2006–2026 --limit 2000` → **189 310
  meccs, 96,7% eredmény, 86,1% odds**.
- [x] A soccerway bootstrap-timeoutokból nyitott breaker lezárva audittal
  (a job `cancelled`, további retry nem indokolt).
- Nyitott: a régi daemon-bináris miatt az API `integrity` mezője csak kézi
  agent-telepítés után él (lásd S30b); soccerway JS dayPicker; asset-licence.
