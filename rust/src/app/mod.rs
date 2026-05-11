//! Command orchestration modules.
//!
//! Each sub-module owns the orchestration flow for one CLI command. The
//! composition root in `lib.rs` wires real boundary adapters and then delegates
//! to these modules. The modules themselves receive already-loaded config and
//! runtime state rather than owning bootstrap logic.

pub(crate) mod apply;
pub(crate) mod info;
