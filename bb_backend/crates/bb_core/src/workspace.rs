//! Workspace — 多项目工作区管理。
//!
//! `Workspace` 根植于 Blackboard data root，管理 `projects/` 下的所有项目注册。
//!
//! Source checkouts keep the bundled Blackboard template in `<repo>/.bb_template`.
//! User/global data and project capsules still use `.bb`; older workspaces may
//! still use `<repo>/.bb` or `<repo>/projects` directly. `Workspace::open`
//! accepts those shapes so dev scripts can keep passing the repository root.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::InboxError;
use crate::fs_util::{
    canonicalize, canonicalize_existing_dir, clean_path_string, path_to_string, read_json_file,
    write_json_pretty,
};
use crate::platform::{current_os_name, current_os_version, machine_host_name, user_home_dir};
use crate::types::*;
use crate::{
    agents_registry, normalize_project_name_or_uuid, project, validate_project_name, Blackboard,
};
use serde::{Deserialize, Serialize};

pub const WORKSPACE_DATA_DIR: &str = ".bb";
pub const WORKSPACE_TEMPLATE_DIR: &str = ".bb_template";
pub const WORKSPACE_MANIFEST: &str = "blackboard.json";
pub const WORKSPACE_RUNTIME_DIR: &str = "runtime";
pub const WORKSPACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct WorkspaceManifest {
    schema_version: u32,
    layout: String,
}

/// Workspace roots Blackboard's writable data directory and enumerates projects
/// under `<data-root>/projects/<project>/`.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
    projects_root: PathBuf,
}

impl Workspace {
    pub fn init_from_seed(
        target_root: impl AsRef<Path>,
        seed_root: impl AsRef<Path>,
    ) -> Result<Self, InboxError> {
        let target_root = Self::target_data_root(target_root.as_ref());
        let seed_root = canonicalize(seed_root.as_ref())?;
        let seed_root = Self::resolve_data_root(&seed_root)?;

        fs::create_dir_all(&target_root).map_err(|source| InboxError::Io {
            path: target_root.clone(),
            source,
        })?;

        for dirname in ["agents", "projects", "task_graphs", "templates"] {
            let source = seed_root.join(dirname);
            if source.is_dir() {
                copy_dir_missing(&source, &target_root.join(dirname))?;
            }
        }

        let source_manifest = seed_root.join(WORKSPACE_MANIFEST);
        let target_manifest = target_root.join(WORKSPACE_MANIFEST);
        if source_manifest.is_file() && !target_manifest.exists() {
            fs::copy(&source_manifest, &target_manifest).map_err(|source| InboxError::Io {
                path: target_manifest.clone(),
                source,
            })?;
        }

        Self::open(target_root)
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self, InboxError> {
        let requested_root = canonicalize(root.as_ref())?;
        let root = Self::resolve_data_root(&requested_root)?;
        let projects_root = root.join("projects");
        let projects_root = canonicalize_existing_dir(&projects_root)
            .map_err(|_| InboxError::ProjectsRootMissing(root.clone()))?;

        if !projects_root.starts_with(&root) {
            return Err(InboxError::ProjectsRootMissing(root));
        }

        let workspace = Self {
            root,
            projects_root,
        };
        workspace.ensure_workspace_layout()?;
        workspace.ensure_projects_registry()?;
        Ok(workspace)
    }

    pub fn discover() -> Result<Self, InboxError> {
        let cwd = std::env::current_dir().map_err(|source| InboxError::Io {
            path: PathBuf::from("."),
            source,
        })?;
        Self::discover_from(cwd)
    }

    pub fn discover_from(start: impl AsRef<Path>) -> Result<Self, InboxError> {
        let start = canonicalize(start.as_ref())?;

        for ancestor in start.ancestors() {
            let direct_template = ancestor.join(WORKSPACE_TEMPLATE_DIR).join("projects");
            if direct_template.is_dir() {
                return Self::open(ancestor);
            }

            let direct_dotbb = ancestor.join(WORKSPACE_DATA_DIR).join("projects");
            if direct_dotbb.is_dir() {
                return Self::open(ancestor);
            }

            let direct = ancestor.join("projects");
            if direct.is_dir() {
                return Self::open(ancestor);
            }

            let nested_template = ancestor
                .join("blackboard")
                .join(WORKSPACE_TEMPLATE_DIR)
                .join("projects");
            if nested_template.is_dir() {
                return Self::open(ancestor.join("blackboard"));
            }

            let nested_dotbb = ancestor
                .join("blackboard")
                .join(WORKSPACE_DATA_DIR)
                .join("projects");
            if nested_dotbb.is_dir() {
                return Self::open(ancestor.join("blackboard"));
            }

            let nested = ancestor.join("blackboard").join("projects");
            if nested.is_dir() {
                return Self::open(ancestor.join("blackboard"));
            }
        }

        Err(InboxError::RootNotFound(start))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn projects_root(&self) -> &Path {
        &self.projects_root
    }

    pub fn runtime_root(&self) -> PathBuf {
        if self.root.file_name().and_then(|name| name.to_str()) == Some(WORKSPACE_TEMPLATE_DIR) {
            if let Some(parent) = self.root.parent() {
                return parent.join(WORKSPACE_DATA_DIR).join(WORKSPACE_RUNTIME_DIR);
            }
        }
        self.root.join(WORKSPACE_RUNTIME_DIR)
    }

    /// List all projects visible on the current machine.
    pub fn list_projects(&self) -> Result<Vec<ProjectEntry>, InboxError> {
        let mut projects = Vec::new();
        let registry = self.read_projects_registry()?;
        for (name, record) in registry.projects {
            if validate_project_name(&name).is_err() {
                continue;
            }
            let Some(data_root) = self.resolve_registry_data_root_inner(&record, Some(&name))?
            else {
                continue;
            };
            let mut meta = project::read_project_meta(&data_root.join("__project__.json"))?;
            meta.data_root = Some(path_to_string(&data_root));
            projects.push(ProjectEntry {
                name,
                uuid: record.uuid,
                meta,
            });
        }
        projects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(projects)
    }

    pub fn read_project_meta(&self, name: &str) -> Result<ProjectMeta, InboxError> {
        let (_, data_root) = self.resolve_registered_project(name)?;
        let mut meta = project::read_project_meta(&data_root.join("__project__.json"))?;
        meta.data_root = Some(path_to_string(&data_root));
        Ok(meta)
    }

    pub fn create_project_directory(
        &self,
        input: ProjectDirectoryCreate,
    ) -> Result<ProjectEntry, InboxError> {
        let name = normalize_project_name_or_uuid(&input.name);
        validate_project_name(&name)?;
        let data_root = self.resolve_user_data_root(&input.data_root)?;
        let metadata = fs::symlink_metadata(&data_root);
        match metadata {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
                    return Err(InboxError::InvalidInput(format!(
                        "project data root is not a directory: {}",
                        data_root.display()
                    )));
                }
                let has_template = data_root.join("__project__.json").exists()
                    || data_root.join("__tickets__.json").exists()
                    || data_root.join("__inbox__.json").exists();
                if has_template {
                    return Err(InboxError::InvalidInput(
                        "project data root already contains a Blackboard template; use Open instead"
                            .to_string(),
                    ));
                }
                let mut entries = fs::read_dir(&data_root).map_err(|source| InboxError::Io {
                    path: data_root.clone(),
                    source,
                })?;
                if entries
                    .next()
                    .transpose()
                    .map_err(|source| InboxError::Io {
                        path: data_root.clone(),
                        source,
                    })?
                    .is_some()
                {
                    return Err(InboxError::InvalidInput(format!(
                        "project data root is not empty: {}",
                        data_root.display()
                    )));
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir_all(&data_root).map_err(|source| InboxError::Io {
                    path: data_root.clone(),
                    source,
                })?;
            }
            Err(source) => {
                return Err(InboxError::Io {
                    path: data_root.clone(),
                    source,
                });
            }
        }

        let uuid = input.uuid.clone();
        self.ensure_unique_data_root_index(&name, &data_root)?;
        self.ensure_registry_key_available_for(&name, &data_root)?;
        Self::write_project_directory_template(&name, &data_root, input)?;
        self.register_project_data_root(&name, &data_root, uuid.as_deref())?;
        let meta = self.validate_project_directory_template(&data_root)?;
        let uuid = self
            .read_projects_registry()?
            .projects
            .get(&name)
            .map(|record| record.uuid.clone())
            .unwrap_or_default();
        Ok(ProjectEntry { name, uuid, meta })
    }

    pub fn open_project_directory(
        &self,
        input: ProjectDirectoryOpen,
    ) -> Result<ProjectEntry, InboxError> {
        let name = normalize_project_name_or_uuid(&input.name);
        validate_project_name(&name)?;
        let data_root = self.resolve_user_data_root(&input.data_root)?;
        let meta = self.validate_project_directory_template(&data_root)?;
        self.register_project_data_root(&name, &data_root, None)?;
        let uuid = self
            .read_projects_registry()?
            .projects
            .get(&name)
            .map(|record| record.uuid.clone())
            .unwrap_or_default();
        Ok(ProjectEntry { name, uuid, meta })
    }

    pub fn init_project_capsule(
        folder_root: impl AsRef<Path>,
        project_name: &str,
        display_name: Option<&str>,
    ) -> Result<ProjectEntry, InboxError> {
        let folder_root = folder_root.as_ref();
        let metadata = fs::symlink_metadata(folder_root).map_err(|source| InboxError::Io {
            path: folder_root.to_path_buf(),
            source,
        })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
            return Err(InboxError::InvalidInput(format!(
                "project folder root is not a directory: {}",
                folder_root.display()
            )));
        }
        if folder_root.join(WORKSPACE_DATA_DIR).exists() {
            return Err(InboxError::InvalidInput(format!(
                "folder already contains {WORKSPACE_DATA_DIR}; open it instead: {}",
                folder_root.display()
            )));
        }

        let folder_root =
            canonicalize_existing_dir(folder_root).map_err(|source| InboxError::Io {
                path: folder_root.to_path_buf(),
                source,
            })?;
        let name = normalize_project_name_or_uuid(project_name);
        validate_project_name(&name)?;
        let display_name = display_name
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let data_root = folder_root.join(WORKSPACE_DATA_DIR);
        fs::create_dir_all(&data_root).map_err(|source| InboxError::Io {
            path: data_root.clone(),
            source,
        })?;
        Self::write_project_directory_template(
            &name,
            &data_root,
            ProjectDirectoryCreate {
                name: name.clone(),
                data_root: path_to_string(&data_root),
                uuid: None,
                display_name,
                kind: "tool".to_string(),
                repos: vec![path_to_string(&folder_root)],
                description: None,
            },
        )?;
        let mut meta = project::read_project_meta(&data_root.join("__project__.json"))?;
        meta.data_root = Some(path_to_string(&data_root));

        Ok(ProjectEntry {
            name,
            uuid: String::new(),
            meta,
        })
    }

    pub fn migrate_legacy_folder_workspace_to_capsule(
        folder_root: impl AsRef<Path>,
    ) -> Result<Option<ProjectEntry>, InboxError> {
        let folder_root = folder_root.as_ref();
        let data_root = folder_root.join(WORKSPACE_DATA_DIR);
        if data_root.join("__project__.json").exists() {
            return Ok(None);
        }
        if !data_root
            .join("projects")
            .join("__projects__.json")
            .is_file()
        {
            return Ok(None);
        }

        let legacy_workspace = Self::open(folder_root)?;
        let projects = legacy_workspace.list_projects()?;
        if projects.len() != 1 {
            return Err(InboxError::InvalidInput(format!(
                "legacy folder workspace at {} contains {} projects; open a single-project folder or migrate it manually",
                data_root.display(),
                projects.len()
            )));
        }
        let project = projects.into_iter().next().expect("checked len");
        let Some(source_root) = project.meta.data_root.as_deref() else {
            return Err(InboxError::InvalidProjectMeta {
                path: data_root.join("projects").join("__projects__.json"),
                message: format!("project `{}` is missing data_root", project.name),
            });
        };
        let source_root = PathBuf::from(source_root);
        for filename in ["__project__.json", "__tickets__.json", "__inbox__.json"] {
            let source = source_root.join(filename);
            let target = data_root.join(filename);
            if source.is_file() && !target.exists() {
                fs::copy(&source, &target).map_err(|source_err| InboxError::Io {
                    path: target,
                    source: source_err,
                })?;
            }
        }
        for dirname in ["inbox", "tickets", "wiki"] {
            let source = source_root.join(dirname);
            if source.is_dir() {
                copy_dir_missing(&source, &data_root.join(dirname))?;
            }
        }

        let mut meta = project::read_project_meta(&data_root.join("__project__.json"))?;
        meta.data_root = Some(path_to_string(&data_root));
        Ok(Some(ProjectEntry {
            name: project.name,
            uuid: project.uuid,
            meta,
        }))
    }

    pub fn inspect_project_directory(&self, data_root: &str) -> Result<ProjectMeta, InboxError> {
        let data_root = self.resolve_user_data_root(data_root)?;
        self.validate_project_directory_template(&data_root)
    }

    pub fn registered_project_for_data_root(
        &self,
        data_root: &str,
    ) -> Result<Option<String>, InboxError> {
        let data_root = self.resolve_user_data_root(data_root)?;
        self.find_project_index_for_data_root(&data_root)
    }

    pub fn list_agents(&self) -> Result<agents_registry::AgentRegistryList, InboxError> {
        agents_registry::list_agents(&self.root)
    }

    pub fn list_project_agents(
        &self,
        name: &str,
    ) -> Result<agents_registry::ProjectAgentList, InboxError> {
        let validated = validate_project_name(name)?;
        agents_registry::list_project_agents(&self.root, validated)
    }

    /// Open a specific project by validated name.
    pub fn open_project(&self, name: &str) -> Result<Blackboard, InboxError> {
        let validated = validate_project_name(name)?;
        let (_, data_root) = self.resolve_registered_project(validated)?;
        let mut board = Blackboard::open_with_name(data_root, validated)?;
        board.workspace_root = Some(self.root.clone());
        Ok(board)
    }

    fn projects_registry_path(&self) -> PathBuf {
        self.projects_root.join("__projects__.json")
    }

    fn resolve_data_root(requested_root: &Path) -> Result<PathBuf, InboxError> {
        if requested_root
            .join(WORKSPACE_TEMPLATE_DIR)
            .join("projects")
            .is_dir()
        {
            return canonicalize_existing_dir(&requested_root.join(WORKSPACE_TEMPLATE_DIR))
                .map_err(|source| InboxError::Io {
                    path: requested_root.join(WORKSPACE_TEMPLATE_DIR),
                    source,
                });
        }

        if requested_root
            .join(WORKSPACE_DATA_DIR)
            .join("projects")
            .is_dir()
        {
            return canonicalize_existing_dir(&requested_root.join(WORKSPACE_DATA_DIR)).map_err(
                |source| InboxError::Io {
                    path: requested_root.join(WORKSPACE_DATA_DIR),
                    source,
                },
            );
        }

        if requested_root.join("projects").is_dir() {
            return Ok(requested_root.to_path_buf());
        }

        Err(InboxError::ProjectsRootMissing(
            requested_root.to_path_buf(),
        ))
    }

    fn target_data_root(requested_root: &Path) -> PathBuf {
        if requested_root.file_name().and_then(|name| name.to_str()) == Some(WORKSPACE_DATA_DIR) {
            requested_root.to_path_buf()
        } else {
            requested_root.join(WORKSPACE_DATA_DIR)
        }
    }

    fn ensure_workspace_layout(&self) -> Result<(), InboxError> {
        fs::create_dir_all(self.runtime_root()).map_err(|source| InboxError::Io {
            path: self.runtime_root(),
            source,
        })?;

        let manifest_path = self.root.join(WORKSPACE_MANIFEST);
        if !manifest_path.exists() {
            let manifest = WorkspaceManifest {
                schema_version: WORKSPACE_SCHEMA_VERSION,
                layout: "blackboard-dotbb".to_string(),
            };
            write_json_pretty(&manifest_path, &manifest)?;
        }
        Ok(())
    }

    fn ensure_projects_registry(&self) -> Result<(), InboxError> {
        let path = self.projects_registry_path();
        if path.exists() {
            let mut registry = self.read_projects_registry()?;
            self.refresh_current_machine(&mut registry);
            self.clean_registry_paths(&mut registry);
            return self.write_projects_registry(&registry);
        }

        let mut registry = ProjectsRegistry {
            projects: BTreeMap::new(),
            machines: BTreeMap::new(),
        };
        self.refresh_current_machine(&mut registry);
        if self.projects_root.join("blackboard").is_dir() {
            registry.projects.insert(
                "blackboard".to_string(),
                ProjectRegistryEntry {
                    uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                    locations: BTreeMap::from([(
                        "CURRENT".to_string(),
                        ProjectLocation {
                            absolute_path: String::new(),
                            relative_path: "./blackboard".to_string(),
                        },
                    )]),
                },
            );
        }
        self.write_projects_registry(&registry)
    }

    fn clean_registry_paths(&self, registry: &mut ProjectsRegistry) {
        for project in registry.projects.values_mut() {
            for location in project.locations.values_mut() {
                location.absolute_path = clean_path_string(&location.absolute_path);
            }
        }
    }

    pub(crate) fn read_projects_registry(&self) -> Result<ProjectsRegistry, InboxError> {
        read_json_file(&self.projects_registry_path())
    }

    pub(crate) fn write_projects_registry(
        &self,
        registry: &ProjectsRegistry,
    ) -> Result<(), InboxError> {
        write_json_pretty(&self.projects_registry_path(), registry)
    }

    fn refresh_current_machine(&self, registry: &mut ProjectsRegistry) {
        let host = machine_host_name().unwrap_or_else(|| "UNKNOWN".to_string());
        registry.machines.insert(
            "CURRENT".to_string(),
            MachineRegistryEntry::Current(host.clone()),
        );
        self.record_current_machine(registry, &host);
    }

    fn record_current_machine(&self, registry: &mut ProjectsRegistry, host: &str) {
        match registry.machines.entry(host.to_string()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(MachineRegistryEntry::Info(MachineInfo {
                    os: current_os_name().to_string(),
                    os_version: current_os_version().unwrap_or_default(),
                }));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if let MachineRegistryEntry::Info(info) = entry.get_mut() {
                    if info.os.trim().is_empty() {
                        info.os = current_os_name().to_string();
                    }
                    if info.os_version.trim().is_empty() {
                        info.os_version = current_os_version().unwrap_or_default();
                    }
                    return;
                }
                entry.insert(MachineRegistryEntry::Info(MachineInfo {
                    os: current_os_name().to_string(),
                    os_version: current_os_version().unwrap_or_default(),
                }));
            }
        }
    }

    fn current_machine_name(&self, registry: &ProjectsRegistry) -> Option<String> {
        match registry.machines.get("CURRENT") {
            Some(MachineRegistryEntry::Current(name)) if !name.trim().is_empty() => {
                Some(name.trim().to_string())
            }
            _ => machine_host_name(),
        }
    }

    fn resolve_registered_project(&self, name: &str) -> Result<(String, PathBuf), InboxError> {
        let validated = validate_project_name(name)?.to_string();
        let registry = self.read_projects_registry()?;
        let record = registry
            .projects
            .get(&validated)
            .ok_or_else(|| InboxError::ProjectNotFound(validated.clone()))?;
        let data_root = self
            .resolve_registry_data_root_inner(record, Some(&validated))?
            .ok_or_else(|| InboxError::ProjectNotFound(validated.clone()))?;
        Ok((record.uuid.clone(), data_root))
    }

    fn resolve_registry_data_root(
        &self,
        record: &ProjectRegistryEntry,
    ) -> Result<Option<PathBuf>, InboxError> {
        self.resolve_registry_data_root_inner(record, None)
    }

    fn resolve_registry_data_root_inner(
        &self,
        record: &ProjectRegistryEntry,
        persist_for_project: Option<&str>,
    ) -> Result<Option<PathBuf>, InboxError> {
        let registry = self.read_projects_registry()?;
        let current_machine = self.current_machine_name(&registry);
        let location = current_machine
            .as_deref()
            .and_then(|name| record.locations.get(name))
            .or_else(|| record.locations.get("CURRENT"));
        if let Some(location) = location {
            if let Some(data_root) = self.resolve_project_location(location)? {
                return Ok(Some(data_root));
            }
        }
        for location in record.locations.values() {
            if let Some(resolved) = self.resolve_project_location(location)? {
                if let Some(project_name) = persist_for_project {
                    if let Some(host) = &current_machine {
                        let relative = location.relative_path.trim();
                        let _ =
                            self.persist_fallback_location(project_name, host, &resolved, relative);
                    }
                }
                return Ok(Some(resolved));
            }
        }
        Ok(None)
    }

    fn persist_fallback_location(
        &self,
        project_name: &str,
        machine_name: &str,
        resolved_absolute: &Path,
        relative_path: &str,
    ) -> Result<(), InboxError> {
        let mut registry = self.read_projects_registry()?;
        if let Some(entry) = registry.projects.get_mut(project_name) {
            entry.locations.insert(
                machine_name.to_string(),
                ProjectLocation {
                    absolute_path: path_to_string(resolved_absolute),
                    relative_path: relative_path.to_string(),
                },
            );
            self.write_projects_registry(&registry)?;
        }
        Ok(())
    }

    fn resolve_project_location(
        &self,
        location: &ProjectLocation,
    ) -> Result<Option<PathBuf>, InboxError> {
        let absolute = location.absolute_path.trim();
        if !absolute.is_empty() {
            let candidate = self.expand_user_path(absolute);
            if candidate.exists() {
                return canonicalize_existing_dir(&candidate)
                    .map(Some)
                    .map_err(|source| InboxError::InvalidProjectMeta {
                        path: self.projects_registry_path(),
                        message: format!(
                            "project absolute_path `{absolute}` cannot be resolved: {source}"
                        ),
                    });
            }
        }

        let relative = location.relative_path.trim();
        if !relative.is_empty() {
            let candidate = self.projects_root.join(relative);
            if !candidate.exists() {
                return Ok(None);
            }
            return canonicalize_existing_dir(&candidate)
                .map(Some)
                .map_err(|source| InboxError::InvalidProjectMeta {
                    path: self.projects_registry_path(),
                    message: format!(
                        "project relative_path `{relative}` cannot be resolved: {source}"
                    ),
                });
        }

        if !absolute.is_empty() {
            return Ok(None);
        }

        Err(InboxError::InvalidProjectMeta {
            path: self.projects_registry_path(),
            message: "project location has neither absolute_path nor relative_path".to_string(),
        })
    }

    fn expand_user_path(&self, raw_path: &str) -> PathBuf {
        if let Some(rest) = raw_path.strip_prefix("~/") {
            if let Some(home) = user_home_dir() {
                return home.join(rest);
            }
        }
        PathBuf::from(raw_path)
    }

    fn resolve_user_data_root(&self, raw_root: &str) -> Result<PathBuf, InboxError> {
        let raw_root = raw_root.trim();
        if raw_root.is_empty() {
            return Err(InboxError::InvalidInput(
                "project data root path is required".to_string(),
            ));
        }
        let path = Path::new(raw_root);
        Ok(if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.root.join(path)
        })
    }

    fn write_project_directory_template(
        name: &str,
        data_root: &Path,
        input: ProjectDirectoryCreate,
    ) -> Result<(), InboxError> {
        fs::create_dir_all(data_root.join("inbox")).map_err(|source| InboxError::Io {
            path: data_root.join("inbox"),
            source,
        })?;
        fs::create_dir_all(data_root.join("tickets")).map_err(|source| InboxError::Io {
            path: data_root.join("tickets"),
            source,
        })?;
        fs::create_dir_all(data_root.join("wiki")).map_err(|source| InboxError::Io {
            path: data_root.join("wiki"),
            source,
        })?;

        let display_name = input
            .display_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(name)
            .to_string();
        let kind = input.kind.trim();
        let kind = if kind.is_empty() { "tool" } else { kind };
        let meta = ProjectMeta {
            name: display_name,
            kind: kind.to_string(),
            repos: input.repos,
            description: input.description,
            data_root: None,
            local_host: None,
            lanes: default_project_lanes(),
            board_view: ProjectBoardViewSettings::default(),
        };
        write_json_pretty(&data_root.join("__project__.json"), &meta)?;

        let tickets = TicketList {
            current_counter: "000000".to_string(),
            tickets: Vec::new(),
        };
        write_json_pretty(&data_root.join("__tickets__.json"), &tickets)?;

        let inbox = InboxIndex { notes: Vec::new() };
        write_json_pretty(&data_root.join("__inbox__.json"), &inbox)
    }

    fn validate_project_directory_template(
        &self,
        data_root: &Path,
    ) -> Result<ProjectMeta, InboxError> {
        let data_root = canonicalize_existing_dir(data_root).map_err(|source| InboxError::Io {
            path: data_root.to_path_buf(),
            source,
        })?;
        for dirname in ["inbox", "tickets", "wiki"] {
            let path = data_root.join(dirname);
            canonicalize_existing_dir(&path).map_err(|source| InboxError::InvalidProjectMeta {
                path: data_root.join("__project__.json"),
                message: format!("required directory `{dirname}` is missing or invalid: {source}"),
            })?;
        }

        let meta = project::read_project_meta(&data_root.join("__project__.json"))?;
        read_json_file::<TicketList>(&data_root.join("__tickets__.json"))?;
        read_json_file::<InboxIndex>(&data_root.join("__inbox__.json"))?;
        Ok(meta)
    }

    fn register_project_data_root(
        &self,
        name: &str,
        data_root: &Path,
        requested_uuid: Option<&str>,
    ) -> Result<(), InboxError> {
        self.ensure_unique_data_root_index(name, data_root)?;
        self.ensure_registry_key_available_for(name, data_root)?;
        self.validate_project_directory_template(data_root)?;
        let mut registry = self.read_projects_registry()?;
        self.refresh_current_machine(&mut registry);
        let host = self
            .current_machine_name(&registry)
            .unwrap_or_else(|| "UNKNOWN".to_string());
        self.record_current_machine(&mut registry, &host);
        let entry = registry
            .projects
            .entry(name.to_string())
            .or_insert_with(|| ProjectRegistryEntry {
                uuid: requested_uuid
                    .filter(|value| !value.trim().is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                locations: BTreeMap::new(),
            });
        entry.locations.insert(
            host,
            ProjectLocation {
                absolute_path: path_to_string(data_root),
                relative_path: self.relative_project_path(data_root).unwrap_or_default(),
            },
        );
        self.write_projects_registry(&registry)
    }

    fn ensure_unique_data_root_index(
        &self,
        name: &str,
        data_root: &Path,
    ) -> Result<(), InboxError> {
        if let Some(indexed_name) = self.find_project_index_for_data_root(data_root)? {
            if indexed_name != name {
                let target =
                    canonicalize_existing_dir(data_root).map_err(|source| InboxError::Io {
                        path: data_root.to_path_buf(),
                        source,
                    })?;
                return Err(InboxError::InvalidInput(format!(
                    "project data root `{}` is already indexed as `{indexed_name}` on this machine",
                    target.display()
                )));
            }
        }
        Ok(())
    }

    fn find_project_index_for_data_root(
        &self,
        data_root: &Path,
    ) -> Result<Option<String>, InboxError> {
        let target = canonicalize_existing_dir(data_root).map_err(|source| InboxError::Io {
            path: data_root.to_path_buf(),
            source,
        })?;
        let registry = self.read_projects_registry()?;
        for (indexed_name, record) in &registry.projects {
            let Some(existing) = self.resolve_registry_data_root(record)? else {
                continue;
            };
            if existing == target {
                return Ok(Some(indexed_name.clone()));
            }
        }
        Ok(None)
    }

    fn ensure_registry_key_available_for(
        &self,
        name: &str,
        data_root: &Path,
    ) -> Result<(), InboxError> {
        let registry = self.read_projects_registry()?;
        let Some(record) = registry.projects.get(name) else {
            return Ok(());
        };
        let Some(existing) = self.resolve_registry_data_root(record)? else {
            return Ok(());
        };
        let target = canonicalize_existing_dir(data_root).map_err(|source| InboxError::Io {
            path: data_root.to_path_buf(),
            source,
        })?;
        if existing == target {
            Ok(())
        } else {
            Err(InboxError::InvalidInput(format!(
                "project name `{name}` is already registered for a different project location `{}`; pick a different name",
                existing.display()
            )))
        }
    }

    fn relative_project_path(&self, data_root: &Path) -> Option<String> {
        let canonical = canonicalize_existing_dir(data_root).ok()?;
        let relative = canonical.strip_prefix(&self.projects_root).ok()?;
        let text = relative.to_string_lossy().replace('\\', "/");
        if text.is_empty() {
            None
        } else {
            Some(format!("./{text}"))
        }
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

fn default_project_lanes() -> Vec<LaneDef> {
    vec![
        LaneDef {
            id: "bbt".to_string(),
            label: "后端".to_string(),
            color: "#0f766e".to_string(),
            description: String::new(),
            status: "active".to_string(),
        },
        LaneDef {
            id: "bbd".to_string(),
            label: "前端".to_string(),
            color: "#2563eb".to_string(),
            description: String::new(),
            status: "active".to_string(),
        },
        LaneDef {
            id: "bbp".to_string(),
            label: "产品".to_string(),
            color: "#7c3aed".to_string(),
            description: String::new(),
            status: "active".to_string(),
        },
        LaneDef {
            id: "bbq".to_string(),
            label: "质量".to_string(),
            color: "#ea580c".to_string(),
            description: String::new(),
            status: "active".to_string(),
        },
    ]
}

fn copy_dir_missing(source: &Path, target: &Path) -> Result<(), InboxError> {
    fs::create_dir_all(target).map_err(|source| InboxError::Io {
        path: target.to_path_buf(),
        source,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| InboxError::Io {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| InboxError::Io {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type().map_err(|source_err| InboxError::Io {
            path: source_path.clone(),
            source: source_err,
        })?;

        if file_type.is_dir() {
            copy_dir_missing(&source_path, &target_path)?;
        } else if file_type.is_file() && !target_path.exists() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|source_err| InboxError::Io {
                    path: parent.to_path_buf(),
                    source: source_err,
                })?;
            }
            fs::copy(&source_path, &target_path).map_err(|source_err| InboxError::Io {
                path: target_path,
                source: source_err,
            })?;
        }
    }
    Ok(())
}
