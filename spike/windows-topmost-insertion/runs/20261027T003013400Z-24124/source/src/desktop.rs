use serde_json::{Value, json};
use windows::{
    Win32::System::{
        StationsAndDesktops::{
            CloseDesktop, CreateDesktopW, DESKTOP_CONTROL_FLAGS, DESKTOP_CREATEMENU,
            DESKTOP_CREATEWINDOW, DESKTOP_ENUMERATE, DESKTOP_READOBJECTS, DESKTOP_WRITEOBJECTS,
            GetThreadDesktop, HDESK, OpenDesktopW, SetThreadDesktop,
        },
        Threading::GetCurrentThreadId,
    },
    core::PCWSTR,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const ACCESS: u32 = DESKTOP_CREATEMENU.0
    | DESKTOP_CREATEWINDOW.0
    | DESKTOP_ENUMERATE.0
    | DESKTOP_READOBJECTS.0
    | DESKTOP_WRITEOBJECTS.0;

pub(crate) struct Desktop {
    handle: HDESK,
    name: String,
    owned: bool,
}

impl Desktop {
    pub(crate) fn current() -> Result<Self> {
        let identity = crate::native::desktop()?;
        let name = identity["name"]
            .as_str()
            .ok_or("missing current desktop name")?
            .to_owned();
        Ok(Self {
            handle: unsafe { GetThreadDesktop(GetCurrentThreadId()) }?,
            name,
            owned: false,
        })
    }

    pub(crate) fn open(name: &str) -> Result<Self> {
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe {
            OpenDesktopW(
                PCWSTR(wide.as_ptr()),
                DESKTOP_CONTROL_FLAGS(0),
                false,
                ACCESS,
            )
        }?;
        Ok(Self {
            handle,
            name: name.to_owned(),
            owned: true,
        })
    }

    pub(crate) fn create(name: &str) -> Result<Self> {
        let original = unsafe { GetThreadDesktop(GetCurrentThreadId()) }?;
        let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
        let handle = unsafe {
            CreateDesktopW(
                PCWSTR(wide.as_ptr()),
                None,
                None,
                DESKTOP_CONTROL_FLAGS(0),
                ACCESS,
                None,
            )
        }?;
        let created = Self {
            handle,
            name: name.to_owned(),
            owned: true,
        };
        unsafe { SetThreadDesktop(original) }?;
        Ok(created)
    }

    pub(crate) fn handle_number(&self) -> usize {
        self.handle.0 as usize
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn close(mut self) -> Result<()> {
        let owned = self.owned;
        self.owned = false;
        if owned {
            unsafe { CloseDesktop(self.handle) }?;
        }
        Ok(())
    }

    pub(crate) fn attach(handle: usize, expected: &str) -> Result<Value> {
        unsafe { SetThreadDesktop(HDESK(handle as *mut std::ffi::c_void)) }?;
        let actual = crate::native::desktop()?;
        if actual["name"].as_str() != Some(expected) {
            return Err("assigned desktop name differs from requested desktop".into());
        }
        Ok(
            json!({"requested": expected, "actual": actual, "verified": true,
            "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() },
            "requested_open_access": ACCESS, "switch_requested": false}),
        )
    }

    pub(crate) fn keep_until_process_exit(mut self) {
        // CloseDesktop rejects an assigned handle and the process's initial desktop.
        self.owned = false;
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        if self.owned
            && let Err(error) = unsafe { CloseDesktop(self.handle) }
        {
            eprintln!("CloseDesktop failed: {error}");
        }
    }
}
