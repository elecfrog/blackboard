use serde::{Deserialize, Serialize};

/// One node in the wiki tree. `path` is always a POSIX-style path relative
/// to `projects/<project>/wiki/` (no leading slash, forward slashes only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiTreeNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "kind")]
    pub kind: WikiNodeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<WikiTreeNode>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WikiNodeKind {
    File,
    Dir,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WikiTreeResponse {
    pub generated_at: String,
    pub tree: Vec<WikiTreeNode>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WikiContentResponse {
    pub content: String,
    /// Lowercased file extension without the leading dot (e.g. `"md"`,
    /// `"svg"`, `"json"`). The frontend uses it to pick a renderer —
    /// Markdown for `md`/`markdown`, an SVG dual-view for `svg`, a
    /// syntax-highlighted code block for everything else.
    pub content_type: String,
}

/// Raw asset bytes + detected MIME type for relative wiki resources
/// (images, svg, pdf, ...). Used by `bb_server` to serve a binary response.
#[derive(Debug)]
pub struct WikiAsset {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}
