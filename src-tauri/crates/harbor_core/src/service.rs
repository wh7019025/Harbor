use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CoreServiceStatus {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub state: String,
    pub pid: Option<u32>,
    pub port: u16,
    pub bind: String,
    pub detail: Option<String>,
    pub managed_by_core: bool,
    pub stoppable: bool,
}

impl CoreServiceStatus {
    pub fn inactive(id: &str, name: &str, kind: &str, port: u16, detail: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            state: "inactive".into(),
            pid: None,
            port,
            bind: format!("127.0.0.1:{port}"),
            detail: Some(detail.into()),
            managed_by_core: true,
            stoppable: false,
        }
    }
}
