use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::windows::{io::AsRawHandle, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
                QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
            },
            Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW},
            RemoteDesktop::ProcessIdToSessionId,
            SystemInformation::GetSystemTime,
        },
    },
    core::{PCWSTR, w},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub fn run(arguments: &[OsString]) -> Result<()> {
    let (requested, repetitions) = options(arguments)?;
    let directory = create_directory(requested)?;
    println!("Evidence directory: {}", directory.display());
    let started = utc();
    let outcome = (|| {
        File::create(directory.join("stderr.log"))?;
        write_json(
            &directory.join("cleanup.json"),
            &json!({"status": "not_finalized"}),
        )?;
        capture_sources(&directory)?;
        capture_environment(&directory, repetitions)?;
        collect(&directory, repetitions)
    })();
    let raw_hash = hash_raw(&directory);
    if let Err(error) = raw_hash {
        eprintln!("Native raw hash unavailable: {error}");
    }
    let eligibility_summary = summarize_collection(&directory);
    if let Err(error) = eligibility_summary {
        eprintln!("Eligibility summary unavailable: {error}");
    }
    if let Err(error) = &outcome {
        let saved = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("stderr.log"))
            .and_then(|mut log| writeln!(log, "launcher failed: {error}"));
        if let Err(save_error) = saved {
            eprintln!("Could not save launcher error: {save_error}");
        }
    }
    let exit_record = write_json(
        &directory.join("exit.json"),
        &json!({
            "started_utc": started, "ended_utc": utc(),
            "exit_code": if outcome.is_ok() { 0 } else { 1 },
            "error": outcome.as_ref().err().map(ToString::to_string),
            "collection": outcome.as_ref().ok(),
        }),
    );
    println!("Evidence saved to {}", directory.display());
    exit_record?;
    outcome?;
    println!(
        "Collection is not a result verdict. Analyze raw.jsonl independently with analyze.py."
    );
    Ok(())
}

fn summarize_collection(directory: &Path) -> Result<()> {
    let raw = BufReader::new(File::open(directory.join("raw.jsonl"))?);
    let mut attempted = 0;
    let mut scheduled = 0;
    let mut not_run = 0;
    let mut setup_accepted = 0;
    let mut collected = 0;
    let mut rejected = 0;
    let mut treatments = 0;
    let mut asynchronous_posts = 0;
    let mut settling_samples = 0;
    let mut reasons = std::collections::BTreeMap::<String, u32>::new();
    for line in raw.lines() {
        let envelope: Value = serde_json::from_str(&line?)?;
        let record = &envelope["record"];
        match record["kind"].as_str() {
            Some("session") => scheduled = record["schedule"].as_array().map_or(0, Vec::len),
            Some("trial_not_run") => {
                not_run += 1;
                *reasons
                    .entry(
                        record["reason"]
                            .as_str()
                            .unwrap_or("environment unavailable")
                            .to_owned(),
                    )
                    .or_default() += 1;
            }
            Some("trial_begin") => attempted += 1,
            Some("setup_eligibility") if record["eligible"] == true => setup_accepted += 1,
            Some("trial_end") if record["status"] == "collected_pending_independent_analysis" => {
                collected += 1
            }
            Some("trial_end") => {
                rejected += 1;
                *reasons
                    .entry(
                        record["error"]
                            .as_str()
                            .unwrap_or("release or collection failure")
                            .to_owned(),
                    )
                    .or_default() += 1;
            }
            Some("treatment_call") => treatments += 1,
            Some("external_async_call") => asynchronous_posts += 1,
            Some("settling") => settling_samples += 1,
            _ => (),
        }
    }
    let summary = json!({"scheduled": scheduled, "not_run": not_run, "attempted": attempted, "setup_accepted": setup_accepted,
        "collected_pending_analysis": collected, "rejected_or_failed": rejected,
        "synchronous_treatment_calls": treatments, "asynchronous_posts": asynchronous_posts,
        "settling_samples": settling_samples, "reasons": reasons});
    write_json(&directory.join("eligibility-summary.json"), &summary)?;
    println!(
        "Coverage scheduled {scheduled}, attempted {attempted}, not run {not_run}. Controls remain pending independent analysis."
    );
    println!(
        "Preflight/setup accepted {setup_accepted}/{attempted}. Collected pending analysis {collected}, rejected or failed {rejected}."
    );
    println!(
        "Recorded treatment calls {treatments}, asynchronous posts {asynchronous_posts}, settling samples {settling_samples}."
    );
    for (reason, count) in reasons {
        println!("  {count} rejected or failed: {reason}");
    }
    if treatments == 0 && asynchronous_posts == 0 {
        println!("NO TREATMENT EXECUTED. Collection completion is not a successful experiment.");
    }
    Ok(())
}

fn options(arguments: &[OsString]) -> Result<(Option<PathBuf>, u32)> {
    let mut directory = None;
    let mut repetitions = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--out") if directory.is_none() => {
                directory = Some(PathBuf::from(
                    arguments.next().ok_or("--out needs a new directory")?,
                ));
            }
            Some("--repetitions") if repetitions.is_none() => {
                repetitions = Some(
                    arguments
                        .next()
                        .and_then(|value| value.to_str())
                        .ok_or("--repetitions needs a count")?
                        .parse::<u32>()?,
                );
            }
            _ => {
                return Err(
                    "Usage: cargo run --release -- [--out NEW_DIRECTORY] [--repetitions 1..20]"
                        .into(),
                );
            }
        }
    }
    let repetitions = repetitions.unwrap_or(5);
    if !(1..=20).contains(&repetitions) {
        return Err("repetitions must be between 1 and 20".into());
    }
    Ok((directory, repetitions))
}

fn create_directory(requested: Option<PathBuf>) -> Result<PathBuf> {
    let directory = requested.unwrap_or_else(|| {
        Path::new("runs").join(format!(
            "{}-{}",
            utc().replace(['-', ':', '.'], ""),
            std::process::id()
        ))
    });
    let absolute = std::path::absolute(directory)?;
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&absolute).map_err(|error| {
        format!(
            "Cannot create new run directory {}: {error}",
            absolute.display()
        )
    })?;
    Ok(absolute)
}

fn utc() -> String {
    let time = unsafe { GetSystemTime() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        time.wYear,
        time.wMonth,
        time.wDay,
        time.wHour,
        time.wMinute,
        time.wSecond,
        time.wMilliseconds
    )
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut file = File::create(path)?;
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    file.write_all(&bytes)?;
    Ok(())
}

fn capture_sources(directory: &Path) -> Result<()> {
    let sources: &[(&str, &[u8])] = &[
        ("Cargo.toml", include_bytes!("../Cargo.toml")),
        ("Cargo.lock", include_bytes!("../Cargo.lock")),
        ("src/main.rs", include_bytes!("main.rs")),
        ("src/native.rs", include_bytes!("native.rs")),
        ("src/observation.rs", include_bytes!("observation.rs")),
        ("src/protocol.rs", include_bytes!("protocol.rs")),
        (
            "src/paired_protocol.rs",
            include_bytes!("paired_protocol.rs"),
        ),
        ("src/paired_native.rs", include_bytes!("paired_native.rs")),
        ("protocol_v7.py", include_bytes!("../protocol_v7.py")),
        ("analyze_v7.py", include_bytes!("../analyze_v7.py")),
        (
            "test_analyze_v7.py",
            include_bytes!("../test_analyze_v7.py"),
        ),
        ("src/desktop.rs", include_bytes!("desktop.rs")),
        ("analyze_v5.py", include_bytes!("../analyze_v5.py")),
        ("analyze_v6.py", include_bytes!("../analyze_v6.py")),
        ("protocol_v6.py", include_bytes!("../protocol_v6.py")),
        (
            "test_analyze_v6.py",
            include_bytes!("../test_analyze_v6.py"),
        ),
        (
            "test_analyze_v5.py",
            include_bytes!("../test_analyze_v5.py"),
        ),
        (
            "tests/scrambled-snapshot.json",
            include_bytes!("../tests/scrambled-snapshot.json"),
        ),
        (
            "verify_saved_run.py",
            include_bytes!("../verify_saved_run.py"),
        ),
        ("src/run_capture.rs", include_bytes!("run_capture.rs")),
        ("analyze.py", include_bytes!("../analyze.py")),
        ("test_analyze.py", include_bytes!("../test_analyze.py")),
        (
            "verify_output_reduction.py",
            include_bytes!("../verify_output_reduction.py"),
        ),
        ("README.md", include_bytes!("../README.md")),
        ("REPORT.md", include_bytes!("../REPORT.md")),
    ];
    fs::create_dir_all(directory.join("source/src"))?;
    fs::create_dir_all(directory.join("source/tests"))?;
    let mut hashes = Vec::new();
    for (name, bytes) in sources {
        fs::write(directory.join("source").join(name), bytes)?;
        hashes.push(json!({"file": name, "sha256": format!("{:x}", Sha256::digest(bytes))}));
    }
    let executable = fs::read(std::env::current_exe()?)?;
    fs::write(directory.join("windows-topmost-insertion.exe"), &executable)?;
    hashes.push(json!({"file": "windows-topmost-insertion.exe", "sha256": format!("{:x}", Sha256::digest(&executable))}));
    write_json(&directory.join("hashes.json"), &json!(hashes))
}

fn registry_value(name: PCWSTR, number: bool) -> Result<Value> {
    let mut buffer = [0u16; 512];
    let mut length = std::mem::size_of_val(&buffer) as u32;
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            name,
            if number {
                RRF_RT_REG_DWORD
            } else {
                RRF_RT_REG_SZ
            },
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut length),
        )
        .ok()?;
    }
    if number {
        Ok(json!(u32::from(buffer[0]) | (u32::from(buffer[1]) << 16)))
    } else {
        let end = buffer
            .iter()
            .position(|value| *value == 0)
            .ok_or("registry string not terminated")?;
        Ok(json!(String::from_utf16(&buffer[..end])?))
    }
}

fn available(result: Result<Value>) -> Value {
    result.unwrap_or_else(|error| json!({"unavailable": error.to_string()}))
}

fn capture_environment(directory: &Path, repetitions: u32) -> Result<()> {
    let mut session = 0;
    let session = unsafe { ProcessIdToSessionId(std::process::id(), &mut session) }
        .map(|()| json!(session))
        .map_err(Into::into);
    write_json(
        &directory.join("environment.json"),
        &json!({
            "utc": utc(), "process": std::process::id(),
            "process_session_id": available(session),
            "session_name": available(std::env::var("SESSIONNAME").map(|name| json!(name)).map_err(Into::into)),
            "desktop": available(crate::native::desktop()),
            "os_version_source": "HKLM\\SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion",
            "build_number": available(registry_value(w!("CurrentBuildNumber"), false)),
            "update_build_revision": available(registry_value(w!("UBR"), true)),
            "display_version": available(registry_value(w!("DisplayVersion"), false)),
            "product_name": available(registry_value(w!("ProductName"), false)),
            "rustc": available(toolchain(directory, "rustc", &["--version", "--verbose"])),
            "cargo": available(toolchain(directory, "cargo", &["--version"])),
            "toolchain_scope": "Tools on PATH at run time, not compiler provenance",
            "source_capture": "Bytes embedded at compilation, not the run-time working tree",
            "build_capture": "Cargo builds before this process starts. Build logs and build timeout are not captured",
            "repetitions": repetitions, "scenario_count": 5, "environment_case_count": 10,
            "protocol": "fresh-ownership-v7",
        }),
    )
}

fn toolchain(directory: &Path, program: &str, arguments: &[&str]) -> Result<Value> {
    let stdout = directory.join(format!("{program}-stdout.log"));
    let stderr = directory.join(format!("{program}-stderr.log"));
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(File::create(&stdout)?)
        .stderr(File::create(&stderr)?)
        .spawn()?;
    let status = match wait(&mut child, Duration::from_secs(5))? {
        Some(status) => status,
        None => {
            child.kill()?;
            return Err("toolchain query exceeded five seconds".into());
        }
    };
    Ok(
        json!({"exit_code": status.code(), "stdout": fs::read_to_string(stdout)?, "stderr": fs::read_to_string(stderr)?}),
    )
}

struct CollectionJob(HANDLE);

impl CollectionJob {
    fn create() -> Result<Self> {
        let job = Self(unsafe { CreateJobObjectW(None, None) }?);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of_val(&limits) as u32,
            )?;
        }
        Ok(job)
    }

    fn active_processes(&self) -> Result<u32> {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(self.0),
                JobObjectBasicAccountingInformation,
                &mut accounting as *mut _ as *mut _,
                std::mem::size_of_val(&accounting) as u32,
                None,
            )?;
        }
        Ok(accounting.ActiveProcesses)
    }
}

impl Drop for CollectionJob {
    fn drop(&mut self) {
        if let Err(error) = unsafe { CloseHandle(self.0) } {
            eprintln!("collection job close failed: {error}");
        }
    }
}

fn wait(child: &mut Child, limit: Duration) -> Result<Option<ExitStatus>> {
    let beginning = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if beginning.elapsed() >= limit {
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(25));
    }
}

fn collect(directory: &Path, repetitions: u32) -> Result<Value> {
    let job = CollectionJob::create()?;
    let mut child = Command::new(std::env::current_exe()?)
        .creation_flags(0x08000000)
        .arg("--collect")
        .arg(directory)
        .arg(repetitions.to_string())
        .stdin(Stdio::piped())
        .stdout(File::create(directory.join("stdout.log"))?)
        .stderr(
            OpenOptions::new()
                .append(true)
                .open(directory.join("stderr.log"))?,
        )
        .spawn()?;
    let limit_seconds = u64::from(repetitions) * 480;
    let outcome = (|| -> Result<Option<ExitStatus>> {
        unsafe { AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())) }?;
        let mut input = child.stdin.take().expect("piped collector stdin");
        input.write_all(b"start\n")?;
        input.flush()?;
        drop(input);
        wait(&mut child, Duration::from_secs(limit_seconds))
    })();
    let before = available(job.active_processes().map(|count| json!(count)));
    let mut errors = Vec::new();
    if let Err(error) = unsafe { TerminateJobObject(job.0, 1) } {
        errors.push(format!("terminate job: {error}"));
    }
    match child.try_wait() {
        Ok(None) => {
            if let Err(error) = child.kill() {
                errors.push(format!("kill collector: {error}"));
            }
        }
        Ok(Some(_)) => (),
        Err(error) => errors.push(format!("query collector exit: {error}")),
    }
    let child_status = match wait(&mut child, Duration::from_secs(5)) {
        Ok(Some(status)) => status.code(),
        Ok(None) => {
            errors.push("collector did not exit within five seconds".to_owned());
            None
        }
        Err(error) => {
            errors.push(error.to_string());
            None
        }
    };
    let beginning = Instant::now();
    let after = loop {
        match job.active_processes() {
            Ok(0) => break Some(0),
            Ok(count) if beginning.elapsed() >= Duration::from_secs(5) => {
                errors.push(format!(
                    "{count} job processes remain after cleanup deadline"
                ));
                break Some(count);
            }
            Ok(_) => thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                errors.push(error.to_string());
                break None;
            }
        }
    };
    write_json(
        &directory.join("cleanup.json"),
        &json!({
            "ended_utc": utc(), "method": "terminate owned collection job, kill on job close",
            "active_processes_before": before, "active_processes_after": after, "errors": errors,
            "native_destruction": "Inspect fixture stderr and stderr.log, zero processes is not proof of successful DestroyWindow",
        }),
    )?;
    let record = json!({
        "process": child.id(), "finished_within_limit": matches!(&outcome, Ok(Some(_))),
        "limit_seconds": limit_seconds, "exit_code": child_status,
        "error": outcome.as_ref().err().map(ToString::to_string),
    });
    write_json(&directory.join("collection-exit.json"), &record)?;
    let status = outcome?.ok_or("collection timed out, preserve the entire evidence directory")?;
    if !status.success() || !errors.is_empty() {
        return Err("collection or cleanup failed, preserve the entire evidence directory".into());
    }
    Ok(record)
}

fn hash_raw(directory: &Path) -> Result<()> {
    let cleanup: Value = serde_json::from_reader(File::open(directory.join("cleanup.json"))?)?;
    if cleanup["active_processes_after"] != 0 {
        return Err("cannot hash raw while collection processes may still exist".into());
    }
    let mut raw = File::open(directory.join("raw.jsonl"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let read = raw.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    write_json(
        &directory.join("raw-native-sha256.json"),
        &json!({
            "file": "raw.jsonl", "sha256": format!("{:x}", hash.finalize()),
            "source": "native supervisor after collector exit and job cleanup", "utc": utc()
        }),
    )
}
