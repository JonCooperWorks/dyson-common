//! Secret detection and redaction for agent traffic, logs and traces.
//!
//! * [`redact`] (always built): take known secrets out of text and JSON
//!   wherever and however they are written, and [`redact::scrub`] a
//!   document until a rescan finds nothing more. Dependency-light.
//! * [`Scanner`] (`kingfisher` feature): MongoDB's Kingfisher with its
//!   built-in rules and Betterleaks' generic rules, reporting each secret
//!   as a [`Hit`] with its place, the lines around it and the name it is
//!   assigned to. It never validates a key against its issuer.
//! * [`payment`] (`payment` feature): card numbers and IBANs, with the
//!   card's issuer from a compiled-in BIN table. With `kingfisher` too,
//!   [`Scanner::scan_at`] reports them first.
//! * [`external`] (`external` feature): Betterleaks, TruffleHog and Nosey
//!   Parker run as programs over a batch of texts, merged into
//!   [`Scanner::scan_at`].
//!
//! Detection only: nothing here ever presents a found key to its issuer.

pub mod hit;
pub mod redact;
pub mod text;

#[cfg(feature = "external")]
pub mod external;
#[cfg(feature = "kingfisher")]
pub mod kingfisher;
#[cfg(feature = "payment")]
pub mod payment;

pub use hit::{
    ContextLine, Dependency, Detection, Extra, Hit, Issuer, KingfisherLocation, Paired, pair,
};
#[cfg(feature = "kingfisher")]
pub use kingfisher::{At, Scanner};
pub use redact::{Leftover, Redactor, Scrubbed, scrub};

/// Why detection or redaction could not be done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}
