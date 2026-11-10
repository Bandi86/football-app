use football_storage::object_store::ObjectStore;

#[test]
fn identical_payload_is_stored_once_and_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(temp.path()).unwrap();
    let payload = br#"{"match":"A-1001"}"#;

    let first = store.put_json(payload).unwrap();
    let second = store.put_json(payload).unwrap();

    assert_eq!(first.sha256, second.sha256);
    assert_eq!(first.storage_key, second.storage_key);
    assert_eq!(store.get(&first).unwrap(), payload);
    assert_eq!(
        std::fs::read_dir(temp.path().join("raw")).unwrap().count(),
        1
    );
}
