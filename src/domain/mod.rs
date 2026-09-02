//! Pure domain logic.
//!
//! Nothing in this module reads the filesystem, shells out, or talks to the
//! network. Every function here is a total, deterministic mapping from its
//! inputs to its result, which is what makes the rules exhaustively testable.

pub mod bump;
pub mod checksum;
pub mod tag;
pub mod version_file;

pub use bump::{PreLabel, StableBump, Transition, TransitionError, apply};
pub use checksum::{ChecksumError, Checksums};
pub use tag::{TagPattern, TagPatternError};
pub use version_file::{Format, VersionFileError};
