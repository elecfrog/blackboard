//! Skill discovery and loading module (Ticket #000050).
//!
//! Skills are stored as `SKILL.md` files under `<bb_root>/skills/{name}/`.
//! Each skill directory may also contain a `reference/` subdirectory with
//! additional context files.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Metadata about a discovered skill.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SkillInfo {
    /// Skill identifier (directory name or frontmatter `name`).
    pub name: String,
    /// Optional description from frontmatter.
    pub description: Option<String>,
    /// Absolute path to the skill directory.
    pub path: PathBuf,
}

/// Full content of a loaded skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillContent {
    pub name: String,
    pub description: Option<String>,
    /// The Markdown body of SKILL.md (after frontmatter).
    pub body: String,
    /// List of reference file paths (relative to skill dir).
    pub reference_files: Vec<String>,
}

/// Discover all valid skills under `<bb_root>/skills/`.
/// Returns an empty Vec if the directory does not exist.
pub fn discover_skills(bb_root: &Path) -> Vec<SkillInfo> {
    let skills_dir = bb_root.join("skills");
    if !skills_dir.is_dir() {
        return Vec::new();
    }

    let mut skills = Vec::new();
    let entries = match fs::read_dir(&skills_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if !skill_md.is_file() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        match parse_skill_frontmatter(&skill_md) {
            Some((name, description)) => {
                skills.push(SkillInfo {
                    name: if name.is_empty() { dir_name } else { name },
                    description,
                    path,
                });
            }
            None => {
                // Fallback: use directory name if frontmatter parsing fails
                skills.push(SkillInfo {
                    name: dir_name,
                    description: None,
                    path,
                });
            }
        }
    }

    skills.sort_by(|a, b| a.name.cmp(&b.name));
    skills
}

/// Load a specific skill by name. Returns None if not found.
pub fn load_skill(bb_root: &Path, skill_name: &str) -> Option<SkillContent> {
    let skill_dir = bb_root.join("skills").join(skill_name);
    let skill_md = skill_dir.join("SKILL.md");
    if !skill_md.is_file() {
        return None;
    }

    let content = fs::read_to_string(&skill_md).ok()?;
    let (name, description, body) = parse_frontmatter_and_body(&content);
    let name = if name.is_empty() {
        skill_name.to_string()
    } else {
        name
    };

    // Collect reference files
    let reference_files = collect_reference_files(&skill_dir);

    Some(SkillContent {
        name,
        description,
        body,
        reference_files,
    })
}

/// Inject skills into the provider-native skill path for the given runtime.
/// Copies skill directories from `<bb_root>/skills/{name}/` to the target path.
pub fn inject_skills_for_runtime(
    bb_root: &Path,
    runtime: &str,
    skills: &[String],
    cwd: &Path,
) -> std::io::Result<()> {
    if skills.is_empty() {
        return Ok(());
    }

    let target_base = match runtime {
        "opencode" => cwd.join(".opencode").join("skills"),
        "codex" => {
            // Codex uses CODEX_HOME/skills or cwd/.codex/skills
            cwd.join(".codex").join("skills")
        }
        "codebuddy" => cwd.join(".codebuddy").join("skills"),
        "claude" => cwd.join(".claude").join("skills"),
        _ => cwd.join(".agent_context").join("skills"),
    };

    for skill_name in skills {
        let source_dir = bb_root.join("skills").join(skill_name);
        if !source_dir.is_dir() {
            // Skip missing skills (warning logged by caller)
            continue;
        }
        let target_dir = target_base.join(skill_name);
        copy_dir_recursive(&source_dir, &target_dir)?;
    }

    Ok(())
}

// ── Internal helpers ──

/// Parse YAML frontmatter from a SKILL.md file.
/// Returns (name, description) or None if no frontmatter found.
fn parse_skill_frontmatter(path: &Path) -> Option<(String, Option<String>)> {
    let content = fs::read_to_string(path).ok()?;
    let (name, description, _) = parse_frontmatter_and_body(&content);
    Some((name, description))
}

/// Parse frontmatter and body from a SKILL.md content string.
/// Frontmatter is delimited by `---` lines.
fn parse_frontmatter_and_body(content: &str) -> (String, Option<String>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (String::new(), None, content.to_string());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let rest = after_first.trim_start_matches(['\r', '\n']);
    if let Some(end_idx) = rest.find("\n---") {
        let frontmatter = &rest[..end_idx];
        let body_start = end_idx + 4; // skip \n---
        let body = rest[body_start..]
            .trim_start_matches(['\r', '\n'])
            .to_string();

        let mut name = String::new();
        let mut description: Option<String> = None;

        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(val) = line.strip_prefix("name:") {
                name = val.trim().trim_matches('"').trim_matches('\'').to_string();
            } else if let Some(val) = line.strip_prefix("description:") {
                let desc = val.trim().trim_matches('"').trim_matches('\'').to_string();
                if !desc.is_empty() {
                    description = Some(desc);
                }
            }
        }

        (name, description, body)
    } else {
        // No closing ---, treat entire content as body
        (String::new(), None, content.to_string())
    }
}

/// Collect reference file paths relative to the skill directory.
fn collect_reference_files(skill_dir: &Path) -> Vec<String> {
    let ref_dir = skill_dir.join("reference");
    if !ref_dir.is_dir() {
        return Vec::new();
    }

    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&ref_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    files.push(format!("reference/{}", name.to_string_lossy()));
                }
            }
        }
    }
    files.sort();
    files
}

/// Recursively copy a directory (idempotent: overwrites existing files).
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)?.flatten() {
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_skill(root: &Path, name: &str, frontmatter: &str, body: &str) {
        let skill_dir = root.join("skills").join(name);
        fs::create_dir_all(&skill_dir).unwrap();
        let content = format!("---\n{frontmatter}\n---\n\n{body}");
        fs::write(skill_dir.join("SKILL.md"), content).unwrap();
    }

    #[test]
    fn discover_skills_empty_when_no_dir() {
        let tmp = TempDir::new().unwrap();
        let skills = discover_skills(tmp.path());
        assert!(skills.is_empty());
    }

    #[test]
    fn discover_skills_finds_valid_skills() {
        let tmp = TempDir::new().unwrap();
        create_skill(
            tmp.path(),
            "triage",
            "name: triage\ndescription: Inbox triage skill",
            "# Triage\n\nDo triage.",
        );
        create_skill(
            tmp.path(),
            "code-review",
            "name: code-review",
            "# Code Review",
        );

        let skills = discover_skills(tmp.path());
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "code-review");
        assert_eq!(skills[1].name, "triage");
        assert_eq!(
            skills[1].description,
            Some("Inbox triage skill".to_string())
        );
    }

    #[test]
    fn discover_skills_uses_dir_name_as_fallback() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("skills").join("my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# No frontmatter").unwrap();

        let skills = discover_skills(tmp.path());
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
    }

    #[test]
    fn load_skill_returns_content() {
        let tmp = TempDir::new().unwrap();
        create_skill(
            tmp.path(),
            "triage",
            "name: triage\ndescription: Triage skill",
            "# Triage\n\nDo triage work.",
        );
        // Add a reference file
        let ref_dir = tmp.path().join("skills/triage/reference");
        fs::create_dir_all(&ref_dir).unwrap();
        fs::write(ref_dir.join("rules.md"), "# Rules").unwrap();

        let skill = load_skill(tmp.path(), "triage").unwrap();
        assert_eq!(skill.name, "triage");
        assert_eq!(skill.description, Some("Triage skill".to_string()));
        assert!(skill.body.contains("Do triage work."));
        assert_eq!(skill.reference_files, vec!["reference/rules.md"]);
    }

    #[test]
    fn load_skill_returns_none_for_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(load_skill(tmp.path(), "nonexistent").is_none());
    }

    #[test]
    fn inject_skills_copies_to_target() {
        let tmp = TempDir::new().unwrap();
        create_skill(tmp.path(), "triage", "name: triage", "# Triage");

        let cwd = TempDir::new().unwrap();
        inject_skills_for_runtime(tmp.path(), "opencode", &["triage".to_string()], cwd.path())
            .unwrap();

        let target = cwd.path().join(".opencode/skills/triage/SKILL.md");
        assert!(target.exists());
    }
}
