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
use std::collections::HashMap;
use std::ffi::{CStr, CString};
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
    let merged = merge_environ(env);
    let mut merged_pointers: Vec<*mut c_char> = Vec::new();
    let envp: *const *mut c_char = if let Some(entries) = &merged {
        merged_pointers.extend(entries.iter().map(|entry| entry.as_ptr() as *mut c_char));
        merged_pointers.push(ptr::null_mut());
        merged_pointers.as_ptr()
    } else {
        unsafe { *libc::_NSGetEnviron() as *const *mut c_char }
    };

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
            envp,
        )
    };
    if rc != 0 {
        return Err(anyhow!("posix_spawn /bin/sh: {rc}"));
    }
    Ok(pid)
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

/// The child's environment, with `overrides` layered over Dome's own. `None`
/// when nothing is overridden, so the caller inherits the process environment.
fn merge_environ(overrides: &HashMap<String, String>) -> Option<Vec<CString>> {
    if overrides.is_empty() {
        return None;
    }
    Some(merge_entries(&current_environ(), overrides))
}

fn current_environ() -> Vec<Vec<u8>> {
    let mut entries = Vec::new();
    unsafe {
        let mut envp = *libc::_NSGetEnviron();
        if envp.is_null() {
            return entries;
        }
        while !(*envp).is_null() {
            entries.push(CStr::from_ptr(*envp).to_bytes().to_vec());
            envp = envp.add(1);
        }
    }
    entries
}

fn merge_entries(inherited: &[Vec<u8>], overrides: &HashMap<String, String>) -> Vec<CString> {
    let mut result = Vec::new();
    for entry in inherited {
        let name_end = entry.iter().position(|&b| b == b'=').unwrap_or(entry.len());
        let replaced =
            std::str::from_utf8(&entry[..name_end]).is_ok_and(|name| overrides.contains_key(name));
        if replaced {
            continue;
        }
        if let Ok(entry) = CString::new(entry.clone()) {
            result.push(entry);
        }
    }
    for (name, value) in overrides {
        match CString::new(format!("{name}={value}")) {
            Ok(entry) => result.push(entry),
            Err(_) => tracing::warn!(%name, "Skipping env override with an interior NUL byte"),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inherited(entries: &[&str]) -> Vec<Vec<u8>> {
        entries.iter().map(|e| e.as_bytes().to_vec()).collect()
    }

    fn names(result: &[CString]) -> Vec<String> {
        result
            .iter()
            .map(|c| c.to_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn merge_environ_inherits_when_nothing_is_overridden() {
        assert!(merge_environ(&HashMap::new()).is_none());
    }

    #[test]
    fn merge_entries_replaces_an_inherited_entry() {
        let base = inherited(&["PATH=/usr/bin", "HOME=/Users/x"]);
        let overrides = HashMap::from([("PATH".to_string(), "/opt/bin".to_string())]);
        let merged = names(&merge_entries(&base, &overrides));
        assert!(merged.contains(&"PATH=/opt/bin".to_string()));
        assert_eq!(merged.iter().filter(|e| e.starts_with("PATH=")).count(), 1);
    }

    #[test]
    fn merge_entries_keeps_an_entry_no_override_names() {
        let base = inherited(&["PATH=/usr/bin", "HOME=/Users/x"]);
        let overrides = HashMap::from([("PATH".to_string(), "/opt/bin".to_string())]);
        let merged = names(&merge_entries(&base, &overrides));
        assert!(merged.contains(&"HOME=/Users/x".to_string()));
    }

    #[test]
    fn merge_entries_adds_an_override_not_inherited() {
        let base = inherited(&["PATH=/usr/bin"]);
        let overrides = HashMap::from([("DOME_TEST".to_string(), "1".to_string())]);
        let merged = names(&merge_entries(&base, &overrides));
        assert!(merged.contains(&"DOME_TEST=1".to_string()));
    }
}
