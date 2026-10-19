use super::dispatcher::DispatcherMarker;
use super::dome::Dome;
use super::event_loop::{NativeEvent, RunLoop};
use super::handle;
use dome_core::PixelRect;
use futures::channel::mpsc::UnboundedReceiver;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::future::Future;
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::core::BOOL;

#[derive(Clone)]
pub(crate) struct MonitorInfo {
    pub(crate) handle: isize,
    /// Friendly display name, or the GDI device identifier when unavailable.
    pub(crate) name: String,
    pub(crate) gdi_device: String,
    pub(crate) work_area: PixelRect,
    pub(crate) is_primary: bool,
    /// Always > 0.
    pub(crate) scale: f64,
}

const BASE_DPI: f64 = 96.0;

impl Dome {
    pub(in crate::platform::windows) fn display_changed(
        &mut self,
        monitors: Vec<MonitorInfo>,
        exclusive: Option<dome_core::WindowId>,
    ) {
        let windows = self.apply_display_change(monitors, exclusive);
        self.window_server.request_constraints(windows);
    }
}

pub(super) fn register(
    events: UnboundedReceiver<NativeEvent>,
    dpi_changes: UnboundedReceiver<NativeEvent>,
) {
    let refreshed = refreshes(
        futures::stream::select(events, dpi_changes),
        || async { RunLoop::dispatch(get_all_monitors).await.ok()?.ok() },
        || async {
            let server = RunLoop::with_dome(|dome| dome.window_server.clone());
            server
                .get_exclusive_fullscreen_window()
                .await
                .ok()
                .flatten()
        },
    );
    RunLoop::insert(refreshed, |(monitors, exclusive), dome| {
        dome.display_changed(monitors, exclusive);
        dome.update_tray();
    });
}

pub(super) fn get_all_monitors(_marker: &DispatcherMarker) -> anyhow::Result<Vec<MonitorInfo>> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();

    unsafe extern "system" fn enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorInfo>) };
        let mut info = MONITORINFOEXW {
            monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        if unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) }.as_bool() {
            let rc = info.monitorInfo.rcWork;
            let gdi_device = utf16_to_string(&info.szDevice);

            let scale = scale_for_monitor(hmonitor);

            monitors.push(MonitorInfo {
                handle: hmonitor.0 as isize,
                name: String::new(),
                gdi_device,
                work_area: handle::rect_to_pixel_rect(rc),
                is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
                scale,
            });
        }
        BOOL(1)
    }

    let success = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        )
    };
    anyhow::ensure!(success.as_bool(), "EnumDisplayMonitors failed");

    let friendly = friendly_names_by_gdi_device();
    for monitor in &mut monitors {
        monitor.name = friendly
            .get(&monitor.gdi_device)
            .cloned()
            .unwrap_or_else(|| monitor.gdi_device.clone());
    }

    Ok(monitors)
}

fn scale_for_monitor(hmonitor: HMONITOR) -> f64 {
    let mut dpi_x: u32 = 0;
    let mut dpi_y: u32 = 0;
    if let Err(e) = unsafe { GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }
    {
        tracing::warn!(%e, ?hmonitor, "GetDpiForMonitor failed, falling back to 1.0");
        return 1.0;
    }
    if dpi_x == 0 {
        tracing::warn!(
            ?hmonitor,
            "GetDpiForMonitor returned 0, falling back to 1.0"
        );
        return 1.0;
    }
    dpi_x as f64 / BASE_DPI
}

fn friendly_names_by_gdi_device() -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Display config can change between GetDisplayConfigBufferSizes and
    // QueryDisplayConfig, which then returns ERROR_INSUFFICIENT_BUFFER.
    // See https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig
    const MAX_RETRIES: u32 = 5;
    for _ in 0..MAX_RETRIES {
        let mut path_count: u32 = 0;
        let mut mode_count: u32 = 0;
        let sizes = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if sizes != ERROR_SUCCESS {
            tracing::warn!(
                error = ?sizes,
                "GetDisplayConfigBufferSizes failed, falling back to GDI device names"
            );
            return map;
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let query = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };
        if query == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if query != ERROR_SUCCESS {
            tracing::warn!(
                error = ?query,
                "QueryDisplayConfig failed, falling back to GDI device names"
            );
            return map;
        }

        paths.truncate(path_count as usize);
        for path in &paths {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            if unsafe { DisplayConfigGetDeviceInfo(&mut source.header) } != ERROR_SUCCESS.0 as i32 {
                continue;
            }
            let gdi_device = utf16_to_string(&source.viewGdiDeviceName);
            if gdi_device.is_empty() {
                continue;
            }

            let mut target = DISPLAYCONFIG_TARGET_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                    size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                ..Default::default()
            };
            if unsafe { DisplayConfigGetDeviceInfo(&mut target.header) } != ERROR_SUCCESS.0 as i32 {
                continue;
            }
            let friendly = utf16_to_string(&target.monitorFriendlyDeviceName);
            if !friendly.is_empty() {
                map.insert(gdi_device, friendly);
            }
        }
        return map;
    }

    map
}

fn utf16_to_string(units: &[u16]) -> String {
    let end = units.iter().take_while(|&&c| c != 0).count();
    String::from_utf16_lossy(&units[..end])
}

fn refreshes<Requests, ReadMonitors, Monitors, ReadExclusive, Exclusive>(
    requests: Requests,
    mut read_monitors: ReadMonitors,
    read_exclusive: ReadExclusive,
) -> impl Stream<Item = (Vec<MonitorInfo>, Option<dome_core::WindowId>)>
where
    Requests: Stream<Item = NativeEvent>,
    ReadMonitors: FnMut() -> Monitors,
    Monitors: Future<Output = Option<Vec<MonitorInfo>>>,
    ReadExclusive: Fn() -> Exclusive + Clone,
    Exclusive: Future<Output = Option<dome_core::WindowId>>,
{
    requests.filter_map(move |event| {
        let monitors = read_monitors();
        let read_exclusive = read_exclusive.clone();
        async move {
            let monitors = monitors.await?;
            let exclusive = if matches!(event, NativeEvent::DisplayChanged) {
                read_exclusive().await
            } else {
                None
            };
            Some((monitors, exclusive))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::{mpsc, oneshot};
    use futures::executor::block_on;
    use futures::future::{FutureExt, ready};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    fn topology(width: u32) -> Vec<MonitorInfo> {
        vec![MonitorInfo {
            handle: 1,
            name: "Display".into(),
            gdi_device: "device1".into(),
            work_area: PixelRect::new(0, 0, width, 100),
            is_primary: true,
            scale: 1.0,
        }]
    }

    #[test]
    fn refresh_while_exclusive_read_pending_applies_before_next_read() {
        for second_is_dpi in [false, true] {
            let (events, requests) = mpsc::unbounded();
            let (dpi_events, dpi_requests) = mpsc::unbounded();
            let (finish_exclusive, exclusive_result) = oneshot::channel();
            let exclusive_result = Rc::new(RefCell::new(Some(exclusive_result)));
            let reads = Cell::new(0);
            let applied_width = Cell::new(0);
            let source = refreshes(
                futures::stream::select(requests, dpi_requests),
                || {
                    let read = reads.get() + 1;
                    if read == 2 {
                        assert_eq!(applied_width.get(), 100, "T1 must apply before reading T2");
                    }
                    reads.set(read);
                    ready(Some(topology(read * 100)))
                },
                move || {
                    let result = exclusive_result.borrow_mut().take().unwrap();
                    async move { result.await.unwrap() }
                },
            );
            let mut source = Box::pin(source);
            events.unbounded_send(NativeEvent::DisplayChanged).unwrap();
            assert!(source.next().now_or_never().is_none());
            assert_eq!(reads.get(), 1);

            let second_sender = if second_is_dpi { &dpi_events } else { &events };
            second_sender
                .unbounded_send(NativeEvent::WorkAreaChanged)
                .unwrap();
            assert!(source.next().now_or_never().is_none());
            assert_eq!(reads.get(), 1);
            assert_eq!(applied_width.get(), 0);

            finish_exclusive.send(None).unwrap();
            let (monitors, exclusive) = block_on(source.next()).unwrap();
            assert!(exclusive.is_none());
            assert_eq!(reads.get(), 1);
            applied_width.set(monitors[0].work_area.width().value());
            assert_eq!(applied_width.get(), 100);

            let (monitors, exclusive) = block_on(source.next()).unwrap();
            assert!(exclusive.is_none());
            applied_width.set(monitors[0].work_area.width().value());
            assert_eq!(reads.get(), 2);
            assert_eq!(applied_width.get(), 200);
        }
    }

    #[test]
    fn refresh_after_failed_monitor_read_skips_exclusive_query_and_continues() {
        let reads = Cell::new(0);
        let source = refreshes(
            futures::stream::iter([NativeEvent::DisplayChanged, NativeEvent::WorkAreaChanged]),
            || {
                reads.set(reads.get() + 1);
                ready((reads.get() == 2).then(|| topology(200)))
            },
            || async { panic!("failed monitor read must not query exclusive fullscreen") },
        );
        let results = block_on(source.collect::<Vec<_>>());
        assert_eq!(reads.get(), 2);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0[0].work_area.width().value(), 200);
        assert!(results[0].1.is_none());
    }

    #[test]
    fn refresh_without_exclusive_window_delivers_topology() {
        let queries = Cell::new(0);
        let source = refreshes(
            futures::stream::iter([NativeEvent::DisplayChanged, NativeEvent::WorkAreaChanged]),
            || ready(Some(topology(100))),
            || {
                queries.set(queries.get() + 1);
                ready(None)
            },
        );
        let results = block_on(source.collect::<Vec<_>>());
        assert_eq!(queries.get(), 1);
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|(_, exclusive)| exclusive.is_none()));
    }
}
