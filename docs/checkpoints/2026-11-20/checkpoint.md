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
