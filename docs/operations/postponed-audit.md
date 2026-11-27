# Elhalasztott meccsek — adatjelentés (2026-11-27)

A kliens S212 óta külön `Elhalasztott` szűrőt kínál; az elhalasztott meccs
nem esik bele sem a `Következik`, sem a `Lejárt, nyitott` szűrőbe, és a
kártyán `ELHALASZTOTT` felirat jelenik meg a kezdési idő helyett.

## Mért adat (dev DB, read-only)

- 2026 novemberben **107** `postponed` státuszú meccs van rögzítve; ebből
  **100** múltbeli kezdésű és **7** mai (2026-11-27) kezdésű.
- A legnagyobb egy-napi csoportok: 11-21 (31), 11-22 (18), 11-14 (13),
  11-04 (7), 11-27 (7), 11-19 (6), 11-07 (5), 11-20 (5).
- 11-26-on 2 elhalasztott meccs: Assabah SC–Al-Ahli Tripoli (Premier League,
  LY) és Cavalier SC–Portmore United (Premier League, JM).
- 11-27-en 7 elhalasztott meccs: Chemnitzer FC–Hertha Berlin II, DJK Don Bosco
  Bamberg–SV Fortuna Regensberg, Sportfreunde Schwaig–Geretsried,
  Jong FC Utrecht–Almere City, FC Dordrecht–Vitesse, Jong AZ–TOP Oss,
  VVV-Venlo–RKC Waalwijk.

## Újra futtatható lekérés

```sql
select m.kickoff_at::date as nap, c.name as liga, co.code as orszag,
       h.name || ' - ' || a.name as meccs
  from core.match m
  join core.competition c on c.id = m.competition_id
  left join core.country co on co.id = c.country_id
  join core.team h on h.id = m.home_team_id
  join core.team a on a.id = m.away_team_id
 where m.status = 'postponed' and m.kickoff_at >= $1 and m.kickoff_at < $2
 order by m.kickoff_at;
```

A riport csak olvas; az elhalasztott meccsek státusza forrásadat, nem kézi
revízió. A `core.match.kickoff_at` az eredeti (elhalasztott) időpont marad —
új időpontot a forrásnak kell adnia.
