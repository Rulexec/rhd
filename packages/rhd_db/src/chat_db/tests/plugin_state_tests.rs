use crate::ChatDb;

#[test]
fn first_upsert_has_version_one() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();

    let row = db
        .upsert_plugin_state("p1", "status", "hello", "json", "s:1")
        .unwrap();

    assert_eq!(row.plugin_id, "p1");
    assert_eq!(row.key, "status");
    assert_eq!(row.content, "hello");
    assert_eq!(row.format, "json");
    assert_eq!(row.schema, "s:1");
    assert_eq!(row.version, 1);
    assert!(!row.is_removed);
    assert!(!row.updated_at.is_empty());
}

#[test]
fn update_bumps_version_and_replaces_fields() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();

    let first = db
        .upsert_plugin_state("p1", "status", "one", "json", "a")
        .unwrap();
    assert_eq!(first.version, 1);

    let second = db
        .upsert_plugin_state("p1", "status", "two", "markdown", "b")
        .unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(second.content, "two");
    assert_eq!(second.format, "markdown");
    assert_eq!(second.schema, "b");
    assert!(!second.is_removed);
}

#[test]
fn remove_tombstones_bumps_version_and_hides_from_get() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();
    db.upsert_plugin_state("p1", "status", "x", "json", "a")
        .unwrap();

    let version = db.remove_plugin_state("p1", "status").unwrap();
    assert_eq!(version, Some(2));

    let states = db.get_plugin_states(None, None).unwrap();
    assert!(states.is_empty());

    let tombstone = db
        .get_plugin_state("p1", "status")
        .unwrap()
        .expect("tombstone row still present");
    assert!(tombstone.is_removed);
    assert_eq!(tombstone.version, 2);
}

#[test]
fn remove_is_noop_on_missing_or_already_removed() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();

    assert_eq!(db.remove_plugin_state("p1", "missing").unwrap(), None);

    db.upsert_plugin_state("p1", "status", "x", "json", "a")
        .unwrap();
    assert_eq!(db.remove_plugin_state("p1", "status").unwrap(), Some(2));
    assert_eq!(db.remove_plugin_state("p1", "status").unwrap(), None);
}

#[test]
fn recreate_after_remove_is_monotonic() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();

    let created = db
        .upsert_plugin_state("p1", "status", "v1", "json", "a")
        .unwrap();
    assert_eq!(created.version, 1);

    let removed_version = db.remove_plugin_state("p1", "status").unwrap();
    assert_eq!(removed_version, Some(2));

    let recreated = db
        .upsert_plugin_state("p1", "status", "v3", "json", "a")
        .unwrap();
    assert_eq!(recreated.version, 3);
    assert!(!recreated.is_removed);
    assert_eq!(recreated.content, "v3");
}

#[test]
fn keys_are_namespaced_per_plugin() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();
    db.register_plugin("p2").unwrap();

    let p1_first = db.upsert_plugin_state("p1", "status", "a", "json", "s").unwrap();
    let p2_first = db.upsert_plugin_state("p2", "status", "b", "json", "s").unwrap();
    assert_eq!(p1_first.version, 1);
    assert_eq!(p2_first.version, 1);

    let p1_second = db
        .upsert_plugin_state("p1", "status", "a2", "json", "s")
        .unwrap();
    assert_eq!(p1_second.version, 2);

    let p2_stored = db
        .get_plugin_state("p2", "status")
        .unwrap()
        .expect("p2 row present");
    assert_eq!(p2_stored.version, 1);
    assert_eq!(p2_stored.content, "b");
}

#[test]
fn get_filters_by_plugin_and_schema() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();
    db.register_plugin("p2").unwrap();

    db.upsert_plugin_state("p1", "status", "1", "json", "a")
        .unwrap();
    db.upsert_plugin_state("p1", "notes", "2", "markdown", "b")
        .unwrap();
    db.upsert_plugin_state("p2", "status", "3", "json", "a")
        .unwrap();

    let all = db.get_plugin_states(None, None).unwrap();
    assert_eq!(all.len(), 3);

    let p1_only = db.get_plugin_states(Some("p1"), None).unwrap();
    assert_eq!(p1_only.len(), 2);
    assert!(p1_only.iter().all(|s| s.plugin_id == "p1"));

    let schema_a = db.get_plugin_states(None, Some("a")).unwrap();
    assert_eq!(schema_a.len(), 2);
    assert!(schema_a.iter().all(|s| s.schema == "a"));

    let p2_schema_a = db.get_plugin_states(Some("p2"), Some("a")).unwrap();
    assert_eq!(p2_schema_a.len(), 1);
    assert_eq!(p2_schema_a[0].plugin_id, "p2");
    assert_eq!(p2_schema_a[0].key, "status");

    let p2_schema_b = db.get_plugin_states(Some("p2"), Some("b")).unwrap();
    assert!(p2_schema_b.is_empty());
}

#[test]
fn remove_plugin_cascades_states() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();
    db.upsert_plugin_state("p1", "status", "x", "json", "a")
        .unwrap();

    db.remove_plugin("p1").unwrap();

    let states = db.get_plugin_states(None, None).unwrap();
    assert!(states.is_empty());
    assert_eq!(db.get_plugin_state("p1", "status").unwrap(), None);

    db.register_plugin("p1").unwrap();
    let recreated = db
        .upsert_plugin_state("p1", "status", "y", "json", "a")
        .unwrap();
    assert_eq!(recreated.version, 1);
}

#[test]
fn deactivate_plugin_keeps_states() {
    let db = ChatDb::new(":memory:").unwrap();
    db.register_plugin("p1").unwrap();
    db.upsert_plugin_state("p1", "status", "x", "json", "a")
        .unwrap();

    db.deactivate_plugin("p1").unwrap();

    let states = db.get_plugin_states(None, None).unwrap();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].plugin_id, "p1");
    assert_eq!(states[0].version, 1);
    assert!(!states[0].is_removed);
}
