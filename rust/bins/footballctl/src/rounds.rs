//! S211: `footballctl rounds-backfill [--apply]` — a kanonikus meccsek
//! hiányzó fordulószámának visszajátszható pótlása. Alapból dry-run.

use anyhow::Result;
use football_storage::repositories::round_labels;
use sqlx::PgPool;

pub async fn run(pool: &PgPool, apply: bool) -> Result<()> {
    if !apply {
        let count = round_labels::count_missing_round_labels(pool).await?;
        println!("Fordulóval pótolható meccs: {count}. Dry-run — semmit nem írtam (--apply).");
        return Ok(());
    }
    let result = round_labels::backfill_round_labels(pool).await?;
    println!(
        "Forduló-pótlás: {} jelölt, {} sor írva (idempotens újrafutás: 0).",
        result.candidates, result.updated
    );
    Ok(())
}
