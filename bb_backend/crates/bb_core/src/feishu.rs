//! 飞书自定义机器人 Webhook 终态通知（088）。
//!
//! 纯逻辑模块：配置解析 + 卡片构造 + blocking 发送。不依赖 daemon/tokio。
//! 线程调度与 run_mode 判定由调用方（bb_daemon / bb_cli）负责。
//!
//! 数据外发白名单（Spec Arena v22 atk-004 决策）：卡片只外发
//! `project / graph_id / run_id / 终态标签 / elapsed`，可选 detail 链接；
//! 不外发 `RunOutcome::Failed.message` 原文或内部 node_id。

use std::io::Read;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};

/// `[feishu]` TOML section（由 `RunnerConfig` 解析后传入）。全部可选。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct FeishuTomlConfig {
    pub enabled: Option<bool>,
    pub webhook_url: Option<String>,
    pub web_base_url: Option<String>,
    pub connect_timeout_secs: Option<u64>,
    pub read_timeout_secs: Option<u64>,
}

/// 已解析、可用于发送的飞书配置。
#[derive(Debug, Clone)]
pub struct FeishuConfig {
    pub enabled: bool,
    pub webhook_url: String,
    pub web_base_url: Option<String>,
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
}

/// 配置错误（绝不携带原始 URL）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeishuConfigError {
    EmptyWebhookUrl,
    InvalidWebhookUrl,
}

impl std::fmt::Display for FeishuConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyWebhookUrl => write!(f, "feishu webhook_url is empty"),
            Self::InvalidWebhookUrl => write!(f, "feishu webhook_url is invalid (<redacted>)"),
        }
    }
}

impl std::error::Error for FeishuConfigError {}

/// 发送错误（脱敏；不含完整 URL）。
#[derive(Debug)]
pub enum FeishuSendError {
    Network,
    ReadBody,
    HttpStatus(u16),
    ParseBody,
    Business { code: i64, msg: String },
    BuildClient,
    BodyTooLarge { limit: usize },
}

impl std::fmt::Display for FeishuSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network => write!(f, "network error sending feishu webhook"),
            Self::ReadBody => write!(f, "failed to read feishu response body"),
            Self::HttpStatus(code) => write!(f, "feishu webhook http status {code}"),
            Self::ParseBody => write!(f, "feishu response body is not valid JSON"),
            Self::Business { code, msg } => {
                write!(f, "feishu business error code={code} msg={msg}")
            }
            Self::BuildClient => write!(f, "failed to build feishu http client"),
            Self::BodyTooLarge { limit } => {
                write!(f, "feishu response body exceeds {limit} bytes")
            }
        }
    }
}

impl std::error::Error for FeishuSendError {}

/// 终态标签（白名单：不携带 message / node_id）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeishuTerminal {
    Succeeded,
    Paused,
    Failed,
}

impl FeishuTerminal {
    const fn label(self) -> &'static str {
        match self {
            Self::Succeeded => "Succeeded",
            Self::Paused => "Paused",
            Self::Failed => "Failed",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Succeeded => "✅ TaskGraph Run 完成",
            Self::Paused => "⏸ TaskGraph Run 暂停",
            Self::Failed => "❌ TaskGraph Run 失败",
        }
    }

    const fn template(self) -> &'static str {
        match self {
            Self::Succeeded => "green",
            Self::Paused => "orange",
            Self::Failed => "red",
        }
    }
}

/// 通知载荷（白名单字段，由调用方从 RunOutcome 映射后构造）。
#[derive(Debug, Clone)]
pub struct FeishuNotification {
    pub project: String,
    pub graph_id: String,
    pub run_id: String,
    pub terminal: FeishuTerminal,
    pub elapsed: Duration,
    /// 可选详情页完整 URL（仅当 web_base_url 已配置且合法时由调用方填入）。
    pub detail_url: Option<String>,
}

const BODY_LIMIT: usize = 64 * 1024;

fn read_env_or_toml(env_name: &str, toml_value: Option<&str>) -> Option<String> {
    if let Ok(value) = std::env::var(env_name) {
        if !value.trim().is_empty() {
            return Some(value);
        }
    }
    toml_value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn read_env_bool_or_toml(env_name: &str, toml_value: Option<bool>) -> Option<bool> {
    if let Ok(value) = std::env::var(env_name) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ));
        }
    }
    toml_value
}

/// 规范化 web_base_url（atk-001：与 webhook 校验完全独立）。
/// 仅接受 http/https、path∈{"","/"}、无 query/fragment 的 origin；否则返回 None。
fn normalize_web_base_url(raw: Option<&str>) -> Option<String> {
    let raw = raw.map(str::trim).filter(|value| !value.is_empty())?;
    let url = reqwest::Url::parse(raw).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return None;
    }
    Some(url.as_str().trim_end_matches('/').to_string())
}

impl FeishuConfig {
    /// 从已解析的 `[feishu]` section + `BB_FEISHU_*` env 构造。
    /// enabled=false 时返回 `Ok(None)`，不校验 webhook。
    pub fn from_toml_and_env(
        cfg: Option<&FeishuTomlConfig>,
    ) -> Result<Option<Self>, FeishuConfigError> {
        let enabled = read_env_bool_or_toml("BB_FEISHU_ENABLED", cfg.and_then(|c| c.enabled))
            .unwrap_or(false);
        if !enabled {
            return Ok(None);
        }

        let webhook_url = read_env_or_toml(
            "BB_FEISHU_WEBHOOK_URL",
            cfg.and_then(|c| c.webhook_url.as_deref()),
        )
        .unwrap_or_default();
        if webhook_url.trim().is_empty() {
            return Err(FeishuConfigError::EmptyWebhookUrl);
        }
        let parsed =
            reqwest::Url::parse(&webhook_url).map_err(|_| FeishuConfigError::InvalidWebhookUrl)?;
        if parsed.scheme() != "https" {
            return Err(FeishuConfigError::InvalidWebhookUrl);
        }

        let web_base_url = normalize_web_base_url(
            read_env_or_toml(
                "BB_FEISHU_WEB_BASE_URL",
                cfg.and_then(|c| c.web_base_url.as_deref()),
            )
            .as_deref(),
        );

        let connect_timeout =
            Duration::from_secs(cfg.and_then(|c| c.connect_timeout_secs).unwrap_or(3));
        let read_timeout = Duration::from_secs(cfg.and_then(|c| c.read_timeout_secs).unwrap_or(10));

        Ok(Some(Self {
            enabled,
            webhook_url,
            web_base_url,
            connect_timeout,
            read_timeout,
        }))
    }

    /// 详情链接：仅在 web_base_url 合法时返回。
    pub fn detail_url(&self, project: &str, run_id: &str) -> Option<String> {
        self.web_base_url
            .as_ref()
            .map(|base| format!("{base}/project/{project}/runs/{run_id}"))
    }
}

fn humanize_elapsed(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

/// 构造互动卡片 JSON（只使用白名单字段）。
pub fn build_card(_cfg: &FeishuConfig, n: &FeishuNotification) -> Value {
    let mut content = format!(
        "**项目**: {}\n**任务**: {}\n**Run**: {}\n**状态**: {}\n**耗时**: {}",
        n.project,
        n.graph_id,
        n.run_id,
        n.terminal.label(),
        humanize_elapsed(n.elapsed),
    );
    let _ = &mut content;

    let mut elements = vec![json!({
        "tag": "div",
        "text": { "tag": "lark_md", "content": content }
    })];

    if let Some(url) = &n.detail_url {
        elements.push(json!({
            "tag": "action",
            "actions": [{
                "tag": "button",
                "text": { "tag": "plain_text", "content": "查看详情" },
                "type": "primary",
                "url": url
            }]
        }));
    }

    json!({
        "config": { "wide_screen_mode": true },
        "header": {
            "title": { "tag": "plain_text", "content": n.terminal.title() },
            "template": n.terminal.template()
        },
        "elements": elements
    })
}

/// blocking 发送一次飞书互动卡片。失败返回 `Err`，调用方只脱敏 warn。
pub fn send_notification(
    cfg: &FeishuConfig,
    n: &FeishuNotification,
) -> Result<(), FeishuSendError> {
    let card = build_card(cfg, n);
    let payload = json!({ "msg_type": "interactive", "card": card });

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(cfg.connect_timeout)
        .timeout(cfg.read_timeout)
        .build()
        .map_err(|_| FeishuSendError::BuildClient)?;

    let response = client
        .post(&cfg.webhook_url)
        .json(&payload)
        .send()
        .map_err(|_| FeishuSendError::Network)?;

    let status = response.status();
    let mut limited = response.take((BODY_LIMIT + 1) as u64);
    let mut body_bytes = Vec::new();
    limited
        .read_to_end(&mut body_bytes)
        .map_err(|_| FeishuSendError::ReadBody)?;
    if body_bytes.len() > BODY_LIMIT {
        return Err(FeishuSendError::BodyTooLarge { limit: BODY_LIMIT });
    }

    if !status.is_success() {
        return Err(FeishuSendError::HttpStatus(status.as_u16()));
    }

    let body: Value =
        serde_json::from_slice(&body_bytes).map_err(|_| FeishuSendError::ParseBody)?;
    let code = body.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code != 0 {
        let msg = body
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        return Err(FeishuSendError::Business { code, msg });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> FeishuConfig {
        FeishuConfig {
            enabled: true,
            webhook_url: "https://open.feishu.cn/open-apis/bot/v2/hook/x".to_string(),
            web_base_url: Some("http://localhost:8060".to_string()),
            connect_timeout: Duration::from_secs(3),
            read_timeout: Duration::from_secs(10),
        }
    }

    fn notif(detail: Option<String>) -> FeishuNotification {
        FeishuNotification {
            project: "blackboard".to_string(),
            graph_id: "spec-arena-4-plus-1".to_string(),
            run_id: "run-x".to_string(),
            terminal: FeishuTerminal::Failed,
            elapsed: Duration::from_secs(150),
            detail_url: detail,
        }
    }

    #[test]
    fn disabled_config_returns_none() {
        let toml = FeishuTomlConfig {
            enabled: Some(false),
            webhook_url: Some("https://x".to_string()),
            ..Default::default()
        };
        assert!(FeishuConfig::from_toml_and_env(Some(&toml))
            .unwrap()
            .is_none());
    }

    #[test]
    fn enabled_without_webhook_is_error() {
        let toml = FeishuTomlConfig {
            enabled: Some(true),
            webhook_url: None,
            ..Default::default()
        };
        // 注意：依赖 BB_FEISHU_WEBHOOK_URL 未设置；测试环境通常如此。
        if std::env::var("BB_FEISHU_WEBHOOK_URL").is_err() {
            assert_eq!(
                FeishuConfig::from_toml_and_env(Some(&toml)).unwrap_err(),
                FeishuConfigError::EmptyWebhookUrl
            );
        }
    }

    #[test]
    fn non_https_webhook_is_invalid() {
        let toml = FeishuTomlConfig {
            enabled: Some(true),
            webhook_url: Some("http://insecure.example.com/hook".to_string()),
            ..Default::default()
        };
        if std::env::var("BB_FEISHU_WEBHOOK_URL").is_err() {
            assert_eq!(
                FeishuConfig::from_toml_and_env(Some(&toml)).unwrap_err(),
                FeishuConfigError::InvalidWebhookUrl
            );
        }
    }

    #[test]
    fn web_base_url_rejects_path_query_fragment() {
        assert!(normalize_web_base_url(Some("http://h/path")).is_none());
        assert!(normalize_web_base_url(Some("http://h/?q=1")).is_none());
        assert!(normalize_web_base_url(Some("http://h/#f")).is_none());
        assert!(normalize_web_base_url(Some("ftp://h")).is_none());
        assert_eq!(
            normalize_web_base_url(Some("https://h")).as_deref(),
            Some("https://h")
        );
        assert_eq!(
            normalize_web_base_url(Some("http://h:8060/")).as_deref(),
            Some("http://h:8060")
        );
    }

    #[test]
    fn card_only_contains_whitelisted_fields() {
        let card = build_card(
            &cfg(),
            &notif(Some(
                "http://localhost:8060/project/blackboard/runs/run-x".to_string(),
            )),
        );
        let s = serde_json::to_string(&card).unwrap();
        assert!(s.contains("blackboard"));
        assert!(s.contains("spec-arena-4-plus-1"));
        assert!(s.contains("run-x"));
        assert!(s.contains("Failed"));
        assert!(s.contains("2m 30s"));
        // 白名单外字段不得出现
        assert!(!s.contains("node_id"));
        assert!(!s.contains("message"));
        assert_eq!(card["header"]["template"], "red");
    }

    #[test]
    fn card_without_web_base_url_has_no_button() {
        let card = build_card(&cfg(), &notif(None));
        let elements = card["elements"].as_array().unwrap();
        assert_eq!(elements.len(), 1);
        assert_eq!(elements[0]["tag"], "div");
    }
}
