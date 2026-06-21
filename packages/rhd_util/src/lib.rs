pub use thiserror;
pub use serde;

#[derive(thiserror::Error, Debug)]
pub enum RhdError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Custom(String),
}

pub type RhdResult<T> = std::result::Result<T, RhdError>;

pub fn substitute_env_vars(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            let mut var_name = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_alphanumeric() || next_ch == '_' {
                    var_name.push(next_ch);
                    chars.next();
                } else {
                    break;
                }
            }
            if var_name.is_empty() {
                output.push('$');
            } else if let Ok(val) = std::env::var(&var_name) {
                output.push_str(&val);
            } else {
                output.push('$');
                output.push_str(&var_name);
            }
        } else {
            output.push(ch);
        }
    }

    output
}

#[cfg(test)]
mod tests {
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
}
