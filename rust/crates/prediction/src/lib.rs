//! Predikciós alapkönyvtár (S195 / Phase 0).
//!
//! Tiszta, adatbázis- és hálózatmentes modul: piac-marginmentesítés (Shin),
//! valószínűségi metrikák konfidenciaintervallummal, és a szigorú as-of őr.
//! A modell és a kanonikus írás későbbi szelet; itt semmilyen entitást nem írunk.
//!
//! A v1 tanulságai miatt a mérési szerződés része a **klaszterezett bootstrap**
//! (meccs-szintű függetlenség feltételezése nélkül) és a Wilson-intervallum —
//! CI nélküli „jobb, mint a bázis" verdiktet nem engedünk.

pub mod as_of;
pub mod fit;
pub mod goals;
pub mod margin;
pub mod metrics;
