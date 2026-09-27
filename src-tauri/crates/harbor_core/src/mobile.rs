use std::sync::Arc;

use axum::extract::State;
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use axum::{Json, Router};
use parking_lot::Mutex;
use serde::Serialize;

use crate::taskcard::{TaskCardService, TaskCardSnapshot};
use harbor_protocol::web_api::MOBILE_WEB_PORT;

const MOBILE_HTML: &str = include_str!("mobile.html");

#[derive(Clone)]
struct MobileState {
    taskcard: Arc<Mutex<TaskCardService>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct MobilePanel {
    id: String,
    task_name: String,
    panel_name: String,
    interface_port: u16,
}

#[derive(Debug, Serialize)]
struct MobilePanels {
    panels: Vec<MobilePanel>,
}

pub async fn bind_listener() -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(("0.0.0.0", MOBILE_WEB_PORT)).await
}

pub fn router(taskcard: Arc<Mutex<TaskCardService>>) -> Router {
    Router::new()
        .route("/", get(mobile_page))
        .route("/mobile", get(mobile_page))
        .route("/mobile/", get(mobile_page))
        .route("/api/panels", get(mobile_panels))
        .with_state(MobileState { taskcard })
}

fn panels_from_snapshot(snapshot: &TaskCardSnapshot) -> Vec<MobilePanel> {
    snapshot
        .tasks
        .iter()
        .filter(|task| task.status == "running")
        .flat_map(|task| {
            task.webview_interface
                .iter()
                .filter(|panel| !panel.localhost_only)
                .map(|panel| MobilePanel {
                    id: format!("{}:{}", task.uuid, panel.panel_name),
                    task_name: task.name.clone(),
                    panel_name: panel.panel_name.clone(),
                    interface_port: panel.interface_port,
                })
        })
        .collect()
}

async fn mobile_page() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Html(MOBILE_HTML),
    )
}

async fn mobile_panels(State(state): State<MobileState>) -> Json<MobilePanels> {
    let snapshot = state.taskcard.lock().snapshot();
    Json(MobilePanels {
        panels: panels_from_snapshot(&snapshot),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use harbor_protocol::taskcard::{TaskCardSnapshot, TaskSummary, WebviewInterface};
    use std::fs;
    use tower::ServiceExt;

    fn task(status: &str, localhost_only: bool) -> TaskSummary {
        TaskSummary {
            uuid: "task-uuid".into(),
            uuid_conflict: false,
            id: "camera".into(),
            prefix_path: String::new(),
            name: "相机".into(),
            description: String::new(),
            workdir: String::new(),
            command: String::new(),
            env_count: 0,
            configs: Vec::new(),
            default_config: None,
            running_config_id: None,
            requires_sudo: false,
            webview_interface: vec![WebviewInterface {
                panel_name: "preview".into(),
                interface_port: 23842,
                localhost_only,
            }],
            remote_display_virtual: false,
            folder: String::new(),
            status: status.into(),
            pid: None,
            started_at_ms: None,
            log_file: None,
        }
    }

    fn snapshot(tasks: Vec<TaskSummary>) -> TaskCardSnapshot {
        TaskCardSnapshot {
            generated_at_ms: 0,
            stale: false,
            root: String::new(),
            default_route_ip: "192.168.1.2".into(),
            vnc_port: 0,
            vnc_ready: false,
            physical_vnc_port: 0,
            physical_vnc_ready: false,
            physical_vnc_error: None,
            search_paths: Vec::new(),
            discovered_task_dirs: Vec::new(),
            discovered_group_dirs: Vec::new(),
            tasks,
            groups: Vec::new(),
            uuid_conflicts: Vec::new(),
            errors: Vec::new(),
        }
    }

    #[test]
    fn exposes_only_running_network_panels() {
        let panels = panels_from_snapshot(&snapshot(vec![
            task("running", false),
            task("running", true),
            task("stopped", false),
        ]));
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].panel_name, "preview");
        assert_eq!(panels[0].interface_port, 23842);
    }

    #[tokio::test]
    async fn mobile_router_exposes_only_read_only_mobile_routes() {
        let root = std::env::temp_dir().join(format!(
            "harbor-mobile-router-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let service = TaskCardService::new(root.clone(), Vec::new()).unwrap();
        let app = router(Arc::new(Mutex::new(service)));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/mobile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("Harbor Mobile"));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let _ = fs::remove_dir_all(root);
    }
}
