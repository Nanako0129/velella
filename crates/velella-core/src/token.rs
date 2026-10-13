//! Account token newtype. Validation pattern adapted from Serein's `SessionSecret`
//! (refs/serein/crates/client-core/src/auth.rs, MIT OR Apache-2.0).
use std::fmt;
use zeroize::Zeroizing;

/// Intentionally neither Clone, Display nor Serialize (share via `Arc`). Only this crate can read
/// the value, and only to put it into IDENTIFY/RESUME payloads.
pub struct Token(Zeroizing<String>);

/// Fixed-text error; never carries the rejected input.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TokenError;

impl Token {
    pub fn new(value: String) -> Result<Self, TokenError> {
        let value = Zeroizing::new(value);
        if value.len() < 16
            || value.len() > 2048
            || value.starts_with("Bot ")
            || value.starts_with("Bearer ")
            || !value.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(TokenError);
        }
        Ok(Self(value))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token([REDACTED])")
    }
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid token")
    }
}
impl fmt::Debug for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TokenError")
    }
}
impl std::error::Error for TokenError {}

pub(crate) fn serialize<S: serde::Serializer>(token: &&Token, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(token.expose())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "SYNTHETIC_TOKEN_MARKER_0123456789";

    #[test]
    fn token_accepts_valid_and_rejects_bad_input_without_echo() {
        assert!(Token::new(MARKER.into()).is_ok());
        let too_long = format!("{MARKER}{}", "x".repeat(2048));
        for bad in [
            String::new(),
            "short".to_owned(),
            too_long,
            format!("Bot {MARKER}"),
            format!("Bearer {MARKER}"),
            format!("{MARKER} with space"),
            format!("{MARKER}\r\nAuthorization: x"),
            format!("{MARKER}\u{e9}"),
        ] {
            let err = Token::new(bad).unwrap_err();
            assert!(!format!("{err:?} {err}").contains("SYNTHETIC"));
        }
        assert!(Token::new("a".repeat(15)).is_err());
        assert!(Token::new("a".repeat(16)).is_ok());
        assert!(Token::new("a".repeat(2048)).is_ok());
        assert!(Token::new("a".repeat(2049)).is_err());
    }

    #[test]
    fn token_debug_is_redacted() {
        let token = Token::new(MARKER.into()).unwrap();
        assert_eq!(format!("{token:?}"), "Token([REDACTED])");
        assert_eq!(format!("{token:#?}"), "Token([REDACTED])");
    }
}
