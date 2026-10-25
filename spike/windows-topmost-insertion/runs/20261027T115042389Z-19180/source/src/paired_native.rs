use super::{
    Fixture, FixtureSet, OwnedWindow, Result, Trial, desktop, disable_input_method, emit, handle,
    local_place, number, observe, pump, settle, snapshot, source_place, timestamp, treatment,
};
use crate::{
    observation::both_topmost_owned_pair,
    paired_protocol::{Member, initialize_owned, schedule},
};
use serde_json::{Value, json};
use std::{
    fs::File,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::{GetLastError, HWND, RECT, SetLastError, WIN32_ERROR},
    System::Threading::GetCurrentThreadId,
    UI::WindowsAndMessaging::{
        DestroyWindow, GWL_STYLE, GetClassNameW, GetWindowLongPtrW, GetWindowRect, HWND_BOTTOM,
        HWND_NOTOPMOST, HWND_TOPMOST, IsWindow, WINDOW_EX_STYLE, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
    },
};

pub(super) fn manifest() -> Value {
    json!({"creation_order": ["A", "B", "P", "X", "O"],
        "class": "DomeTopmostInsertionSpike", "source_geometry": [160, 160, 300, 180],
        "overlay_geometry": [120, 160, 300, 180], "owner": "X", "setup_flags": 19,
        "initial_fixture_order": ["A", "B", "O", "X", "P"],
        "setup_after_creation": [["P", 1], ["B", -1], ["A", -1]],
        "owner_promotion": {"precreation_calls": [], "owner_topmost_at_creation": false, "treatment": ["X", -1]},
        "owner_after_a": {"precreation_calls": [["X", -1]], "owner_topmost_at_creation": true, "treatment": ["X", "A"]},
        "owned_demotion": {"precreation_calls": [["X", -1]], "owner_topmost_at_creation": true, "treatment": ["O", -2]},
        "freshness": "fresh A/B/P processes and windows, fresh X/O owning thread and windows, collector process and desktop persist",
        "comparison_limit": "matched recorded fixture history, not control of every Windows internal or external desktop condition"})
}

fn attributes(window: HWND) -> Result<Value> {
    let started_us = timestamp();
    let observation = observe(window);
    let mut class = [0u16; 256];
    unsafe { SetLastError(WIN32_ERROR(0)) };
    let length = unsafe { GetClassNameW(window, &mut class) };
    if length == 0 {
        return Err(windows::core::Error::from_thread().into());
    }
    unsafe { SetLastError(WIN32_ERROR(0)) };
    let style = unsafe { GetWindowLongPtrW(window, GWL_STYLE) };
    let style_error = unsafe { GetLastError().0 };
    let mut rectangle = RECT::default();
    unsafe { GetWindowRect(window, &mut rectangle) }?;
    Ok(
        json!({"started_us": started_us, "finished_us": timestamp(), "window": observation,
        "class": String::from_utf16(&class[..length as usize])?, "style": style,
        "style_read_error": (style == 0 && style_error != 0).then_some(style_error),
        "rectangle": [rectangle.left, rectangle.top, rectangle.right, rectangle.bottom]}),
    )
}

pub(super) fn create_window(overlay: bool, owner: Option<HWND>) -> Result<(OwnedWindow, Value)> {
    let mut slot = None;
    let creation = create_tracked(&mut slot, overlay, owner, None)?;
    Ok((slot.expect("successful creation is owned"), creation))
}

fn create_tracked(
    slot: &mut Option<OwnedWindow>,
    overlay: bool,
    owner: Option<HWND>,
    mut recording: Option<(&mut File, &str)>,
) -> Result<Value> {
    let started = timestamp();
    let result = {
        let owner_read_started_us = timestamp();
        let owner_before = owner.map(observe);
        let owner_read_finished_us = timestamp();
        let started_us = timestamp();
        initialize_owned(
            slot,
            || OwnedWindow::create(overlay, owner),
            |window| {
                let returned_us = timestamp();
                if let Some((output, role)) = recording.as_mut() {
                    emit(
                        output,
                        json!({"kind": "window_acquired", "role": role,
            "creation": {"handle": number(window.0), "calling_process": std::process::id(),
            "calling_thread": unsafe { GetCurrentThreadId() }, "started_us": started_us, "returned_us": returned_us}}),
                    )?;
                }
                let actual = attributes(window.0)?;
                let creation = json!({"api": "CreateWindowExW", "handle": number(window.0),
        "owner": owner.map_or(0, number), "owner_before": owner_before,
        "owner_read_started_us": owner_read_started_us, "owner_read_finished_us": owner_read_finished_us,
        "style": ((if overlay { WS_POPUP } else { WS_OVERLAPPEDWINDOW }) | WS_VISIBLE).0,
        "extended_style": (WS_EX_NOACTIVATE | if overlay { WS_EX_TOOLWINDOW } else { WINDOW_EX_STYLE(0) }).0,
        "class": "DomeTopmostInsertionSpike", "geometry": [if overlay { 120 } else { 160 }, 160, 300, 180],
        "calling_process": std::process::id(), "calling_thread": unsafe { GetCurrentThreadId() },
        "started_us": started_us, "returned_us": returned_us, "actual": actual,
        "desktop": desktop()?});
                Ok(creation)
            },
        )
    };
    if let (Err(error), Some((output, role))) = (&result, recording) {
        emit(
            output,
            json!({"kind": "initialization_failed", "role": role,
            "stage": if slot.is_some() { "attributes" } else { "create" },
            "started_us": started, "finished_us": timestamp(), "error": error.to_string(),
            "resource_owned": slot.is_some()}),
        )?;
    }
    result
}

fn spawn_tracked(
    output: &mut File,
    slot: &mut Option<Fixture>,
    role: &str,
    directory: &Path,
    desktop_name: &str,
) -> Result<()> {
    let started_us = timestamp();
    let result = initialize_owned(
        slot,
        || Fixture::launch(role, directory, 30, desktop_name),
        |fixture| fixture.initialize(output, role, desktop_name),
    );
    if let Err(error) = &result {
        emit(
            output,
            json!({"kind": "initialization_failed", "role": role,
            "stage": if slot.is_some() { "readiness" } else { "launch" },
            "started_us": started_us, "finished_us": timestamp(), "error": error.to_string(),
            "resource_owned": slot.is_some()}),
        )?;
    }
    result
}

fn destroy(output: &mut File, window: &mut Option<OwnedWindow>, role: &str) -> Result<()> {
    if let Some(mut window) = window.take() {
        let identity = json!({"handle": number(window.0), "process": std::process::id(),
            "thread": unsafe { GetCurrentThreadId() }});
        let started_us = timestamp();
        let result = unsafe { DestroyWindow(window.0) };
        let returned_us = timestamp();
        let absent = !unsafe { IsWindow(Some(window.0)) }.as_bool();
        if result.is_ok() {
            window.0 = HWND::default();
        }
        emit(
            output,
            json!({"kind": "window_retired", "role": role, "identity": identity,
            "api": "DestroyWindow", "started_us": started_us, "returned_us": returned_us,
            "ok": result.is_ok(), "absent": absent, "checked_us": timestamp()}),
        )?;
        result?;
        if !absent {
            return Err("local window survived destruction".into());
        }
    }
    Ok(())
}

pub(super) fn measure(trial: &mut Trial<'_>) -> Result<()> {
    let owned = trial.owned.ok_or("fresh trial missing O")?;
    let handles = FixtureSet {
        handles: vec![
            trial.a.window,
            trial.b.window,
            trial.normal.window,
            trial.overlay,
            owned,
        ],
        ownership: Some((number(owned), number(trial.overlay))),
    };
    source_place(
        trial.output,
        trial.normal,
        HWND_BOTTOM,
        &handles,
        "setup_p_bottom",
    )?;
    source_place(
        trial.output,
        trial.b,
        HWND_TOPMOST,
        &handles,
        "setup_b_topmost",
    )?;
    source_place(
        trial.output,
        trial.a,
        HWND_TOPMOST,
        &handles,
        "setup_a_topmost",
    )?;
    pump();
    thread::sleep(Duration::from_millis(50));
    let before = snapshot(&handles, true, true);
    emit(
        trial.output,
        json!({"kind": "before", "snapshot": before,
        "attributes": handles.handles.iter().copied().map(attributes).collect::<Result<Vec<_>>>()?}),
    )?;
    let topmost = !trial.case.starts_with("fresh_owner_promotion");
    let fixture_order: Vec<_> = before
        .global_order
        .iter()
        .filter(|item| handles.handles.contains(&handle(item.handle)))
        .map(|item| item.handle)
        .collect();
    let expected_order = [
        trial.a.window,
        trial.b.window,
        owned,
        trial.overlay,
        trial.normal.window,
    ]
    .map(number);
    let expected_bands = [true, true, false, topmost, topmost];
    let reason = if before.invalid_reason.is_some() {
        before.invalid_reason
    } else if fixture_order != expected_order {
        Some("fresh fixture order is not A-B-O-X-P")
    } else if before
        .fixtures
        .iter()
        .map(|item| item.topmost)
        .collect::<Vec<_>>()
        != expected_bands
    {
        Some("fresh initial bands differ from premise")
    } else if before.global_order.iter().any(|window| {
        window.owner != 0
            && handles.handles.contains(&handle(window.owner))
            && window.handle != number(owned)
    }) {
        Some("unscheduled owned window in global premise")
    } else if before.fixture_process_classes.len() != 5 {
        Some("fixture process inventory contains extra or missing windows")
    } else {
        None
    };
    emit(
        trial.output,
        json!({"kind": "setup_eligibility", "eligible": reason.is_none(), "reason": reason}),
    )?;
    if let Some(reason) = reason {
        return Err(reason.into());
    }
    let before_insert = snapshot(&handles, false, false);
    emit(
        trial.output,
        json!({"kind": "before_insert", "snapshot": before_insert}),
    )?;
    if before_insert.invalid_reason.is_some() || before_insert.fixtures != before.fixtures {
        return Err("fixture state changed before treatment".into());
    }
    let (target, after) = if trial.case.starts_with("fresh_owner_promotion") {
        (trial.overlay, HWND_TOPMOST)
    } else if trial.case == "fresh_owned_demotion" {
        (owned, HWND_NOTOPMOST)
    } else {
        (trial.overlay, trial.a.window)
    };
    treatment(
        trial.output,
        target,
        after,
        None,
        &handles,
        "immediate_after_local_write",
        trial.case.ends_with("noownerzorder"),
    )?;
    settle(trial.output, &handles, "settling")
}

fn member(
    output: &mut File,
    directory: &Path,
    member: &Member,
    desktop_name: &str,
    desktop_handle: usize,
) -> Result<()> {
    let connected = crate::desktop::Desktop::attach(desktop_handle, desktop_name)?;
    let input_method = disable_input_method();
    let mut begin = serde_json::to_value(member)?;
    begin["kind"] = json!("trial_begin");
    begin["fixture_slots"] = json!(["A", "B", "P", "X", "O"]);
    begin["owned_by"] = json!("X");
    emit(output, begin)?;
    emit(
        output,
        json!({"kind": "trial_thread_ready", "desktop": connected, "input_method": input_method,
        "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() }}),
    )?;
    let mut a = None;
    let mut b = None;
    let mut predecessor = None;
    let mut overlay = None;
    let mut owned = None;
    let result = (|| -> Result<()> {
        if input_method["ok"] != true {
            return Err("fresh thread IME preflight failed".into());
        }
        spawn_tracked(output, &mut a, "A", directory, desktop_name)?;
        spawn_tracked(output, &mut b, "B", directory, desktop_name)?;
        spawn_tracked(output, &mut predecessor, "P", directory, desktop_name)?;
        let creation = create_tracked(&mut overlay, true, None, Some((output, "X")))?;
        let overlay_handle = overlay.as_ref().expect("created X").0;
        emit(
            output,
            json!({"kind": "creation", "role": "X", "creation": creation}),
        )?;
        let base = FixtureSet {
            handles: vec![
                a.as_ref().expect("created A").window,
                b.as_ref().expect("created B").window,
                predecessor.as_ref().expect("created P").window,
                overlay_handle,
            ],
            ownership: None,
        };
        emit(
            output,
            json!({"kind": "fresh_inventory", "snapshot": snapshot(&base, false, true)}),
        )?;
        if member.family != "owner_promotion" {
            let promoted = local_place(
                output,
                overlay_handle,
                HWND_TOPMOST,
                &base,
                "owner_promoted_before_creation",
            )?;
            if promoted.invalid_reason.is_some() || !promoted.fixtures[3].topmost {
                return Err("fresh unowned owner promotion unverified".into());
            }
        }
        let creation = create_tracked(&mut owned, true, Some(overlay_handle), Some((output, "O")))?;
        let owned_handle = owned.as_ref().expect("created O").0;
        emit(
            output,
            json!({"kind": "creation", "role": "O", "creation": creation}),
        )?;
        let handles = FixtureSet {
            handles: base.handles.into_iter().chain([owned_handle]).collect(),
            ownership: Some((number(owned_handle), number(overlay_handle))),
        };
        let created = snapshot(&handles, false, true);
        emit(
            output,
            json!({"kind": "after_owned_creation", "snapshot": created}),
        )?;
        if created.invalid_reason.is_some() || created.fixture_process_classes.len() != 5 {
            return Err("owned creation observation or inventory invalid".into());
        }
        if member.family != "owner_promotion"
            && !both_topmost_owned_pair(
                &created.fixtures,
                number(overlay_handle),
                number(owned_handle),
            )
        {
            return Err("fresh precreation promotion did not establish both topmost".into());
        }
        if member.family == "owner_promotion"
            && (created.fixtures[3].topmost || created.fixtures[4].topmost)
        {
            return Err("fresh owner promotion requires normal X and O".into());
        }
        Trial {
            output,
            a: a.as_mut().expect("created A"),
            b: b.as_mut().expect("created B"),
            normal: predecessor.as_mut().expect("created P"),
            other: None,
            overlay: overlay_handle,
            owned: Some(owned_handle),
            case: member.case,
        }
        .collect()
    })();
    let mut cleanup_errors = Vec::new();
    for (window, role) in [(&mut owned, "O"), (&mut overlay, "X")] {
        if let Err(error) = destroy(output, window, role) {
            cleanup_errors.push(error.to_string());
        }
    }
    for (fixture, role) in [(&mut predecessor, "P"), (&mut b, "B"), (&mut a, "A")] {
        if let Some(fixture) = fixture.as_mut() {
            let stopped = fixture.stop();
            emit(
                output,
                json!({"kind": "process_retired", "role": role, "process": fixture.child.id(),
                "thread": fixture.thread, "handle": number(fixture.window), "finished_us": timestamp(),
                "exited": stopped.is_ok(), "error": stopped.as_ref().err().map(ToString::to_string)}),
            )?;
            if let Err(error) = stopped {
                cleanup_errors.push(error.to_string());
            }
        }
    }
    emit(
        output,
        json!({"kind": "trial_end", "case": member.case, "environment": member.environment,
        "repetition": member.repetition, "lifetime_id": member.lifetime_id,
        "status": if result.is_ok() && cleanup_errors.is_empty() { "collected_pending_independent_analysis" } else { "ineligible_or_failed" },
        "error": result.as_ref().err().map(ToString::to_string), "cleanup_errors": cleanup_errors}),
    )?;
    if !cleanup_errors.is_empty() {
        return Err("fresh fixture retirement failed, admission stopped".into());
    }
    Ok(())
}

pub(super) fn collect_environment(
    output: &mut File,
    directory: &Path,
    repetitions: u32,
    environment: &'static str,
    desktop_name: &str,
    desktop_handle: usize,
) -> Result<()> {
    emit(
        output,
        json!({"kind": "environment_ready", "environment": environment, "desktop_name": desktop_name}),
    )?;
    let beginning = Instant::now();
    for planned in schedule(repetitions)
        .into_iter()
        .filter(|member| member.environment == environment)
    {
        if beginning.elapsed() >= Duration::from_secs(u64::from(repetitions) * 240 - 20) {
            return Err("environment exceeded admission limit".into());
        }
        let member_directory = directory.join(format!("{}-{}", planned.repetition, planned.case));
        std::fs::create_dir(&member_directory)?;
        let mut worker_output = output.try_clone()?;
        let name = desktop_name.to_owned();
        let lifetime_id = planned.lifetime_id.clone();
        let outcome = thread::spawn(move || {
            member(
                &mut worker_output,
                &member_directory,
                &planned,
                &name,
                desktop_handle,
            )
            .map_err(|error| error.to_string())
        })
        .join()
        .unwrap_or_else(|_| Err("fresh fixture thread panicked".to_owned()));
        emit(
            output,
            json!({"kind": "trial_thread_retired", "lifetime_id": lifetime_id, "joined": true,
                "error": outcome.as_ref().err()}),
        )?;
        outcome?;
    }
    Ok(())
}
