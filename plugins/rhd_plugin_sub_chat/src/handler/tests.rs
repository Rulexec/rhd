//! Unit tests for the handler entries ([`super::parse_target_chat_id`]).

use super::parse_target_chat_id;

#[test]
fn parses_the_contract_shape() {
    assert_eq!(parse_target_chat_id(r#"{"chatId":5}"#), Ok(5));
}

#[test]
fn rejects_missing_key() {
    assert!(parse_target_chat_id("{}").is_err());
    // The error fragment names the missing field for the model.
    let err = parse_target_chat_id("{}").unwrap_err();
    assert!(err.contains("chatId"), "error mentions the key: {err}");
}

#[test]
fn rejects_non_integer_values() {
    for arguments in [
        r#"{"chatId":"5"}"#,
        r#"{"chatId":5.5}"#,
        r#"{"chatId":null}"#,
        r#"{"chatId":true}"#,
    ] {
        assert!(
            parse_target_chat_id(arguments).is_err(),
            "expected rejection of {arguments}"
        );
    }
}

#[test]
fn rejects_non_object_and_unparseable_json() {
    for arguments in [
        "",
        "not json at all",
        "5",
        "\"chat\"",
        "[5]",
        "[{\"chatId\":5}]",
    ] {
        assert!(
            parse_target_chat_id(arguments).is_err(),
            "expected rejection of {arguments:?}"
        );
    }
}

#[test]
fn tolerates_extra_fields_like_spawn_parsing() {
    assert_eq!(parse_target_chat_id(r#"{"chatId":42,"junk":true}"#), Ok(42));
}
