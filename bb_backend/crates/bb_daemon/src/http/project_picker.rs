use axum::extract::State;
use axum::Json;
use bb_core::{InboxError, ProjectMeta};
use serde::Serialize;
use std::path::PathBuf;

use super::{ApiError, AppState};

#[derive(Debug, Serialize)]
pub(super) struct ProjectDirectorySelection {
    data_root: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ProjectDirectoryInspection {
    data_root: String,
    meta: ProjectMeta,
    indexed_as: Option<String>,
}

pub(super) async fn select_create_project_directory(
) -> Result<Json<ProjectDirectorySelection>, ApiError> {
    let data_root = pick_project_directory("Select a directory for the new Blackboard project")
        .await
        .map_err(ApiError)?;
    Ok(Json(ProjectDirectorySelection {
        data_root: data_root.to_string_lossy().to_string(),
    }))
}

pub(super) async fn select_open_project_directory(
    State(state): State<AppState>,
) -> Result<Json<ProjectDirectoryInspection>, ApiError> {
    let data_root = pick_project_directory("Open an existing Blackboard project directory")
        .await
        .map_err(ApiError)?;
    let data_root = data_root.to_string_lossy().to_string();
    let workspace = state.workspace()?;
    let meta = workspace.inspect_project_directory(&data_root)?;
    let indexed_as = workspace.registered_project_for_data_root(&data_root)?;
    Ok(Json(ProjectDirectoryInspection {
        data_root,
        meta,
        indexed_as,
    }))
}

pub(super) async fn select_workspace_folder(
    State(state): State<AppState>,
) -> Result<Json<super::WorkspaceFolderInspection>, ApiError> {
    let root = pick_project_directory("Open a folder as a Blackboard workspace")
        .await
        .map_err(ApiError)?;
    Ok(Json(super::inspect_workspace_folder_root(&state, &root)?))
}

async fn pick_project_directory(description: &'static str) -> Result<PathBuf, InboxError> {
    tokio::task::spawn_blocking(move || pick_project_directory_blocking(description))
        .await
        .map_err(|err| InboxError::InvalidInput(format!("directory dialog failed: {err}")))?
}

fn pick_project_directory_blocking(description: &str) -> Result<PathBuf, InboxError> {
    #[cfg(target_os = "windows")]
    {
        pick_project_directory_with_common_item_dialog(description)
    }

    #[cfg(target_os = "macos")]
    {
        pick_project_directory_with_osascript(description)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = description;
        Err(InboxError::InvalidInput(
            "directory dialog is not available on this platform yet".to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn pick_project_directory_with_osascript(description: &str) -> Result<PathBuf, InboxError> {
    let script = format!(
        r#"set chosenFolder to choose folder with prompt "{}"
return POSIX path of chosenFolder"#,
        description.replace('"', "\\\"")
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|err| InboxError::InvalidInput(format!("failed to launch osascript: {err}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err(InboxError::InvalidInput(
                "directory selection was cancelled".to_string(),
            ));
        }
        return Err(InboxError::InvalidInput(format!(
            "directory dialog failed: {stderr}"
        )));
    }

    let path_str = String::from_utf8(output.stdout)
        .map_err(|err| InboxError::InvalidInput(format!("invalid path encoding: {err}")))?;
    let path_str = path_str.trim();
    if path_str.is_empty() {
        return Err(InboxError::InvalidInput(
            "directory selection returned empty path".to_string(),
        ));
    }
    Ok(PathBuf::from(path_str))
}

#[cfg(target_os = "windows")]
fn pick_project_directory_with_common_item_dialog(
    description: &str,
) -> Result<PathBuf, InboxError> {
    use windows::core::{HRESULT, HSTRING};
    use windows::Win32::Foundation::ERROR_CANCELLED;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS,
        SIGDN_FILESYSPATH,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    struct ComApartment;

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    let cancelled = HRESULT::from_win32(ERROR_CANCELLED.0);
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|err| {
                InboxError::InvalidInput(format!("directory dialog initialization failed: {err}"))
            })?;
        let _apartment = ComApartment;

        let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
            .map_err(|err| {
                InboxError::InvalidInput(format!("directory dialog creation failed: {err}"))
            })?;
        dialog
            .SetTitle(&HSTRING::from(description))
            .map_err(|err| {
                InboxError::InvalidInput(format!("directory dialog title failed: {err}"))
            })?;

        let options = dialog.GetOptions().map_err(|err| {
            InboxError::InvalidInput(format!("directory dialog options failed: {err}"))
        })?;
        dialog
            .SetOptions(options | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .map_err(|err| {
                InboxError::InvalidInput(format!("directory dialog options failed: {err}"))
            })?;

        let owner = GetForegroundWindow();
        let result = if owner.0.is_null() {
            dialog.Show(None)
        } else {
            dialog.Show(Some(owner))
        };

        if let Err(err) = result {
            if err.code() == cancelled {
                return Err(InboxError::InvalidInput(
                    "directory selection was cancelled".to_string(),
                ));
            }
            return Err(InboxError::InvalidInput(format!(
                "directory selection failed: {err}"
            )));
        }

        let item = dialog.GetResult().map_err(|err| {
            InboxError::InvalidInput(format!("directory dialog result failed: {err}"))
        })?;
        let display_name = item.GetDisplayName(SIGDN_FILESYSPATH).map_err(|err| {
            InboxError::InvalidInput(format!("directory dialog path failed: {err}"))
        })?;
        let selected = display_name.to_string().map_err(|err| {
            InboxError::InvalidInput(format!("directory dialog path failed: {err}"))
        })?;
        Ok(PathBuf::from(selected))
    }
}
