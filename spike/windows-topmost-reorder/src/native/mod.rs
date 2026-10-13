mod controlled;
mod fixture;
mod monitor;
mod windows;

use serde_json::{Value, json};
use std::{fs::OpenOptions, io::Write, path::PathBuf};
use windows_topmost_reorder::Completion;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub struct Options {
    mode: String,
    handle: Option<usize>,
    output: PathBuf,
    seconds: u64,
    sample_ms: u64,
    settle_ms: u64,
    baseline_ms: u64,
    transition_ms: u64,
    repeats: u64,
}

impl Options {
    fn parse(arguments: &[String]) -> Result<Self> {
        let mut options = Self {
            mode: arguments
                .first()
                .ok_or("expected controlled or manual")?
                .clone(),
            handle: None,
            output: "reorder.jsonl".into(),
            seconds: 120,
            sample_ms: 10,
            settle_ms: 500,
            baseline_ms: 1000,
            transition_ms: 2000,
            repeats: 5,
        };
        if !matches!(options.mode.as_str(), "controlled" | "manual") {
            return Err("expected controlled or manual".into());
        }
        for pair in arguments[1..].chunks(2) {
            let value = pair.get(1).ok_or("each flag needs a value")?;
            match pair[0].as_str() {
                "--hwnd" => {
                    options.handle = Some(if let Some(hex) = value.strip_prefix("0x") {
                        usize::from_str_radix(hex, 16)?
                    } else {
                        value.parse()?
                    });
                }
                "--out" => options.output = value.into(),
                "--seconds" => options.seconds = value.parse()?,
                "--sample-ms" => options.sample_ms = value.parse()?,
                "--settle-ms" => options.settle_ms = value.parse()?,
                "--baseline-ms" => options.baseline_ms = value.parse()?,
                "--transition-ms" => options.transition_ms = value.parse()?,
                "--repeats" => options.repeats = value.parse()?,
                _ => return Err(format!("unknown flag {}", pair[0]).into()),
            }
        }
        if !(1..=3600).contains(&options.seconds)
            || !(1..=1000).contains(&options.sample_ms)
            || !(1..=60_000).contains(&options.settle_ms)
            || !(1..=60_000).contains(&options.baseline_ms)
            || !(1..=60_000).contains(&options.transition_ms)
            || !(1..=1000).contains(&options.repeats)
        {
            return Err("invalid timing or repeat count, see README".into());
        }
        if options.mode == "manual" && options.handle.is_none_or(|handle| handle == 0) {
            return Err("manual requires a nonzero --hwnd".into());
        }
        if options.mode == "controlled" && options.handle.is_some() {
            return Err("controlled owns fixtures and does not accept --hwnd".into());
        }
        Ok(options)
    }
}

pub struct Log(std::fs::File);

impl Log {
    fn open(path: &PathBuf) -> Result<Self> {
        Ok(Self(
            OpenOptions::new().write(true).create_new(true).open(path)?,
        ))
    }

    pub fn write(&mut self, kind: &str, fields: Value) -> Result<()> {
        let line = json!({"kind": kind, "logged_tick": windows::tick(), "fields": fields});
        serde_json::to_writer(&mut self.0, &line)?;
        self.0.write_all(b"\n")?;
        self.0.flush()?;
        Ok(())
    }
}

pub fn run() -> Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--fixture")
    {
        return fixture::run();
    }
    let options = Options::parse(&arguments)?;
    let mut log = Log::open(&options.output)?;
    log.write("session", json!({
        "mode": options.mode, "process": std::process::id(),
        "unix_ms": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis(),
        "sample_ms": options.sample_ms, "settle_ms": options.settle_ms,
        "baseline_ms": options.baseline_ms, "transition_ms": options.transition_ms,
        "seconds": options.seconds, "repeats": options.repeats,
        "hook_scope": "global_current_desktop_skip_own_process_no_object_filter"
    }))?;
    let _interrupt = windows::Interrupt::install()?;
    let _hook = match windows::Hook::install() {
        Ok(hook) => hook,
        Err(error) => {
            log.write("hook_setup_failure", json!({"error": error.to_string()}))?;
            return Err(error);
        }
    };
    log.write("hook_installed", json!({}))?;
    let mut monitor = monitor::Monitor::new(&options, log);
    let result = if options.mode == "controlled" {
        controlled::run(&mut monitor)
    } else {
        monitor.manual(options.handle.expect("validated handle"))
    };
    monitor.target = None;
    let _ = monitor.poll()?;
    match result {
        Ok(reason) => {
            monitor.log.write(
                "session_end",
                json!({"reason": reason, "interrupted": windows::interrupted()}),
            )?;
            if matches!(
                reason,
                Completion::TargetClosed
                    | Completion::Deadline
                    | Completion::ObservedTransition
                    | Completion::Interrupted
            ) {
                Ok(())
            } else {
                Err(format!("session ended with {reason:?}, see log").into())
            }
        }
        Err(error) => {
            monitor
                .log
                .write("session_error", json!({"error": error.to_string()}))?;
            Err(error)
        }
    }
}
