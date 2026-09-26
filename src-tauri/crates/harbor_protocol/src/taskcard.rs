use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::web_api::{PHYSICAL_VNC_PORT, VIRTUAL_VNC_PORT};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebviewInterface {
    pub panel_name: String,
    pub interface_port: u16,
    #[serde(default)]
    pub localhost_only: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VncInterface {
    pub panel_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskConfig {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupDefinition {
    #[serde(deserialize_with = "deserialize_yaml_version")]
    pub version: String,
    #[serde(default = "generate_uuid")]
    pub uuid: String,
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub tasks: Vec<GroupTask>,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub prefix_path: String,
    #[serde(default, skip_deserializing)]
    pub definition_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupTask {
    pub task: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config: String,
    #[serde(default)]
    pub wait_after_sec: u64,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix_path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TaskSummary {
    pub uuid: String,
    pub uuid_conflict: bool,
    pub id: String,
    pub prefix_path: String,
    pub name: String,
    pub description: String,
    pub workdir: String,
    pub command: String,
    pub env_count: usize,
    pub configs: Vec<TaskConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_config: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub running_config_id: Option<String>,
    pub requires_sudo: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub webview_interface: Vec<WebviewInterface>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vnc_interface: Vec<VncInterface>,
    pub folder: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<u128>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_file: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskCardSnapshot {
    #[serde(default)]
    pub generated_at_ms: u128,
    #[serde(default)]
    pub stale: bool,
    pub root: String,
    #[serde(default = "loopback_address")]
    pub default_route_ip: String,
    #[serde(default = "virtual_vnc_port")]
    pub vnc_port: u16,
    #[serde(default)]
    pub vnc_ready: bool,
    #[serde(default = "physical_vnc_port")]
    pub physical_vnc_port: u16,
    #[serde(default)]
    pub physical_vnc_ready: bool,
    #[serde(default)]
    pub physical_vnc_error: Option<String>,
    pub search_paths: Vec<String>,
    pub discovered_task_dirs: Vec<String>,
    pub discovered_group_dirs: Vec<String>,
    pub tasks: Vec<TaskSummary>,
    pub groups: Vec<GroupDefinition>,
    #[serde(default)]
    pub uuid_conflicts: Vec<UuidConflict>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UuidDefinitionRef {
    pub kind: String,
    pub id: String,
    pub prefix_path: String,
    pub path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UuidConflict {
    pub uuid: String,
    pub definitions: Vec<UuidDefinitionRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResearchResult {
    pub search_paths: Vec<String>,
    pub discovered_task_dirs: Vec<String>,
    pub discovered_group_dirs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskLogSummary {
    pub file: String,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_id: Option<String>,
    pub started_at_ms: u128,
    pub modified_at_ms: u128,
    pub bytes: u64,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskLogContent {
    pub file: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskLogChunk {
    pub file: String,
    pub content: String,
    pub next_offset: u64,
    pub reset: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskCardYamlDocument {
    pub content: String,
    pub folder: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedProcessInfo {
    pub pid: i32,
    pub ppid: i32,
    pub name: String,
    pub command: String,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedProcessGroup {
    pub uuid: String,
    pub prefix_path: String,
    pub task_id: String,
    pub leader_pid: u32,
    pub pgid: i32,
    pub started_at_ms: u128,
    pub log_file: String,
    pub config_id: Option<String>,
    pub task_running: bool,
    pub orphaned: bool,
    pub processes: Vec<ManagedProcessInfo>,
}

fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn loopback_address() -> String {
    "127.0.0.1".into()
}

fn virtual_vnc_port() -> u16 {
    VIRTUAL_VNC_PORT
}

fn physical_vnc_port() -> u16 {
    PHYSICAL_VNC_PORT
}

fn deserialize_yaml_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct VersionVisitor;

    impl<'de> serde::de::Visitor<'de> for VersionVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a string or number version")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value.to_string())
        }
    }

    deserializer.deserialize_any(VersionVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_contract_accepts_numeric_version_and_generates_uuid() {
        let group: GroupDefinition = serde_yaml::from_str(
            r#"version: 1
id: demo
tasks: []
"#,
        )
        .unwrap();
        assert_eq!(group.version, "1");
        assert!(Uuid::parse_str(&group.uuid).is_ok());
    }

    #[test]
    fn snapshot_defaults_use_protocol_ports() {
        let snapshot: TaskCardSnapshot = serde_json::from_str(
            r#"{
                "root":"/tmp",
                "search_paths":[],
                "discovered_task_dirs":[],
                "discovered_group_dirs":[],
                "tasks":[],
                "groups":[],
                "errors":[]
            }"#,
        )
        .unwrap();
        assert_eq!(snapshot.vnc_port, VIRTUAL_VNC_PORT);
        assert_eq!(snapshot.physical_vnc_port, PHYSICAL_VNC_PORT);
    }
}
