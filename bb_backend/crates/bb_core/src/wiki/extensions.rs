use std::path::Path;

/// Extensions that can be safely read as UTF-8 text via
/// `read_wiki_content`. This is intentionally broad: "if it can be
/// previewed, preview it" — SVG, XML, JSON and source code all get
/// rendered inline on the document pane. Truly binary formats (png, pdf,
/// ...) still go through `read_wiki_asset` and are served as bytes.
pub(super) const WIKI_DOC_EXTENSIONS: &[&str] = &[
    // Markdown proper.
    "md",
    "markdown",
    // Plain text and notes.
    "txt",
    "log",
    "csv",
    "tsv",
    "rst",
    "org",
    // Structured config / data (all text).
    "json",
    "jsonc",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "env",
    // Web / markup.
    "html",
    "htm",
    "xml",
    "svg",
    "css",
    "scss",
    "less",
    // Source code (kept broad so dropping a module's files into wiki for
    // review works out of the box; renderer just highlights them).
    "js",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "jsx",
    "vue",
    "rs",
    "py",
    "go",
    "java",
    "kt",
    "kts",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "rb",
    "php",
    "sh",
    "bash",
    "zsh",
    "ps1",
    "sql",
    "lua",
    "swift",
    "dart",
    "proto",
    "dockerfile",
    "makefile",
    "gradle",
];

/// Binary asset extensions served by `read_wiki_asset`. These are NOT
/// overlapping with document extensions for routing purposes: the
/// frontend decides which endpoint to hit based on extension. SVG is a
/// deliberate exception — it's valid as both (text via file, rasterised
/// by <img> via asset), and the frontend picks per use-case.
pub(super) const WIKI_BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "bmp", "pdf", "svg",
];

/// Full set of extensions accepted by the upload endpoint. Union of the
/// two sets above — uploads must produce files at least one read endpoint
/// can serve.
pub(super) const WIKI_UPLOAD_EXTENSIONS: &[&str] = &[
    // docs (subset; kept explicit so stray entries in WIKI_DOC_EXTENSIONS
    // don't silently become uploadable without review).
    "md", "markdown", "txt", "log", "csv", "tsv", "json", "jsonc", "yaml", "yml", "toml", "ini",
    "cfg", "conf", "html", "htm", "xml", "css", "scss", "less", "js", "mjs", "cjs", "ts", "tsx",
    "jsx", "vue", "rs", "py", "go", "java", "sh", "bash", "ps1", "sql", // binaries
    "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "bmp", "pdf", "svg",
];

/// Sniff a `Content-Type` from the path's extension for binary asset
/// responses. Unknown or text-only extensions fall back to
/// `application/octet-stream` here — they should not reach this code path
/// because `ValidateKind::Asset` bounds what is allowed; any additions
/// to `WIKI_BINARY_EXTENSIONS` must also get a MIME mapping below.
pub(super) fn sniff_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("ico") => "image/x-icon",
        Some("bmp") => "image/bmp",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}
