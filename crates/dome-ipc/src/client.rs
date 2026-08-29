use std::io::{BufRead, BufReader, Write};

use interprocess::local_socket::traits::Stream;

use crate::action::IpcMessage;
use crate::socket::socket_name;

#[derive(Default)]
pub struct DomeClient;

impl DomeClient {
    pub fn ping(&self) -> bool {
        interprocess::local_socket::Stream::connect(socket_name()).is_ok()
    }

    fn send(&self, msg: &IpcMessage) -> std::io::Result<String> {
        let mut stream = interprocess::local_socket::Stream::connect(socket_name())?;
        let json = serde_json::to_string(msg).map_err(std::io::Error::other)?;
        writeln!(stream, "{json}")?;

        let mut response = String::new();
        BufReader::new(&stream).read_line(&mut response)?;
        Ok(response.trim().to_string())
    }

    pub fn action(&self, action: &crate::action::Action) -> std::io::Result<String> {
        self.send(&IpcMessage::Action(action.clone()))
    }

    pub fn query(&self, query: &crate::action::Query) -> std::io::Result<String> {
        self.send(&IpcMessage::Query(query.clone()))
    }

    pub fn export(&self) -> std::io::Result<String> {
        self.send(&IpcMessage::ExportLayout)
    }
}
