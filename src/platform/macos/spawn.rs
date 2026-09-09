//! macOS keys TCC permission prompts and grants to a child's responsible
//! process, which defaults to its parent. Without disclaiming, every child
//! Dome spawns gets attributed to Dome.
//!
//! `responsibility_spawnattrs_setdisclaim` is the private libsystem_secinit
//! call that severs this. Stable since macOS 10.14, shipped in LLDB. Resolved
//! via dlsym so its disappearance on a future macOS only degrades to the
//! pre-fix behavior. Revisit if
//! `nm -gU /usr/lib/system/libsystem_secinit.dylib | grep setdisclaim` stops
//! listing it.
use std::collections::{BTreeMap, HashMap};
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};

pub(super) fn spawn_disclaimed_sh(
    command: &str,
    env: &HashMap<String, String>,
) -> Result<libc::pid_t> {
    let path = CString::new("/bin/sh").context("CString /bin/sh")?;
    let arg0 = CString::new("sh").context("CString argv[0]")?;
    let arg1 = CString::new("-c").context("CString -c")?;
    let arg2 = CString::new(command).context("CString command")?;
    let mut argv: [*mut c_char; 4] = [
        arg0.as_ptr() as *mut c_char,
        arg1.as_ptr() as *mut c_char,
        arg2.as_ptr() as *mut c_char,
        ptr::null_mut(),
    ];
    // launchd starts Dome with a minimal environment, so a spawned command
    // inherits that rather than the login shell. Layer the user's `env`
    // overrides on top so PATH and similar resolve.
    let env_block = merge_env(std::env::vars(), env);
    let mut envp: Vec<*mut c_char> = env_block
        .iter()
        .map(|e| e.as_ptr() as *mut c_char)
        .collect();
    envp.push(ptr::null_mut());

    let mut attrs: libc::posix_spawnattr_t = ptr::null_mut();
    let rc = unsafe { libc::posix_spawnattr_init(&mut attrs) };
    if rc != 0 {
        return Err(anyhow!("posix_spawnattr_init: {rc}"));
    }
    let _guard = AttrsGuard(&mut attrs as *mut _);

    let rc =
        unsafe { libc::posix_spawnattr_setflags(&mut attrs, libc::POSIX_SPAWN_SETPGROUP as i16) };
    if rc != 0 {
        return Err(anyhow!("posix_spawnattr_setflags: {rc}"));
    }
    let rc = unsafe { libc::posix_spawnattr_setpgroup(&mut attrs, 0) };
    if rc != 0 {
        return Err(anyhow!("posix_spawnattr_setpgroup: {rc}"));
    }
    if let Some(set_disclaim) = resolve_setdisclaim() {
        let rc = unsafe { set_disclaim(&mut attrs, true) };
        if rc != 0 {
            tracing::warn!(rc, "TCC disclaim attr returned non-zero");
        }
    }

    let mut pid: libc::pid_t = 0;
    let rc = unsafe {
        libc::posix_spawn(
            &mut pid,
            path.as_ptr(),
            ptr::null(),
            &attrs,
            argv.as_mut_ptr(),
            envp.as_ptr(),
        )
    };
    if rc != 0 {
        return Err(anyhow!("posix_spawn /bin/sh: {rc}"));
    }
    Ok(pid)
}

// BTreeMap dedupes by key, so a user override replaces the inherited value,
// and its ordering keeps the block stable for tests.
fn merge_env(
    base: impl Iterator<Item = (String, String)>,
    overrides: &HashMap<String, String>,
) -> Vec<CString> {
    let mut merged: BTreeMap<String, String> = base.collect();
    for (key, value) in overrides {
        merged.insert(key.clone(), value.clone());
    }
    merged
        .into_iter()
        .filter_map(|(key, value)| CString::new(format!("{key}={value}")).ok())
        .collect()
}

type SetDisclaimFn = unsafe extern "C" fn(*mut libc::posix_spawnattr_t, bool) -> c_int;

struct AttrsGuard(*mut libc::posix_spawnattr_t);

impl Drop for AttrsGuard {
    fn drop(&mut self) {
        unsafe {
            libc::posix_spawnattr_destroy(self.0);
        }
    }
}

fn resolve_setdisclaim() -> Option<SetDisclaimFn> {
    static CACHED: OnceLock<Option<SetDisclaimFn>> = OnceLock::new();
    *CACHED.get_or_init(|| {
        for name in [
            c"responsibility_spawnattrs_setdisclaim_v2",
            c"responsibility_spawnattrs_setdisclaim",
        ] {
            let p = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
            if !p.is_null() {
                let f: SetDisclaimFn =
                    unsafe { std::mem::transmute::<*mut std::ffi::c_void, SetDisclaimFn>(p) };
                tracing::debug!(symbol = %name.to_string_lossy(), "Resolved TCC disclaim symbol");
                return Some(f);
            }
        }
        tracing::warn!(
            "No TCC disclaim symbol available, spawned children may inherit Dome's TCC attribution"
        );
        None
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_env_overrides_inherited_and_appends_new() {
        let base = [
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("HOME".to_string(), "/Users/x".to_string()),
        ]
        .into_iter();
        let mut overrides = HashMap::new();
        overrides.insert("PATH".to_string(), "/opt/homebrew/bin:/usr/bin".to_string());
        overrides.insert("EDITOR".to_string(), "nvim".to_string());

        let block: Vec<String> = merge_env(base, &overrides)
            .into_iter()
            .map(|c| c.into_string().unwrap())
            .collect();

        assert!(block.contains(&"PATH=/opt/homebrew/bin:/usr/bin".to_string()));
        assert!(block.contains(&"HOME=/Users/x".to_string()));
        assert!(block.contains(&"EDITOR=nvim".to_string()));
        assert_eq!(block.len(), 3);
    }
}
