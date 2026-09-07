use std::io::{BufRead, BufReader, Write};

use anyhow::Context;
use interprocess::local_socket::traits::Stream;
use serde::de::DeserializeOwned;

use crate::action::{Action, IpcMessage, MinimizedWindow, MonitorDetails, Query, WorkspaceInfo};
use crate::response::Response;
use crate::socket::socket_name;

#[derive(Default)]
pub struct DomeClient;

impl DomeClient {
    pub fn ping(&self) -> bool {
        interprocess::local_socket::Stream::connect(socket_name()).is_ok()
    }

    pub fn action(&self, action: &Action) -> anyhow::Result<()> {
        self.send(&IpcMessage::Action {
            action: action.clone(),
        })?
        .into_unit()
    }

    pub fn export_layout(&self) -> anyhow::Result<()> {
        self.send(&IpcMessage::ExportLayout)?.into_unit()
    }

    pub fn query<T: DeserializeOwned>(&self, query: &Query) -> anyhow::Result<T> {
        let data = self
            .send(&IpcMessage::Query {
                query: query.clone(),
            })?
            .into_data()?;
        serde_json::from_value(data).context("decode query response")
    }

    pub fn workspaces(&self) -> anyhow::Result<Vec<WorkspaceInfo>> {
        self.query(&Query::Workspaces)
    }

    pub fn minimized_windows(&self) -> anyhow::Result<Vec<MinimizedWindow>> {
        self.query(&Query::MinimizedWindows)
    }

    pub fn monitors(&self) -> anyhow::Result<Vec<MonitorDetails>> {
        self.query(&Query::Monitors)
    }

    fn send(&self, msg: &IpcMessage) -> anyhow::Result<Response> {
        let mut stream = interprocess::local_socket::Stream::connect(socket_name())
            .context("connect to dome socket")?;
        let json = serde_json::to_string(msg).context("serialize request")?;
        writeln!(stream, "{json}").context("write request")?;

        let mut response = String::new();
        BufReader::new(&stream)
            .read_line(&mut response)
            .context("read response")?;
        serde_json::from_str(response.trim()).context("decode response envelope")
    }
}
