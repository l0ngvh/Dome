use super::{Log, Options, Result, windows};
use serde::Serialize;
use serde_json::json;
use std::{thread, time::Duration};
use windows_topmost_reorder::{Completion, Reorder, correlate, manual_settlement};

#[derive(Clone, Copy, Serialize)]
pub struct Sample {
    pub started_tick: u64,
    pub finished_tick: u64,
    pub topmost: bool,
}

#[derive(Clone, Copy, Serialize)]
pub struct Edge {
    pub previous: Sample,
    pub observed: Sample,
}

pub struct Monitor<'a> {
    pub options: &'a Options,
    pub log: Log,
    pub events: Vec<Reorder>,
    pub edges: Vec<Edge>,
    pub last: Option<Sample>,
    pub target: Option<usize>,
    identity: Option<(u32, u32)>,
    next_sample: u64,
    deadline: u64,
    collected_through: u64,
    failure: Option<Completion>,
}

impl<'a> Monitor<'a> {
    pub fn new(options: &'a Options, log: Log) -> Self {
        Self {
            options,
            log,
            events: Vec::new(),
            edges: Vec::new(),
            last: None,
            target: None,
            identity: None,
            next_sample: 0,
            deadline: windows::tick() + options.seconds * 1000,
            collected_through: 0,
            failure: None,
        }
    }

    pub fn target(&mut self, handle: usize) -> Result<()> {
        self.target = Some(handle);
        self.identity = windows::identity(handle);
        self.log.write(
            "target_selected",
            json!({"hwnd": handle, "identity": self.identity}),
        )?;
        if self.identity.is_none() {
            self.log.write(
                "target_closed",
                json!({"hwnd": handle, "phase": "selection"}),
            )?;
            self.failure = Some(Completion::TargetClosed);
        }
        self.next_sample = 0;
        Ok(())
    }

    pub fn sample_now(&mut self) -> Result<Option<Completion>> {
        self.next_sample = 0;
        self.poll()
    }

    pub fn poll(&mut self) -> Result<Option<Completion>> {
        let collected_through = windows::tick();
        windows::pump();
        self.collected_through = collected_through;
        for mut event in windows::drain_events() {
            event.sequence = self.events.len() as u64 + 1;
            self.log.write("reorder", serde_json::to_value(&event)?)?;
            self.events.push(event);
        }
        if self.failure.is_none()
            && windows::tick() >= self.next_sample
            && let Some(handle) = self.target
        {
            let started_tick = windows::tick();
            if windows::identity(handle) != self.identity {
                self.log.write(
                    "target_closed",
                    json!({"hwnd": handle, "identity_changed": true}),
                )?;
                self.failure = Some(Completion::TargetClosed);
            } else {
                let state = windows::topmost(handle);
                let finished_tick = windows::tick();
                if windows::identity(handle) != self.identity {
                    self.log.write(
                        "target_closed",
                        json!({"hwnd": handle, "during_sample": true}),
                    )?;
                    self.failure = Some(Completion::TargetClosed);
                } else {
                    match state {
                        Ok(topmost) => {
                            let sample = Sample {
                                started_tick,
                                finished_tick,
                                topmost,
                            };
                            self.log.write("observation", json!({"hwnd": handle, "sample": sample,
                                "gap_ms": self.last.map(|previous| started_tick.saturating_sub(previous.started_tick)) }))?;
                            if let Some(previous) = self.last
                                && previous.topmost != topmost
                            {
                                let edge = Edge {
                                    previous,
                                    observed: sample,
                                };
                                self.log.write("observed_transition", json!({"hwnd": handle, "edge": edge,
                                    "adjacent_previous": windows::adjacent(handle, ::windows::Win32::UI::WindowsAndMessaging::GW_HWNDPREV),
                                    "adjacent_next": windows::adjacent(handle, ::windows::Win32::UI::WindowsAndMessaging::GW_HWNDNEXT) }))?;
                                self.edges.push(edge);
                            }
                            self.last = Some(sample);
                        }
                        Err(error) => {
                            self.log.write(
                                "sampling_failure",
                                json!({"hwnd": handle, "error": error.to_string()}),
                            )?;
                            self.failure = Some(Completion::SamplingFailed);
                        }
                    }
                }
            }
            self.next_sample = started_tick + self.options.sample_ms;
        }
        Ok(self.failure.or_else(|| {
            if windows::interrupted() {
                Some(Completion::Interrupted)
            } else if windows::tick() >= self.deadline {
                Some(Completion::Deadline)
            } else {
                None
            }
        }))
    }

    pub fn wait_until(&mut self, until: u64) -> Result<Option<Completion>> {
        loop {
            if let Some(reason) = self.poll()? {
                return Ok(Some(reason));
            }
            if windows::tick() >= until {
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    pub fn baseline(&mut self, label: &str) -> Result<Option<Completion>> {
        let start = windows::tick();
        let edge_start = self.edges.len();
        self.log
            .write("baseline_start", json!({"label": label, "start": start}))?;
        let reason = self.wait_until(start + self.options.baseline_ms)?;
        let end = windows::tick();
        let event_sequences: Vec<_> = self
            .events
            .iter()
            .filter(|event| event.delivered_tick >= start && event.delivered_tick <= end)
            .map(|event| event.sequence)
            .collect();
        self.log.write(
            "baseline_end",
            json!({"label": label, "start": start, "end": end,
            "event_sequences": event_sequences, "quiet": event_sequences.is_empty(),
            "observed_edges": self.edges.len() - edge_start,
            "interrupted": reason }),
        )?;
        Ok(reason)
    }

    fn manual_trial(
        &mut self,
        index: usize,
        stop: Option<Completion>,
        collected_through: u64,
    ) -> Result<()> {
        let edge = self.edges[index];
        let settled = edge.observed.finished_tick + self.options.settle_ms;
        let (completion, end) = manual_settlement(settled, collected_through, stop);
        let overlapping_edges = self
            .edges
            .iter()
            .filter(|other| {
                other.observed.finished_tick > edge.previous.started_tick
                    && other.previous.started_tick <= end
            })
            .count()
            .saturating_sub(1);
        self.log.write(
            "manual_trial",
            json!({"index": index, "edge": edge, "settled_tick": end,
            "completion": completion, "overlapping_edges": overlapping_edges,
            "evidence": correlate(completion, edge.previous.started_tick, end, &self.events) }),
        )
    }

    pub fn manual(&mut self, handle: usize) -> Result<Completion> {
        self.target(handle)?;
        if let Some(reason) = self.sample_now()? {
            return Ok(reason);
        }
        self.log
            .write("manual_snapshot", windows::snapshot(handle).value)?;
        eprintln!(
            "Monitoring HWND {handle:#x}. Leave it unchanged for the baseline, then toggle its own control or PowerToys. Ctrl+C stops."
        );
        let baseline_reason = self.baseline("manual_initial")?;
        if baseline_reason.is_none() {
            self.log.write("manual_ready", json!({"hwnd": handle}))?;
            eprintln!("Baseline finished. Manual toggles can begin.");
        }
        let mut next = 0;
        let reason = loop {
            let reason = match baseline_reason {
                Some(reason) => Some(reason),
                None => self.poll()?,
            };
            let collected_through = self.collected_through;
            while next < self.edges.len()
                && collected_through
                    >= self.edges[next].observed.finished_tick + self.options.settle_ms
            {
                self.manual_trial(next, reason, collected_through)?;
                next += 1;
            }
            if let Some(reason) = reason {
                break reason;
            }
            thread::sleep(Duration::from_millis(1));
        };
        let collected_through = self.collected_through;
        while next < self.edges.len() {
            self.manual_trial(next, Some(reason), collected_through)?;
            next += 1;
        }
        self.log
            .write("manual_end_snapshot", windows::snapshot(handle).value)?;
        Ok(reason)
    }
}
