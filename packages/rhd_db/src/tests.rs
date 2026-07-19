use super::*;
use std::fs;

#[test]
fn test_next_id_increments() {
    let db_path = "test_increment.db";
    let _ = fs::remove_file(db_path);
    
    let db = ScenarioDb::new(db_path).unwrap();
    
    let id1 = db.next_id().unwrap();
    let id2 = db.next_id().unwrap();
    let id3 = db.next_id().unwrap();
    
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
    
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(format!("{}-wal", db_path));
    let _ = fs::remove_file(format!("{}-shm", db_path));
}

#[test]
fn test_persistence_across_restarts() {
    let db_path = "test_persistence.db";
    let _ = fs::remove_file(db_path);
    
    {
        let db = ScenarioDb::new(db_path).unwrap();
        let id1 = db.next_id().unwrap();
        let id2 = db.next_id().unwrap();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }
    
    {
        let db = ScenarioDb::new(db_path).unwrap();
        let id3 = db.next_id().unwrap();
        assert_eq!(id3, 3);
    }
    
    let _ = fs::remove_file(db_path);
    let _ = fs::remove_file(format!("{}-wal", db_path));
    let _ = fs::remove_file(format!("{}-shm", db_path));
}
