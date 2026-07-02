//! Small shared, dependency-light helpers that both repos need.
//!
//! - [`ct_eq_bytes`] / [`ct_eq_str`]: constant-time equality for secret
//!   comparison. Both repos rolled several near-identical `subtle`-based
//!   copies; this is the one home.
//! - [`env_truthy`]: shared boolean-env parsing.
//! - [`backoff_ms`]: the capped-exponential term shared by every retry loop.
//!   Jitter is deliberately left to the caller so this crate needs no `rand`
//!   (dyson jitters with `rand`, swarm avoids it on purpose).

use subtle::ConstantTimeEq;

/// Constant-time byte comparison. Returns `false` on length mismatch (the
/// length itself is not secret — payload lengths leak through every transport
/// layer above this anyway). Equal-length inputs are compared byte-for-byte in
/// constant time via `subtle`.
#[inline]
#[must_use]
pub fn ct_eq_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    bool::from(a.ct_eq(b))
}

/// Constant-time string comparison. Identical semantics to [`ct_eq_bytes`] over
/// the underlying UTF-8 bytes.
#[inline]
#[must_use]
pub fn ct_eq_str(a: &str, b: &str) -> bool {
    ct_eq_bytes(a.as_bytes(), b.as_bytes())
}

/// Boolean environment-variable parser. Returns `true` when `$name` is set to
/// `1`, `true`, `TRUE`, `yes`, or `YES`; everything else (including unset,
/// empty, or "false") is `false`. Centralised so adding or removing an accepted
/// spelling is a one-place change.
#[must_use]
pub fn env_truthy(name: &str) -> bool {
    matches!(
        std::env::var(name).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

/// Largest exponent [`backoff_ms`] applies to `base_ms`. Caps the exponential
/// term at `base_ms * 2^6` so a long retry streak can't overflow or produce an
/// absurd sleep.
pub const MAX_BACKOFF_SHIFT: u32 = 6;

/// Capped exponential backoff term: `base_ms * 2^min(attempt, MAX_BACKOFF_SHIFT)`,
/// saturating. This is the deterministic, dependency-free part every retry loop
/// shares; callers add their own jitter on top (dyson via `rand`, swarm via a
/// pseudo-random source) so this crate stays lean.
#[must_use]
pub fn backoff_ms(base_ms: u64, attempt: usize) -> u64 {
    let shift = (attempt as u32).min(MAX_BACKOFF_SHIFT);
    base_ms.saturating_mul(1u64 << shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_bytes_match() {
        assert!(ct_eq_bytes(b"hello", b"hello"));
    }

    #[test]
    fn different_bytes_diverge() {
        assert!(!ct_eq_bytes(b"hello", b"world"));
    }

    #[test]
    fn length_mismatch_short_circuits() {
        assert!(!ct_eq_bytes(b"hello", b"hello!"));
        assert!(!ct_eq_bytes(b"hello!", b"hello"));
        assert!(!ct_eq_str("abc", "abcd"));
    }

    #[test]
    fn empty_and_utf8() {
        assert!(ct_eq_bytes(b"", b""));
        assert!(!ct_eq_bytes(b"", b"x"));
        assert!(ct_eq_str("café", "café"));
        assert!(ct_eq_str("token-abc", "token-abc"));
        assert!(!ct_eq_str("token-abc", "token-xyz"));
    }

    #[test]
    fn backoff_is_capped_and_saturating() {
        assert_eq!(backoff_ms(100, 0), 100);
        assert_eq!(backoff_ms(100, 1), 200);
        assert_eq!(backoff_ms(100, 6), 100 * 64);
        // Beyond the shift cap the term stops growing.
        assert_eq!(backoff_ms(100, 50), 100 * 64);
        // Saturates instead of overflowing.
        assert_eq!(backoff_ms(u64::MAX, 3), u64::MAX);
    }

    /// Unique env-var names per case so parallel `cargo test` (shared process
    /// env) doesn't race.
    #[test]
    fn env_truthy_recognises_truthy_spellings() {
        for (i, val) in ["1", "true", "TRUE", "yes", "YES"].iter().enumerate() {
            let name = format!("DYSON_COMMON_ENV_TRUTHY_T{i}");
            // SAFETY: distinct name, only this test touches it.
            unsafe { std::env::set_var(&name, val) };
            assert!(env_truthy(&name), "expected {val:?} truthy");
            unsafe { std::env::remove_var(&name) };
        }
    }

    #[test]
    fn env_truthy_rejects_falsey_and_unset() {
        assert!(!env_truthy("DYSON_COMMON_ENV_TRUTHY_UNSET"));
        for (i, val) in ["0", "false", "no", "off", "", "True"].iter().enumerate() {
            let name = format!("DYSON_COMMON_ENV_TRUTHY_F{i}");
            unsafe { std::env::set_var(&name, val) };
            assert!(!env_truthy(&name), "expected {val:?} falsey");
            unsafe { std::env::remove_var(&name) };
        }
    }
}
