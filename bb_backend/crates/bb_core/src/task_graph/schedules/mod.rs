//! Stateless `TaskGraph` schedule store and time calculation.

use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDateTime, NaiveTime, TimeZone, Utc, Weekday,
};
use chrono_tz::Tz;
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use uuid::Uuid;

use super::types::{TaskGraphError, TaskGraphScope};

const DEFAULT_TIMEZONE: &str = "Asia/Shanghai";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskSchedule {
    pub id: String,
    pub name: String,
    pub project: String,
    pub enabled: bool,
    pub graph_ref: TaskScheduleGraphRef,
    #[serde(default)]
    pub input: serde_json::Value,
    pub schedule: TaskScheduleSpec,
    pub timezone: String,
    pub concurrency_policy: TaskScheduleConcurrencyPolicy,
    pub misfire_policy: TaskScheduleMisfirePolicy,
    pub created_at: String,
    pub updated_at: String,
    pub state: TaskScheduleState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskScheduleGraphRef {
    pub scope: TaskGraphScope,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskScheduleSpec {
    pub kind: TaskScheduleKind,
    pub expression: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskScheduleKind {
    Interval,
    Daily,
    Weekly,
    Cron,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskScheduleConcurrencyPolicy {
    Skip,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskScheduleMisfirePolicy {
    RunOnce,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskScheduleState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_planned_fire_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskScheduleCreate {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub graph_ref: TaskScheduleGraphRef,
    #[serde(default)]
    pub input: serde_json::Value,
    pub schedule: TaskScheduleSpec,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskSchedulePatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub graph_ref: Option<TaskScheduleGraphRef>,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub schedule: Option<TaskScheduleSpec>,
    #[serde(default)]
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FireClaim {
    schedule_id: String,
    project: String,
    planned_fire_at: String,
    claimed_at: String,
}

pub fn list_schedules(
    workspace_root: &Path,
    project: &str,
) -> Result<Vec<TaskSchedule>, TaskGraphError> {
    validate_project(project)?;
    let dir = schedules_dir(workspace_root, project);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut schedules: Vec<TaskSchedule> = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|source| TaskGraphError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| TaskGraphError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            schedules.push(read_json(&path)?);
        }
    }
    schedules.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
    Ok(schedules)
}

pub fn read_schedule(
    workspace_root: &Path,
    project: &str,
    id: &str,
) -> Result<TaskSchedule, TaskGraphError> {
    validate_project(project)?;
    let id = validate_schedule_id(id)?;
    let path = schedule_path(workspace_root, project, id);
    if !path.exists() {
        return Err(TaskGraphError::ScheduleNotFound {
            project: project.to_string(),
            id: id.to_string(),
        });
    }
    read_json(&path)
}

pub fn create_schedule(
    workspace_root: &Path,
    project: &str,
    input: TaskScheduleCreate,
) -> Result<TaskSchedule, TaskGraphError> {
    validate_project(project)?;
    let id = match input.id {
        Some(id) => validate_schedule_id(&id)?.to_string(),
        None => generated_schedule_id(&input.name),
    };
    let path = schedule_path(workspace_root, project, &id);
    if path.exists() {
        return Err(TaskGraphError::DuplicateScheduleId(id));
    }

    let now = Utc::now();
    let mut schedule = TaskSchedule {
        id,
        name: validate_name(&input.name)?.to_string(),
        project: project.to_string(),
        enabled: input.enabled,
        graph_ref: validate_graph_ref(input.graph_ref)?,
        input: if input.input.is_null() {
            serde_json::json!({})
        } else {
            input.input
        },
        schedule: validate_schedule_spec(input.schedule)?,
        timezone: validate_timezone(input.timezone.as_deref().unwrap_or(DEFAULT_TIMEZONE))?,
        concurrency_policy: TaskScheduleConcurrencyPolicy::Skip,
        misfire_policy: TaskScheduleMisfirePolicy::RunOnce,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
        state: TaskScheduleState::default(),
    };
    schedule.state.next_run_at = Some(compute_next_run_after(&schedule, now)?.to_rfc3339());
    write_json_create_new(&path, &schedule)?;
    Ok(schedule)
}

pub fn patch_schedule(
    workspace_root: &Path,
    project: &str,
    id: &str,
    patch: TaskSchedulePatch,
) -> Result<TaskSchedule, TaskGraphError> {
    let mut schedule = read_schedule(workspace_root, project, id)?;
    let now = Utc::now();

    if let Some(name) = patch.name {
        schedule.name = validate_name(&name)?.to_string();
    }
    if let Some(enabled) = patch.enabled {
        schedule.enabled = enabled;
    }
    if let Some(graph_ref) = patch.graph_ref {
        schedule.graph_ref = validate_graph_ref(graph_ref)?;
    }
    if let Some(input) = patch.input {
        schedule.input = if input.is_null() {
            serde_json::json!({})
        } else {
            input
        };
    }
    if let Some(spec) = patch.schedule {
        schedule.schedule = validate_schedule_spec(spec)?;
    }
    if let Some(timezone) = patch.timezone {
        schedule.timezone = validate_timezone(&timezone)?;
    }
    schedule.updated_at = now.to_rfc3339();
    schedule.state.next_run_at = Some(compute_next_run_after(&schedule, now)?.to_rfc3339());
    schedule.state.last_error = None;

    write_json(
        &schedule_path(workspace_root, project, &schedule.id),
        &schedule,
    )?;
    Ok(schedule)
}

pub fn delete_schedule(
    workspace_root: &Path,
    project: &str,
    id: &str,
) -> Result<(), TaskGraphError> {
    validate_project(project)?;
    let id = validate_schedule_id(id)?;
    let path = schedule_path(workspace_root, project, id);
    if !path.exists() {
        return Err(TaskGraphError::ScheduleNotFound {
            project: project.to_string(),
            id: id.to_string(),
        });
    }
    fs::remove_file(&path).map_err(|source| TaskGraphError::Io { path, source })
}

pub fn due_planned_fire_at(
    schedule: &TaskSchedule,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, TaskGraphError> {
    if !schedule.enabled {
        return Ok(None);
    }
    let planned = match schedule.state.next_run_at.as_deref() {
        Some(value) => parse_rfc3339(value)?,
        None => compute_next_run_after(schedule, now)?,
    };
    if planned <= now {
        Ok(Some(planned))
    } else {
        Ok(None)
    }
}

pub fn compute_next_run_after(
    schedule: &TaskSchedule,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, TaskGraphError> {
    match schedule.schedule.kind {
        TaskScheduleKind::Interval => {
            let duration = parse_interval(&schedule.schedule.expression)?;
            Ok(after + duration)
        }
        TaskScheduleKind::Daily => next_daily(schedule, after),
        TaskScheduleKind::Weekly => next_weekly(schedule, after),
        TaskScheduleKind::Cron => next_cron(schedule, after),
    }
}

pub fn claim_schedule_fire(
    workspace_root: &Path,
    project: &str,
    schedule_id: &str,
    planned_fire_at: DateTime<Utc>,
) -> Result<bool, TaskGraphError> {
    validate_project(project)?;
    let schedule_id = validate_schedule_id(schedule_id)?;
    let dir = workspace_root
        .join("runtime")
        .join("task_graph_schedule_claims")
        .join(project)
        .join(schedule_id);
    fs::create_dir_all(&dir).map_err(|source| TaskGraphError::Io {
        path: dir.clone(),
        source,
    })?;
    let path = dir.join(format!(
        "{}.json",
        sanitize_claim_name(&planned_fire_at.to_rfc3339())
    ));
    let claim = FireClaim {
        schedule_id: schedule_id.to_string(),
        project: project.to_string(),
        planned_fire_at: planned_fire_at.to_rfc3339(),
        claimed_at: Utc::now().to_rfc3339(),
    };
    let json = serde_json::to_string_pretty(&claim).expect("claim is serializable");
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(json.as_bytes())
                .map_err(|source| TaskGraphError::Io {
                    path: path.clone(),
                    source,
                })?;
            Ok(true)
        }
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(source) => Err(TaskGraphError::Io { path, source }),
    }
}

pub fn mark_schedule_triggered(
    workspace_root: &Path,
    project: &str,
    schedule_id: &str,
    planned_fire_at: DateTime<Utc>,
    run_id: &str,
    status: &str,
    now: DateTime<Utc>,
) -> Result<TaskSchedule, TaskGraphError> {
    update_schedule_state(workspace_root, project, schedule_id, |schedule| {
        schedule.state.last_run_at = Some(now.to_rfc3339());
        schedule.state.last_run_id = Some(run_id.to_string());
        schedule.state.last_status = Some(status.to_string());
        schedule.state.last_error = None;
        schedule.state.last_planned_fire_at = Some(planned_fire_at.to_rfc3339());
        schedule.state.next_run_at = Some(compute_next_run_after(schedule, now)?.to_rfc3339());
        Ok(())
    })
}

pub fn mark_schedule_skipped(
    workspace_root: &Path,
    project: &str,
    schedule_id: &str,
    planned_fire_at: DateTime<Utc>,
    reason: &str,
    now: DateTime<Utc>,
) -> Result<TaskSchedule, TaskGraphError> {
    update_schedule_state(workspace_root, project, schedule_id, |schedule| {
        schedule.state.last_run_at = Some(now.to_rfc3339());
        schedule.state.last_status = Some("skipped".to_string());
        schedule.state.last_error = Some(reason.to_string());
        schedule.state.last_planned_fire_at = Some(planned_fire_at.to_rfc3339());
        schedule.state.next_run_at = Some(compute_next_run_after(schedule, now)?.to_rfc3339());
        Ok(())
    })
}

pub fn mark_schedule_failed(
    workspace_root: &Path,
    project: &str,
    schedule_id: &str,
    planned_fire_at: DateTime<Utc>,
    error: &str,
    now: DateTime<Utc>,
) -> Result<TaskSchedule, TaskGraphError> {
    update_schedule_state(workspace_root, project, schedule_id, |schedule| {
        schedule.state.last_run_at = Some(now.to_rfc3339());
        schedule.state.last_status = Some("failed".to_string());
        schedule.state.last_error = Some(error.to_string());
        schedule.state.last_planned_fire_at = Some(planned_fire_at.to_rfc3339());
        schedule.state.next_run_at = Some(compute_next_run_after(schedule, now)?.to_rfc3339());
        Ok(())
    })
}

pub fn refresh_schedule_last_status(
    workspace_root: &Path,
    project: &str,
    schedule_id: &str,
    status: &str,
) -> Result<TaskSchedule, TaskGraphError> {
    update_schedule_state(workspace_root, project, schedule_id, |schedule| {
        schedule.state.last_status = Some(status.to_string());
        Ok(())
    })
}

fn update_schedule_state<F>(
    workspace_root: &Path,
    project: &str,
    id: &str,
    mut update: F,
) -> Result<TaskSchedule, TaskGraphError>
where
    F: FnMut(&mut TaskSchedule) -> Result<(), TaskGraphError>,
{
    let mut schedule = read_schedule(workspace_root, project, id)?;
    update(&mut schedule)?;
    schedule.updated_at = Utc::now().to_rfc3339();
    write_json(&schedule_path(workspace_root, project, id), &schedule)?;
    Ok(schedule)
}

fn next_daily(
    schedule: &TaskSchedule,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, TaskGraphError> {
    let tz = parse_timezone(&schedule.timezone)?;
    let time = parse_time(&schedule.schedule.expression)?;
    let local_after = after.with_timezone(&tz);
    for offset in 0..=370 {
        let date = local_after.date_naive() + Duration::days(offset);
        if let Some(candidate) = local_to_utc(tz, date.and_time(time)) {
            if candidate > after {
                return Ok(candidate);
            }
        }
    }
    Err(TaskGraphError::InvalidSchedule(
        "unable to resolve next daily run".to_string(),
    ))
}

fn next_weekly(
    schedule: &TaskSchedule,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, TaskGraphError> {
    let tz = parse_timezone(&schedule.timezone)?;
    let (weekday, time) = parse_weekly(&schedule.schedule.expression)?;
    let local_after = after.with_timezone(&tz);
    for offset in 0..=400 {
        let date = local_after.date_naive() + Duration::days(offset);
        if date.weekday() != weekday {
            continue;
        }
        if let Some(candidate) = local_to_utc(tz, date.and_time(time)) {
            if candidate > after {
                return Ok(candidate);
            }
        }
    }
    Err(TaskGraphError::InvalidSchedule(
        "unable to resolve next weekly run".to_string(),
    ))
}

fn next_cron(
    schedule: &TaskSchedule,
    after: DateTime<Utc>,
) -> Result<DateTime<Utc>, TaskGraphError> {
    let tz = parse_timezone(&schedule.timezone)?;
    let expression = normalize_cron_expression(&schedule.schedule.expression)?;
    let cron = Schedule::from_str(&expression)
        .map_err(|err| TaskGraphError::InvalidSchedule(format!("invalid cron: {err}")))?;
    let local_after = after.with_timezone(&tz);
    cron.after(&local_after)
        .next()
        .map(|next| next.with_timezone(&Utc))
        .ok_or_else(|| TaskGraphError::InvalidSchedule("cron has no next run".to_string()))
}

fn local_to_utc(tz: Tz, local: NaiveDateTime) -> Option<DateTime<Utc>> {
    match tz.from_local_datetime(&local) {
        LocalResult::Single(value) => Some(value.with_timezone(&Utc)),
        LocalResult::Ambiguous(a, b) => Some(std::cmp::min(a, b).with_timezone(&Utc)),
        LocalResult::None => None,
    }
}

#[allow(clippy::option_if_let_else)]
fn parse_interval(expression: &str) -> Result<Duration, TaskGraphError> {
    let expression = expression.trim().to_ascii_lowercase();
    let (digits, multiplier) = if let Some(value) = expression.strip_suffix('s') {
        (value, 1)
    } else if let Some(value) = expression.strip_suffix('m') {
        (value, 60)
    } else if let Some(value) = expression.strip_suffix('h') {
        (value, 60 * 60)
    } else if let Some(value) = expression.strip_suffix('d') {
        (value, 60 * 60 * 24)
    } else {
        (expression.as_str(), 1)
    };
    let count: i64 = digits
        .trim()
        .parse()
        .map_err(|_| TaskGraphError::InvalidSchedule(format!("invalid interval: {expression}")))?;
    if count <= 0 {
        return Err(TaskGraphError::InvalidSchedule(
            "interval must be positive".to_string(),
        ));
    }
    Ok(Duration::seconds(count.saturating_mul(multiplier)))
}

fn parse_time(expression: &str) -> Result<NaiveTime, TaskGraphError> {
    let expression = expression.trim();
    NaiveTime::parse_from_str(expression, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(expression, "%H:%M:%S"))
        .map_err(|_| TaskGraphError::InvalidSchedule(format!("invalid time: {expression}")))
}

fn parse_weekly(expression: &str) -> Result<(Weekday, NaiveTime), TaskGraphError> {
    let mut parts = expression.split_whitespace();
    let day = parts
        .next()
        .ok_or_else(|| TaskGraphError::InvalidSchedule("weekly day is required".to_string()))?;
    let time = parts
        .next()
        .ok_or_else(|| TaskGraphError::InvalidSchedule("weekly time is required".to_string()))?;
    if parts.next().is_some() {
        return Err(TaskGraphError::InvalidSchedule(
            "weekly expression must be '<day> <HH:MM>'".to_string(),
        ));
    }
    Ok((parse_weekday(day)?, parse_time(time)?))
}

fn parse_weekday(value: &str) -> Result<Weekday, TaskGraphError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "mon" | "monday" | "1" => Ok(Weekday::Mon),
        "tue" | "tuesday" | "2" => Ok(Weekday::Tue),
        "wed" | "wednesday" | "3" => Ok(Weekday::Wed),
        "thu" | "thursday" | "4" => Ok(Weekday::Thu),
        "fri" | "friday" | "5" => Ok(Weekday::Fri),
        "sat" | "saturday" | "6" => Ok(Weekday::Sat),
        "sun" | "sunday" | "0" | "7" => Ok(Weekday::Sun),
        other => Err(TaskGraphError::InvalidSchedule(format!(
            "invalid weekday: {other}"
        ))),
    }
}

fn normalize_cron_expression(expression: &str) -> Result<String, TaskGraphError> {
    match expression.split_whitespace().count() {
        5 => Ok(format!("0 {} *", expression.trim())),
        6 => Ok(format!("{} *", expression.trim())),
        7 => Ok(expression.trim().to_string()),
        _ => Err(TaskGraphError::InvalidSchedule(
            "cron must have 5, 6, or 7 fields".to_string(),
        )),
    }
}

fn parse_timezone(value: &str) -> Result<Tz, TaskGraphError> {
    value
        .parse::<Tz>()
        .map_err(|_| TaskGraphError::InvalidSchedule(format!("invalid timezone: {value}")))
}

fn validate_timezone(value: &str) -> Result<String, TaskGraphError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(TaskGraphError::InvalidSchedule(
            "timezone is required".to_string(),
        ));
    }
    let _ = parse_timezone(value)?;
    Ok(value.to_string())
}

fn validate_schedule_spec(spec: TaskScheduleSpec) -> Result<TaskScheduleSpec, TaskGraphError> {
    if spec.expression.trim().is_empty() {
        return Err(TaskGraphError::InvalidSchedule(
            "schedule expression is required".to_string(),
        ));
    }
    let probe = TaskSchedule {
        id: "probe".to_string(),
        name: "probe".to_string(),
        project: "probe".to_string(),
        enabled: true,
        graph_ref: TaskScheduleGraphRef {
            scope: TaskGraphScope::Project,
            id: "probe".to_string(),
        },
        input: serde_json::json!({}),
        schedule: spec.clone(),
        timezone: DEFAULT_TIMEZONE.to_string(),
        concurrency_policy: TaskScheduleConcurrencyPolicy::Skip,
        misfire_policy: TaskScheduleMisfirePolicy::RunOnce,
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
        state: TaskScheduleState::default(),
    };
    let _ = compute_next_run_after(&probe, Utc::now())?;
    Ok(TaskScheduleSpec {
        kind: spec.kind,
        expression: spec.expression.trim().to_string(),
    })
}

fn validate_graph_ref(
    graph_ref: TaskScheduleGraphRef,
) -> Result<TaskScheduleGraphRef, TaskGraphError> {
    if graph_ref.id.trim().is_empty() || graph_ref.id.trim() != graph_ref.id {
        return Err(TaskGraphError::InvalidSchedule(
            "graph_ref.id is required".to_string(),
        ));
    }
    Ok(graph_ref)
}

fn validate_name(name: &str) -> Result<&str, TaskGraphError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed != name {
        return Err(TaskGraphError::InvalidSchedule(
            "schedule name is required and must not have surrounding whitespace".to_string(),
        ));
    }
    Ok(trimmed)
}

fn validate_project(project: &str) -> Result<&str, TaskGraphError> {
    crate::validate_project_name(project).map_err(|err| {
        TaskGraphError::InvalidSchedule(format!("invalid project `{project}`: {err}"))
    })
}

fn validate_schedule_id(id: &str) -> Result<&str, TaskGraphError> {
    let trimmed = id.trim();
    if trimmed != id
        || trimmed.len() < 2
        || trimmed.len() > 80
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        || !trimmed
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
    {
        return Err(TaskGraphError::InvalidSchedule(format!(
            "invalid schedule id: {id}"
        )));
    }
    Ok(trimmed)
}

fn generated_schedule_id(name: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in name.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.len() >= 2 { slug } else { "schedule" };
    format!("{}-{}", slug, &Uuid::new_v4().to_string()[..8])
}

fn sanitize_claim_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn schedules_dir(workspace_root: &Path, project: &str) -> PathBuf {
    workspace_root
        .join("runtime")
        .join("task_graph_schedules")
        .join(project)
}

fn schedule_path(workspace_root: &Path, project: &str, id: &str) -> PathBuf {
    schedules_dir(workspace_root, project).join(format!("{id}.json"))
}

fn parse_rfc3339(value: &str) -> Result<DateTime<Utc>, TaskGraphError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| TaskGraphError::InvalidSchedule(format!("invalid timestamp: {err}")))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), TaskGraphError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(value).expect("schedule is serializable");
    let tmp = path.with_extension(format!("json.tmp-{}", &Uuid::new_v4().to_string()[..8]));
    fs::write(&tmp, json.as_bytes()).map_err(|source| TaskGraphError::Io {
        path: tmp.clone(),
        source,
    })?;
    fs::rename(&tmp, path).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_json_create_new<T: Serialize>(path: &Path, value: &T) -> Result<(), TaskGraphError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| TaskGraphError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(value).expect("schedule is serializable");
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file
            .write_all(json.as_bytes())
            .map_err(|source| TaskGraphError::Io {
                path: path.to_path_buf(),
                source,
            }),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(TaskGraphError::DuplicateScheduleId(
                path.file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            ))
        }
        Err(source) => Err(TaskGraphError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, TaskGraphError> {
    let contents = fs::read_to_string(path).map_err(|source| TaskGraphError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&contents).map_err(|source| TaskGraphError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Timelike};
    use tempfile::TempDir;

    fn schedule(kind: TaskScheduleKind, expression: &str) -> TaskSchedule {
        TaskSchedule {
            id: "daily".to_string(),
            name: "Daily".to_string(),
            project: "demo".to_string(),
            enabled: true,
            graph_ref: TaskScheduleGraphRef {
                scope: TaskGraphScope::Project,
                id: "graph".to_string(),
            },
            input: serde_json::json!({}),
            schedule: TaskScheduleSpec {
                kind,
                expression: expression.to_string(),
            },
            timezone: DEFAULT_TIMEZONE.to_string(),
            concurrency_policy: TaskScheduleConcurrencyPolicy::Skip,
            misfire_policy: TaskScheduleMisfirePolicy::RunOnce,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            state: TaskScheduleState::default(),
        }
    }

    #[test]
    fn interval_next_run_adds_duration() {
        let item = schedule(TaskScheduleKind::Interval, "15m");
        let after = Utc.with_ymd_and_hms(2026, 5, 15, 1, 0, 0).unwrap();
        let next = compute_next_run_after(&item, after).unwrap();
        assert_eq!(next, Utc.with_ymd_and_hms(2026, 5, 15, 1, 15, 0).unwrap());
    }

    #[test]
    fn daily_next_run_uses_timezone() {
        let item = schedule(TaskScheduleKind::Daily, "09:00");
        let after = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let next = compute_next_run_after(&item, after).unwrap();
        assert_eq!(next.hour(), 1);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn weekly_next_run_uses_named_day() {
        let item = schedule(TaskScheduleKind::Weekly, "mon 09:00");
        let after = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let next = compute_next_run_after(&item, after).unwrap();
        assert_eq!(next.weekday(), Weekday::Mon);
        assert_eq!(next.hour(), 1);
    }

    #[test]
    fn cron_next_run_accepts_five_field_expression() {
        let item = schedule(TaskScheduleKind::Cron, "0 9 * * *");
        let after = Utc.with_ymd_and_hms(2026, 5, 15, 0, 0, 0).unwrap();
        let next = compute_next_run_after(&item, after).unwrap();
        assert_eq!(next.hour(), 1);
    }

    #[test]
    fn due_schedule_returns_planned_time_once_enabled() {
        let mut item = schedule(TaskScheduleKind::Interval, "15m");
        item.state.next_run_at = Some("2026-05-15T01:00:00Z".to_string());
        let now = Utc.with_ymd_and_hms(2026, 5, 15, 1, 5, 0).unwrap();
        let due = due_planned_fire_at(&item, now).unwrap().unwrap();
        assert_eq!(due, Utc.with_ymd_and_hms(2026, 5, 15, 1, 0, 0).unwrap());
        item.enabled = false;
        assert!(due_planned_fire_at(&item, now).unwrap().is_none());
    }

    #[test]
    fn claim_schedule_fire_is_idempotent() {
        let temp = TempDir::new().unwrap();
        let planned = Utc.with_ymd_and_hms(2026, 5, 15, 1, 0, 0).unwrap();
        assert!(claim_schedule_fire(temp.path(), "demo", "schedule-a", planned).unwrap());
        assert!(!claim_schedule_fire(temp.path(), "demo", "schedule-a", planned).unwrap());
    }
}
