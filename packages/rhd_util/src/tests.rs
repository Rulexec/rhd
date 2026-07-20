use super::*;

#[test]
fn substitutes_env_var() {
    std::env::set_var("TEST_VAR_123", "hello");
    assert_eq!(substitute_env_vars("val=$TEST_VAR_123!"), "val=hello!");
    std::env::remove_var("TEST_VAR_123");
}

#[test]
fn missing_var_kept_as_is() {
    assert_eq!(
        substitute_env_vars("val=$NONEXISTENT_VAR_XYZ"),
        "val=$NONEXISTENT_VAR_XYZ"
    );
}

#[test]
fn no_vars_passthrough() {
    assert_eq!(substitute_env_vars("hello world"), "hello world");
}

#[test]
fn dollar_at_end() {
    assert_eq!(substitute_env_vars("abc$"), "abc$");
}

#[test]
fn multiple_vars() {
    std::env::set_var("TEST_A", "1");
    std::env::set_var("TEST_B", "2");
    assert_eq!(
        substitute_env_vars("$TEST_A and $TEST_B"),
        "1 and 2"
    );
    std::env::remove_var("TEST_A");
    std::env::remove_var("TEST_B");
}
