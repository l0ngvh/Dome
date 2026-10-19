use super::{EventSender, Shutdown};
use crate::action::{Action, Query};
use crate::ipc::tests::connection_pair;
use crate::platform::macos::dome::ipc::handle_connection;
use crate::platform::messages::{HubMessage, HubSender};
use dome_ipc::response::{ErrorCode, Response};
use futures::channel::mpsc;
use interprocess::local_socket::traits::Stream;
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

struct ConnectionHandlers {
    hub: HubSender,
    incoming: Option<mpsc::UnboundedReceiver<HubMessage>>,
    shutdown: Shutdown,
    operations: Arc<Mutex<()>>,
    workers: Vec<std::thread::JoinHandle<anyhow::Result<()>>>,
}

struct ClientConnection {
    stream: interprocess::local_socket::Stream,
    worker: usize,
}

impl ConnectionHandlers {
    fn new() -> Self {
        let (sender, incoming) = mpsc::unbounded();
        let waker = Arc::new(OnceLock::new());
        Self {
            hub: HubSender(EventSender {
                sender,
                waker: waker.clone(),
            }),
            incoming: Some(incoming),
            shutdown: Shutdown {
                stopped: Arc::new(AtomicBool::new(false)),
                waker,
            },
            operations: Arc::new(Mutex::new(())),
            workers: Vec::new(),
        }
    }

    fn start(&mut self, request: &str) -> ClientConnection {
        let (connection, mut stream) = connection_pair("");
        stream.write_all(request.as_bytes()).unwrap();
        let hub = self.hub.clone();
        let operations = self.operations.clone();
        let shutdown = self.shutdown.clone();
        let worker = self.workers.len();
        let (started, started_reading) = std::sync::mpsc::channel();
        self.workers.push(std::thread::spawn(move || {
            started.send(()).unwrap();
            handle_connection(connection, &hub, &operations, &shutdown)
        }));
        started_reading
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        ClientConnection { stream, worker }
    }

    fn is_pending(&self, client: &ClientConnection) -> bool {
        !self.workers[client.worker].is_finished()
    }

    fn next_message(&mut self, deadline: Instant) -> HubMessage {
        loop {
            if let Ok(message) = self.incoming.as_mut().unwrap().try_recv() {
                return message;
            }
            assert!(Instant::now() < deadline, "IPC handler did not dispatch");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn reply_to_query(&mut self, expected: Query, value: serde_json::Value, deadline: Instant) {
        let HubMessage::Query { query, reply } = self.next_message(deadline) else {
            panic!("expected a query request");
        };
        assert_eq!(
            serde_json::to_value(query).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        reply.send(value).unwrap();
    }

    fn reply_to_close(&mut self, outcome: anyhow::Result<()>, deadline: Instant) {
        let HubMessage::Action { action, reply } = self.next_message(deadline) else {
            panic!("expected an action request");
        };
        assert!(matches!(action, Action::Close));
        reply.send(outcome).unwrap();
    }
}

impl Drop for ConnectionHandlers {
    fn drop(&mut self) {
        drop(self.incoming.take());
        for worker in self.workers.drain(..) {
            let result = worker.join();
            if !std::thread::panicking() {
                result.unwrap().unwrap();
            }
        }
    }
}

impl ClientConnection {
    fn complete(&mut self, remainder: &str) {
        writeln!(self.stream, "{remainder}").unwrap();
    }

    fn response_before(self, deadline: Instant) -> Response {
        self.stream.set_nonblocking(true).unwrap();
        let mut reader = BufReader::new(self.stream);
        let mut line = String::new();
        loop {
            match reader.read_line(&mut line) {
                Ok(_) if line.ends_with('\n') => return serde_json::from_str(&line).unwrap(),
                Ok(0) => panic!("IPC handler closed without a response"),
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("IPC response read failed: {error}"),
            }
            assert!(
                Instant::now() < deadline,
                "IPC response missed its deadline"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[test]
fn handle_connection_with_incomplete_peer_serves_complete_request() {
    let mut handlers = ConnectionHandlers::new();
    let mut first = handlers.start(r#"{"type":"query","#);
    let deadline = Instant::now() + Duration::from_secs(1);
    let second = handlers.start("{\"type\":\"query\",\"query\":{\"type\":\"monitors\"}}\n");
    let monitors = serde_json::json!([{"name": "Display 1"}]);
    handlers.reply_to_query(Query::Monitors, monitors.clone(), deadline);
    assert_eq!(second.response_before(deadline), Response::ok(monitors));
    assert!(
        handlers.is_pending(&first),
        "incomplete request finished before its read deadline"
    );

    first.complete(r#""query":{"type":"workspaces"}}"#);
    let deadline = Instant::now() + Duration::from_secs(1);
    handlers.reply_to_query(Query::Workspaces, serde_json::json!([]), deadline);
    assert_eq!(
        first.response_before(deadline),
        Response::ok(serde_json::json!([]))
    );
}

#[test]
fn handle_action_request_preserves_success_and_failure_context() {
    for outcome in [
        Ok(()),
        Err(anyhow::anyhow!("window rejected close").context("close Editor")),
    ] {
        let mut handlers = ConnectionHandlers::new();
        let client = handlers.start("{\"type\":\"action\",\"action\":{\"type\":\"close\"}}\n");
        let deadline = Instant::now() + Duration::from_secs(1);
        let expected = outcome
            .as_ref()
            .map(|_| ())
            .map_err(|error| format!("{error:#}"));
        handlers.reply_to_close(outcome, deadline);
        let response = client.response_before(deadline);
        match expected {
            Ok(()) => assert_eq!(response, Response::ok(serde_json::Value::Null)),
            Err(message) => {
                assert_eq!(response, Response::error(ErrorCode::ActionFailed, message))
            }
        }
    }
}
