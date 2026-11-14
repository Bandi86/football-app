use std::collections::HashMap;

use football_domain::policy::Transport;
use football_ingest::fetch::{Fetchers, FixtureFetcher};
use football_ingest::http::RustHttpFetcher;

#[test]
fn selects_by_transport() {
    let fetchers = Fetchers::new(
        RustHttpFetcher::new("football-app-test").unwrap(),
        FixtureFetcher::new(HashMap::new()),
    );
    assert!(fetchers.for_transport(&Transport::RustHttp).is_ok());
    assert!(fetchers.for_transport(&Transport::Fixture).is_ok());
    // Obscura fetcher nélkül (feature KI vagy inicializálatlan) → típusos hiba.
    assert!(fetchers.for_transport(&Transport::Obscura).is_err());
}
