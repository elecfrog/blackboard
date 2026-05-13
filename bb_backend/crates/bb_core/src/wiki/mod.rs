//! Wiki filesystem operations for project-level knowledge bases.
//!
//! Wiki content lives under `projects/<project>/wiki/` and is a third
//! first-class resource next to `tickets/` and `inbox/`. It follows a
//! convention shared with the upstream opencode wiki workflow:
//!
//! - `wiki/index.md` — entry point (presence is recommended but not required
//!   for the tree to be considered present; an empty wiki dir is still a
//!   legitimate wiki)
//! - `wiki/architecture/`, `wiki/modules/<sub>/`, `wiki/topics/`,
//!   `wiki/references/` — optional, conventional subdirectories; `modules/`
//!   may nest its own sub-wikis (`modules/<sub>/index.md + ...`)
//!
//! This module provides read-only access to the wiki tree, individual
//! Markdown documents, and relative static assets (images, svg, pdf, ...).
//! All writes are handled out-of-band (either by the user's external
//! distillation workflow or by the upload endpoint in `bb_server`).

mod content;
mod extensions;
mod model;
mod root;
mod tree;
mod validation;

pub use model::{WikiAsset, WikiContentResponse, WikiNodeKind, WikiTreeNode, WikiTreeResponse};

#[cfg(test)]
mod tests;
