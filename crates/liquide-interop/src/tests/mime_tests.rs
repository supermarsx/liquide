use crate::mime::{MimeAssociation, MimeDatabase, MimeSource, MimeType};

#[test]
fn test_parse_type() {
    let mt = MimeType::parse("text/plain").unwrap();
    assert_eq!(mt.type_, "text");
    assert_eq!(mt.subtype, "plain");
}

#[test]
fn test_parse_invalid() {
    assert!(MimeType::parse("invalid").is_err());
}

#[test]
fn test_matches_exact() {
    let a = MimeType::parse("text/plain").unwrap();
    let b = MimeType::parse("text/plain").unwrap();
    assert!(a.matches(&b));
}

#[test]
fn test_matches_wildcard() {
    let a = MimeType::parse("text/*").unwrap();
    let b = MimeType::parse("text/html").unwrap();
    assert!(a.matches(&b));
}

#[test]
fn test_database_add_lookup() {
    let mut db = MimeDatabase::new();
    let mt = MimeType::parse("text/plain").unwrap();
    db.add_association(MimeAssociation {
        mime_type: mt.clone(),
        desktop_entry_id: "org.gnome.TextEditor".to_string(),
        source: MimeSource::System,
    });
    let results = db.lookup(&mt);
    assert_eq!(results, vec!["org.gnome.TextEditor"]);
}

#[test]
fn test_default_for() {
    let mut db = MimeDatabase::new();
    let mt = MimeType::parse("text/html").unwrap();
    db.add_association(MimeAssociation {
        mime_type: mt.clone(),
        desktop_entry_id: "firefox".to_string(),
        source: MimeSource::System,
    });
    assert_eq!(db.default_for(&mt), Some("firefox".to_string()));
}

#[test]
fn test_multiple_associations() {
    let mut db = MimeDatabase::new();
    let mt = MimeType::parse("image/png").unwrap();
    db.add_association(MimeAssociation {
        mime_type: mt.clone(),
        desktop_entry_id: "eog".to_string(),
        source: MimeSource::System,
    });
    db.add_association(MimeAssociation {
        mime_type: mt.clone(),
        desktop_entry_id: "gimp".to_string(),
        source: MimeSource::Application,
    });
    let results = db.lookup(&mt);
    assert_eq!(results.len(), 2);
}

#[test]
fn test_user_precedence() {
    let mut db = MimeDatabase::new();
    let mt = MimeType::parse("text/html").unwrap();
    db.add_association(MimeAssociation {
        mime_type: mt.clone(),
        desktop_entry_id: "firefox".to_string(),
        source: MimeSource::System,
    });
    db.add_association(MimeAssociation {
        mime_type: mt.clone(),
        desktop_entry_id: "chromium".to_string(),
        source: MimeSource::User,
    });
    assert_eq!(db.default_for(&mt), Some("chromium".to_string()));
}
