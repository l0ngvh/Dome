use super::{Result, fixture::Fixture, monitor::Monitor, windows};
use serde_json::{Value, json};
use std::{sync::mpsc::TryRecvError, thread, time::Duration};
use windows_topmost_reorder::{Completion, case_adjacency_eligible, correlate};

fn await_reply(
    monitor: &mut Monitor<'_>,
    fixture: &Fixture,
    request: Option<u64>,
) -> Result<std::result::Result<Value, Completion>> {
    let until = windows::tick() + monitor.options.transition_ms;
    loop {
        if let Some(reason) = monitor.poll()? {
            return Ok(Err(reason));
        }
        match fixture.replies.try_recv() {
            Ok(value) => {
                monitor.log.write("fixture_reply", value.clone())?;
                let matches = match request {
                    Some(request) => value["request"].as_u64() == Some(request),
                    None => value["reply"] == "ready",
                };
                if !matches {
                    monitor.log.write(
                        "fixture_failure",
                        json!({"error": "unexpected reply", "reply": value}),
                    )?;
                    return Ok(Err(Completion::FixtureFailed));
                }
                return Ok(Ok(value));
            }
            Err(TryRecvError::Disconnected) => {
                monitor
                    .log
                    .write("fixture_failure", json!({"error": "fixture stdout closed"}))?;
                return Ok(Err(Completion::FixtureFailed));
            }
            Err(TryRecvError::Empty) => {}
        }
        if windows::tick() >= until {
            monitor.log.write(
                "fixture_failure",
                json!({"error": "reply timeout", "request": request}),
            )?;
            return Ok(Err(Completion::FixtureFailed));
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn send(monitor: &mut Monitor<'_>, fixture: &mut Fixture, command: Value) -> Result<bool> {
    monitor.log.write("fixture_command", command.clone())?;
    if let Err(error) = fixture.send(command) {
        monitor
            .log
            .write("fixture_failure", json!({"error": error.to_string()}))?;
        return Ok(false);
    }
    Ok(true)
}

struct CasePreconditions {
    initially_eligible: bool,
    peer: usize,
    band_peer: usize,
    reference_neighbors: Option<(Option<u64>, Option<u64>)>,
}

fn trial(
    monitor: &mut Monitor<'_>,
    fixture: &mut Fixture,
    request: u64,
    case: &str,
    repetition: u64,
    desired: bool,
    preconditions: &CasePreconditions,
) -> Result<Option<Completion>> {
    let handle = monitor.target.expect("fixture selected");
    if let Some(reason) = monitor.sample_now()? {
        return Ok(Some(reason));
    }
    let before = windows::snapshot(handle);
    let reference_matched = preconditions
        .reference_neighbors
        .is_none_or(|reference| before.local.at_reference(reference, true));
    let eligibility = preconditions.initially_eligible
        && reference_matched
        && before.boundary().eligible(
            case,
            desired,
            true,
            preconditions.peer,
            preconditions.band_peer,
        );
    monitor.log.write(
        "trial_start",
        json!({"request": request, "case": case, "repetition": repetition,
        "desired_topmost": desired, "case_eligible": eligibility, "before": before.value,
        "setup_reference_neighbors": preconditions.reference_neighbors,
        "setup_reference_matched": reference_matched }),
    )?;
    if case == "lowest_topmost_unchanged_adjacency" && !eligibility {
        monitor.log.write("setup_ineligible", json!({
            "case": case, "repetition": repetition, "request": request,
            "phase": "before_demotion", "reason": "captured boundary no longer established",
            "setup_reference_neighbors": preconditions.reference_neighbors, "snapshot": before.value,
        }))?;
        return Ok(None);
    }
    let edge_start = monitor.edges.len();
    let start = windows::tick();
    let initial = monitor.last.expect("sampled target");
    let mut observed = None;
    let completion = if initial.topmost == desired {
        monitor.log.write(
            "absent_transition",
            json!({"request": request, "error": "already in requested band before command"}),
        )?;
        Completion::NoTransition
    } else if !send(
        monitor,
        fixture,
        json!({"command": "toggle", "request": request, "topmost": desired}),
    )? {
        Completion::FixtureFailed
    } else {
        match await_reply(monitor, fixture, Some(request))? {
            Err(reason) => reason,
            Ok(reply) if reply["ok"] != true => {
                monitor
                    .log
                    .write("api_failure", json!({"request": request, "reply": reply}))?;
                Completion::ApiFailed
            }
            Ok(_) => {
                let until = start + monitor.options.transition_ms;
                loop {
                    if let Some(reason) = monitor.poll()? {
                        break reason;
                    }
                    if let Some(edge) = monitor.edges[edge_start..]
                        .iter()
                        .find(|edge| edge.observed.topmost == desired)
                    {
                        observed = Some(*edge);
                        break Completion::ObservedTransition;
                    }
                    if windows::tick() >= until {
                        monitor.log.write(
                            "absent_transition",
                            json!({"request": request, "deadline": until}),
                        )?;
                        break Completion::NoTransition;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    };
    let mut completion = completion;
    if let Some(edge) = observed {
        if let Some(reason) =
            monitor.wait_until(edge.observed.finished_tick + monitor.options.settle_ms)?
        {
            completion = reason;
        }
    } else {
        let _ = monitor.poll()?;
    }
    let end = windows::tick();
    let after = windows::snapshot(handle);
    let extra_edges = monitor
        .edges
        .len()
        .saturating_sub(edge_start + usize::from(observed.is_some()));
    let before_neighbors = (
        before.value["previous"].as_u64(),
        before.value["next"].as_u64(),
    );
    let after_neighbors = (
        after.value["previous"].as_u64(),
        after.value["next"].as_u64(),
    );
    let adjacency_eligible = case_adjacency_eligible(
        case,
        before_neighbors,
        after_neighbors,
        before.value["target_topmost"].as_bool(),
        after.value["target_topmost"].as_bool(),
    );
    let eligibility = eligibility
        && (case != "lowest_topmost_unchanged_adjacency"
            || completion == Completion::ObservedTransition)
        && extra_edges == 0
        && adjacency_eligible
        && after.boundary().eligible(
            case,
            desired,
            false,
            preconditions.peer,
            preconditions.band_peer,
        );
    monitor.log.write("controlled_trial", json!({
        "request": request, "case": case, "repetition": repetition, "desired_topmost": desired,
        "case_status": if eligibility { "preconditions_observed" } else { "unexercised_or_contaminated" },
        "start_tick": start, "settled_tick": end, "initial_sample": initial,
        "edge": observed, "completion": completion, "extra_edges": extra_edges,
        "evidence": correlate(completion, start, end, &monitor.events),
        "adjacency_changed": before_neighbors != after_neighbors,
        "adjacency_eligible": adjacency_eligible,
        "setup_reference_neighbors": preconditions.reference_neighbors,
        "setup_reference_matched": reference_matched,
        "before": before.value, "after": after.value,
    }))?;
    Ok(Some(completion))
}

pub fn run(monitor: &mut Monitor<'_>) -> Result<Completion> {
    let mut fixture = Fixture::spawn()?;
    let ready = match await_reply(monitor, &fixture, None)? {
        Ok(ready) => ready,
        Err(reason) => return Ok(reason),
    };
    let handles = ready["handles"]
        .as_array()
        .ok_or("fixture omitted handles")?;
    if handles.len() != 3 {
        return Err("fixture returned wrong handle count".into());
    }
    let target = handles[0].as_u64().ok_or("invalid target")? as usize;
    let peer = handles[1].as_u64().ok_or("invalid peer")? as usize;
    let band_peer = handles[2].as_u64().ok_or("invalid band peer")? as usize;
    monitor.target(target)?;
    let mut request = 0;
    for case in [
        "below_ordinary_peer",
        "highest_ordinary_no_other_topmost",
        "alongside_fixture_topmost",
        "lowest_topmost_unchanged_adjacency",
    ] {
        let desired_states: &[bool] = if case == "lowest_topmost_unchanged_adjacency" {
            &[false]
        } else {
            &[true, false]
        };
        for repetition in 1..=monitor.options.repeats {
            request += 1;
            if !send(
                monitor,
                &mut fixture,
                json!({"command": "setup", "request": request, "case": case}),
            )? {
                return Ok(Completion::FixtureFailed);
            }
            let setup_reply = match await_reply(monitor, &fixture, Some(request))? {
                Err(reason) => return Ok(reason),
                Ok(reply) if reply["ok"] != true => {
                    monitor.log.write("setup_api_failure", reply)?;
                    return Ok(Completion::ApiFailed);
                }
                Ok(reply) => reply,
            };
            if let Some(reason) = monitor.wait_until(windows::tick() + monitor.options.settle_ms)? {
                return Ok(reason);
            }
            if let Some(reason) = monitor.baseline(case)? {
                return Ok(reason);
            }
            if let Some(reason) = monitor.sample_now()? {
                return Ok(reason);
            }
            let snapshot = windows::snapshot(target);
            let initial_topmost = monitor.last.map(|sample| sample.topmost);
            let expected_initial_topmost = !desired_states[0];
            let reference_neighbors = (case == "lowest_topmost_unchanged_adjacency").then(|| {
                (
                    setup_reply["setup"]["reference"]["previous"].as_u64(),
                    setup_reply["setup"]["reference"]["next"].as_u64(),
                )
            });
            let setup_eligible = reference_neighbors.is_none_or(|reference| {
                setup_reply["setup"]["eligible"] == true
                    && snapshot.local.at_reference(reference, true)
            });
            let eligible = setup_eligible
                && initial_topmost == Some(expected_initial_topmost)
                && snapshot
                    .boundary()
                    .eligible(case, desired_states[0], true, peer, band_peer);
            let preconditions = CasePreconditions {
                initially_eligible: eligible,
                peer,
                band_peer,
                reference_neighbors,
            };
            monitor.log.write("case_preconditions", json!({"case": case, "repetition": repetition,
                "eligible": eligible, "target_ordinary": initial_topmost == Some(false),
                "target_topmost": initial_topmost, "expected_initial_topmost": expected_initial_topmost,
                "peer": peer, "band_peer": band_peer, "snapshot": snapshot.value,
                "setup": setup_reply["setup"], "setup_reference_neighbors": reference_neighbors }))?;
            if case == "lowest_topmost_unchanged_adjacency" && !eligible {
                monitor.log.write("setup_ineligible", json!({
                    "case": case, "repetition": repetition, "phase": "after_setup_settle",
                    "reason": "captured boundary not established", "setup": setup_reply["setup"],
                    "snapshot": snapshot.value,
                }))?;
                continue;
            }
            for &desired in desired_states {
                if !desired && let Some(reason) = monitor.baseline("before_demotion")? {
                    return Ok(reason);
                }
                request += 1;
                let completion = trial(
                    monitor,
                    &mut fixture,
                    request,
                    case,
                    repetition,
                    desired,
                    &preconditions,
                )?;
                if let Some(completion) = completion
                    && completion != Completion::ObservedTransition
                {
                    return Ok(completion);
                }
            }
        }
    }
    if let Some(reason) = monitor.baseline("controlled_final")? {
        return Ok(reason);
    }
    Ok(Completion::ObservedTransition)
}
