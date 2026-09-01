//! Pure domain logic.
//!
//! Nothing in this module reads the filesystem, shells out, or talks to the
//! network. Every function here is a total, deterministic mapping from its
//! inputs to its result, which is what makes the rules exhaustively testable.

pub mod bump;

pub use bump::{PreLabel, StableBump, Transition, TransitionError, apply};
