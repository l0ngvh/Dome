use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use calloop::{EventLoop, LoopSignal, timer::{TimeoutAction, Timer}};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::glow::GlowRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::winit::platform::pump_events::PumpStatus;
use smithay::utils::Transform;

use super::CalloopData;

pub(super) struct WinitBackendData {
    pub(super) backend: WinitGraphicsBackend<GlowRenderer>,
    pub(super) output: Output,
    pub(super) damage_tracker: OutputDamageTracker,
    pub(super) gl: Arc<glow::Context>,
}

pub(super) fn init_winit_backend(
    event_loop: &mut EventLoop<CalloopData>,
    loop_signal: LoopSignal,
) -> Result<WinitBackendData> {
    let (mut backend, mut winit_evt) = winit::init::<GlowRenderer>()
        .map_err(|e| anyhow!("winit init failed: {e}"))?;

    let gl = backend.renderer().with_context(|gl| gl.clone())
        .map_err(|e| anyhow!("failed to get glow context: {e}"))?;

    let size = backend.window_size();
    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Dome".into(),
            model: "Winit".into(),
        },
    );
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    let damage_tracker = OutputDamageTracker::from_output(&output);

    let timer = Timer::immediate();
    event_loop.handle().insert_source(timer, move |_, _, data| {
        let status = winit_evt.dispatch_new_events(|event| {
            handle_winit_event(&mut data.state, event);
        });

        if let PumpStatus::Exit(_) = status {
            loop_signal.stop();
            return TimeoutAction::Drop;
        }

        data.state.render_winit();
        TimeoutAction::ToDuration(Duration::from_millis(16))
    }).map_err(|e| anyhow!("failed to insert timer source: {e}"))?;

    Ok(WinitBackendData {
        backend,
        output,
        damage_tracker,
        gl,
    })
}

fn handle_winit_event(state: &mut super::state::DomeState, event: WinitEvent) {
    match event {
        WinitEvent::Resized { size, .. } => {
            let mode = Mode {
                size,
                refresh: 60_000,
            };
            if let Some(ref mut winit_data) = state.winit_data {
                winit_data.output.change_current_state(Some(mode), None, None, None);
                winit_data.output.set_preferred(mode);
                winit_data.damage_tracker = OutputDamageTracker::from_output(&winit_data.output);
            }
            // Re-arrange layers for new output size, then update hub with usable area
            if let Some(output) = state.get_output() {
                smithay::desktop::layer_map_for_output(&output).arrange();
            }
            state.update_usable_area();
        }
        WinitEvent::Input(event) => {
            state.process_input_event(event);
        }
        WinitEvent::CloseRequested => {
            state.loop_signal.stop();
        }
        _ => {}
    }
}
