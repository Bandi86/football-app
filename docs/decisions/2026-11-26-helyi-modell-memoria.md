# Helyi AI-modell kiválasztása 16 GB-os M4-en (2026-11-26)

## Helyzet

A `football-ai` alapértelmezett modellje a gemma 4 E4B Artemis Q4_0 volt.
A gép Mac mini M4 / **16 GB unified**, de a Magnitude csak **2,42 GB**
szabad memóriát látott, miközben a modell betöltése **5,00 GB**-t igényel
(4,44 GB modell + 292 MB MTP drafter). A `magnitude models list` ezért
`memory_shortage` / `retryable` állapotot mutatott, aktív modell nélkül.
A rendszermemória-nyomás zöld volt (582 MB swap, 3,34 GB cache), tehát nem
hardveres OOM, hanem a betöltési pillanatban rendelkezésre álló memória hiányzott.
A Magnitude egy óra kérés nélkül vagy memórianyomáskor automatikusan
kiveszi a modellt — ezért tűnt úgy, hogy „soha nincs elég memória".

## Döntés

1. A `football-ai` napi modellje: **`lfm3-1.7b:gguf:q4-qat`**
   (`FOOTBALL_AI_MODEL` a lokális `.env`-ben). Mérete 1,05 GB,
   betöltési igénye **2,7 GB**, tool-call és structured output támogatással;
   vision nincs, de a `footballctl ai search|ask` szöveges munkához nem kell.
2. A gemma 4 E4B Artemis Q4_0 **marad telepítve**, de a draftere kikapcsolva
   (`magnitude models drafter <id> none`, ~292 MB-tal kisebb a igény) —
   csak akkor használjuk, ha épp szabad a memória.
3. Nincs külső API, token vagy felhő modell; nincs RAM-bővítés (az M4 RAM-ja
   forrasztott), és nem növeljük a swap-ot.
4. Az LFM3 hibrid reasoning modell: `reasoning_effort` nélkül a 700 tokenes
   válaszkeret nagy részét gondolkodásra használja, és `content: null` jön vissza.
   Ezért a `football-ai` opcionálisan elküldi a `reasoning_effort` mezőt
   (`FOOTBALL_AI_REASONING_EFFORT`, alapból nincs elküldve); lokálisan `none`.
   Így az `ai ask` ismét működik tool-callal és forrásokkal.

## Indoklás

- Az LFM3 jelenlegi 2,4–2,5 GB-os szabad memóriába is bekerül, tehát a
  `footballctl ai search|ask` megbízhatóan működik; a gemma eseti használatra
  alkalmas csak ezen a gépen.
- A modellnév kód nélkül, környezeti változóval állítható
  (`rust/crates/ai/src/config.rs`), ezért nem kellett fordítani vagy API-szerződést módosítani.

## Visszaállás

- `FOOTBALL_AI_MODEL` visszaállítása a korábbi értékre (vagy a sor törlése:
  ekkor a default a gemma 4 E4B Artemis Q4_0).
- `magnitude models drafter <gemma-id> auto` — visszaállítja az MTP draftert.
- A letöltött LFM3 modell `magnitude models rm lfm3-1.7b:gguf:q4-qat`-vel távolítható el.
