#[cfg(target_os = "macos")]
use crate::platform::macos as native;
#[cfg(target_os = "windows")]
use crate::platform::windows as native;
use futures::channel::oneshot;

use crate::action::{Action, Query};
use dome_core::PreferredLayouts;

pub(crate) enum HubMessage {
    Environment {
        reply: oneshot::Sender<std::collections::HashMap<String, String>>,
    },
    Action {
        action: Action,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    CaptureLayouts {
        reply: oneshot::Sender<PreferredLayouts>,
    },
    ApplyLayouts {
        layouts: PreferredLayouts,
        reply: oneshot::Sender<()>,
    },
    Query {
        query: Query,
        reply: oneshot::Sender<serde_json::Value>,
    },
}

#[derive(Clone)]
pub(crate) struct HubSender(pub(crate) native::event_loop::EventSender<HubMessage>);

impl HubSender {
    pub(crate) async fn execute_action(&self, action: Action) -> anyhow::Result<()> {
        let (reply, result) = oneshot::channel();
        self.0
            .send(HubMessage::Action { action, reply })
            .map_err(|_| anyhow::anyhow!("Hub stopped"))?;
        result.await?
    }

    pub(crate) async fn environment(
        &self,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let (reply, result) = oneshot::channel();
        self.0
            .send(HubMessage::Environment { reply })
            .map_err(|_| anyhow::anyhow!("Hub stopped"))?;
        Ok(result.await?)
    }

    pub(crate) async fn capture_layouts(&self) -> anyhow::Result<PreferredLayouts> {
        let (reply, result) = oneshot::channel();
        self.0
            .send(HubMessage::CaptureLayouts { reply })
            .map_err(|_| anyhow::anyhow!("Hub stopped"))?;
        Ok(result.await?)
    }

    pub(crate) async fn apply_layouts(&self, layouts: PreferredLayouts) -> anyhow::Result<()> {
        let (reply, result) = oneshot::channel();
        self.0
            .send(HubMessage::ApplyLayouts { layouts, reply })
            .map_err(|_| anyhow::anyhow!("Hub stopped"))?;
        Ok(result.await?)
    }

    pub(crate) async fn query(&self, query: Query) -> anyhow::Result<serde_json::Value> {
        let (reply, result) = oneshot::channel();
        self.0
            .send(HubMessage::Query { query, reply })
            .map_err(|_| anyhow::anyhow!("Hub stopped"))?;
        Ok(result.await?)
    }
}
