//! Inbox note CRUD, indexing, and search logic for a single project.

mod crud;
mod index;
mod input;
mod render;
mod search;

#[cfg(feature = "schema")]
pub(crate) use render::inbox_json_document_schema;
pub use render::render_note;
