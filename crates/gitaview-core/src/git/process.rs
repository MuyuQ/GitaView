//! 子进程树处理：deadline 同时覆盖进程退出和管道读取。
//! Windows 用 Job Object 保证子孙进程一起终止、CREATE_NO_WINDOW 防闪控制台；
//! Unix 用进程组；管道按块读取，连续输出不会饿死 deadline。

use std::io::{self, Read};
use std::process::{Child, Command};

#[cfg(all(test, target_os = "windows"))]
type AfterSpawnHook = Box<dyn FnOnce(&mut Child, bool)>;
#[cfg(all(test, target_os = "windows"))]
thread_local! {
    static AFTER_SPAWN: std::cell::RefCell<Option<AfterSpawnHook>> = const { std::cell::RefCell::new(None) };
}

/// 子进程生成模式（N14 排障结论，见 process.rs Windows 分支注释）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnMode {
    /// 挂起入 Job：网络操作安全路径（Windows 上每次生成一次全系统线程快照）。
    Suspended,
    /// 读操作低开销路径：只捕获新进程的线程，不遍历全系统线程。
    /// 与 Suspended 一样，执行用户代码之前必须先完成 Job 入组。
    Direct,
}

#[cfg(unix)]
pub struct ProcessTree(u32);

#[cfg(unix)]
impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        // Unix 进程组覆盖两种模式：mode 仅在 Windows 语义上有别。
        Self::spawn_with_mode(command, SpawnMode::Direct)
    }

    pub fn spawn_with_mode(command: &mut Command, _mode: SpawnMode) -> io::Result<(Child, Self)> {
        use std::os::unix::process::CommandExt;
        let child = command.process_group(0).spawn()?;
        let tree = Self(child.id());
        Ok((child, tree))
    }

    pub fn terminate(&self) {
        // The group remains addressable after the original process has exited.
        unsafe {
            libc::kill(-(self.0 as i32), libc::SIGKILL);
        }
    }
}

#[cfg(target_os = "windows")]
pub struct ProcessTree(std::os::windows::io::OwnedHandle);

#[cfg(target_os = "windows")]
impl ProcessTree {
    pub fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        Self::spawn_with_mode(command, SpawnMode::Suspended)
    }

    pub fn spawn_with_mode(command: &mut Command, mode: SpawnMode) -> io::Result<(Child, Self)> {
        use std::os::windows::{
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
            process::CommandExt,
        };
        use windows::Win32::{
            Foundation::HANDLE,
            System::{
                JobObjects::{
                    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                    SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                },
                Threading::{CREATE_NO_WINDOW, CREATE_SUSPENDED},
            },
        };
        let job = unsafe { CreateJobObjectW(None, None) }.map_err(io::Error::other)?;
        let tree = Self(unsafe { OwnedHandle::from_raw_handle(job.0) });
        // 宿主意外退出时由内核终止整棵 Git 子进程树：不能留下孤儿 fetch/pull
        // 继续写用户工作区。
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(io::Error::other)?;
        // Both modes must join the job before user code can exit or spawn descendants.
        let flags = CREATE_NO_WINDOW.0 | CREATE_SUSPENDED.0;
        let mut child = command.creation_flags(flags).spawn()?;
        #[cfg(test)]
        AFTER_SPAWN.with(|slot| {
            if let Some(hook) = slot.borrow_mut().take() {
                hook(&mut child, true);
            }
        });
        let attached = unsafe { AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())) }
            .map_err(io::Error::other)
            .and_then(|()| match mode {
                SpawnMode::Direct => resume_scoped_initial_thread(&child),
                SpawnMode::Suspended => resume_initial_thread(child.id()),
            });
        if let Err(error) = attached {
            tree.terminate();
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok((child, tree))
    }

    pub fn terminate(&self) {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{Foundation::HANDLE, System::JobObjects::TerminateJobObject};
        let _ = unsafe { TerminateJobObject(HANDLE(self.0.as_raw_handle()), 1) };
    }
}

#[cfg(target_os = "windows")]
fn resume_scoped_initial_thread(child: &Child) -> io::Result<()> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use windows::Win32::{
        Foundation::{ERROR_NO_MORE_ITEMS, HANDLE},
        System::{
            Diagnostics::ProcessSnapshotting::{
                PssCaptureSnapshot, PssFreeSnapshot, PssWalkMarkerCreate, PssWalkMarkerFree,
                PssWalkSnapshot, HPSS, HPSSWALK, PSS_CAPTURE_THREADS, PSS_THREAD_ENTRY,
                PSS_WALK_THREADS,
            },
            Threading::{GetCurrentProcess, OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };
    fn check(code: u32) -> io::Result<()> {
        if code == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(code as i32))
        }
    }
    struct Snapshot(HPSS);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            unsafe {
                PssFreeSnapshot(GetCurrentProcess(), self.0);
            }
        }
    }
    struct Marker(HPSSWALK);
    impl Drop for Marker {
        fn drop(&mut self) {
            unsafe {
                PssWalkMarkerFree(self.0);
            }
        }
    }
    fn next(snapshot: HPSS, marker: HPSSWALK, entry: &mut PSS_THREAD_ENTRY) -> u32 {
        // entry is properly aligned and alive for the complete native call.
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (entry as *mut PSS_THREAD_ENTRY).cast::<u8>(),
                std::mem::size_of::<PSS_THREAD_ENTRY>(),
            )
        };
        unsafe { PssWalkSnapshot(snapshot, PSS_WALK_THREADS, marker, Some(bytes)) }
    }
    let mut snapshot = HPSS::default();
    check(unsafe {
        PssCaptureSnapshot(
            HANDLE(child.as_raw_handle()),
            PSS_CAPTURE_THREADS,
            None,
            &mut snapshot,
        )
    })?;
    let snapshot = Snapshot(snapshot);
    let mut marker = HPSSWALK::default();
    check(unsafe { PssWalkMarkerCreate(None, &mut marker) })?;
    let marker = Marker(marker);
    let mut entry = PSS_THREAD_ENTRY::default();
    check(next(snapshot.0, marker.0, &mut entry))?;
    let tid = entry.ThreadId;
    // A freshly created suspended process has one initial thread. Fail closed
    // rather than resuming an arbitrary thread if external instrumentation changed it.
    let end = next(snapshot.0, marker.0, &mut entry);
    if end != ERROR_NO_MORE_ITEMS.0 {
        check(end)?;
        return Err(io::Error::other("suspended process has multiple threads"));
    }
    let thread =
        unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, tid) }.map_err(io::Error::other)?;
    let _thread = unsafe { OwnedHandle::from_raw_handle(thread.0) };
    if unsafe { ResumeThread(thread) } == u32::MAX {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::process::Stdio;

    #[test]
    fn fast_read_cannot_exit_before_job_assignment() {
        AFTER_SPAWN.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(|child, suspended| {
                if !suspended {
                    // Force the problematic scheduling order instead of relying on a sleep.
                    child.wait().unwrap();
                } else {
                    assert!(child.try_wait().unwrap().is_none());
                }
            }));
        });
        let mut command = Command::new("git");
        command
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (child, _tree) = ProcessTree::spawn_with_mode(&mut command, SpawnMode::Direct)
            .expect("a successful fast Git command must not become a spawn failure");
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).starts_with("git version"));
    }

    #[test]
    #[ignore = "manual Windows spawn latency measurement"]
    fn scoped_and_toolhelp_spawn_latency() {
        for (label, mode) in [
            ("process-scoped", SpawnMode::Direct),
            ("system-toolhelp", SpawnMode::Suspended),
        ] {
            let started = std::time::Instant::now();
            for _ in 0..20 {
                let mut command = Command::new("git");
                command
                    .arg("--version")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
                let (mut child, _tree) = ProcessTree::spawn_with_mode(&mut command, mode).unwrap();
                assert!(child.wait().unwrap().success());
            }
            println!(
                "{label}: {:.1} ms/spawn (20 iterations)",
                started.elapsed().as_secs_f64() * 50.0
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn resume_initial_thread(pid: u32) -> io::Result<()> {
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows::Win32::System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
    };
    let snapshot =
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }.map_err(io::Error::other)?;
    let _snapshot = unsafe { OwnedHandle::from_raw_handle(snapshot.0) };
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    unsafe { Thread32First(snapshot, &mut entry) }.map_err(io::Error::other)?;
    loop {
        if entry.th32OwnerProcessID == pid {
            let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) }
                .map_err(io::Error::other)?;
            let _thread = unsafe { OwnedHandle::from_raw_handle(thread.0) };
            if unsafe { ResumeThread(thread) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            return Ok(());
        }
        if unsafe { Thread32Next(snapshot, &mut entry) }.is_err() {
            break;
        }
    }
    Err(io::Error::other("suspended process has no initial thread"))
}

#[cfg(unix)]
pub fn prepare_pipe(pipe: &impl std::os::fd::AsRawFd) -> io::Result<()> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn prepare_pipe<T>(_pipe: &T) -> io::Result<()> {
    Ok(())
}

// Read at most one chunk per iteration so continuous output cannot starve the deadline.
#[cfg(unix)]
pub fn read_pipe(pipe: &mut impl Read, bytes: &mut Vec<u8>) -> io::Result<bool> {
    let mut chunk = [0; 65536];
    match pipe.read(&mut chunk) {
        Ok(0) => Ok(true),
        Ok(count) => {
            bytes.extend_from_slice(&chunk[..count]);
            Ok(false)
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
pub fn read_pipe(
    pipe: &mut (impl Read + std::os::windows::io::AsRawHandle),
    bytes: &mut Vec<u8>,
) -> io::Result<bool> {
    use windows::Win32::{
        Foundation::{ERROR_BROKEN_PIPE, HANDLE},
        System::Pipes::PeekNamedPipe,
    };
    let mut available = 0;
    if let Err(error) = unsafe {
        PeekNamedPipe(
            HANDLE(pipe.as_raw_handle()),
            None,
            0,
            None,
            Some(&mut available),
            None,
        )
    } {
        if error.code() == ERROR_BROKEN_PIPE.to_hresult() {
            return Ok(true);
        }
        return Err(io::Error::other(error));
    }
    if available == 0 {
        return Ok(false);
    }
    let mut chunk = [0; 65536];
    let count = pipe.read(&mut chunk[..(available as usize).min(65536)])?;
    bytes.extend_from_slice(&chunk[..count]);
    Ok(count == 0)
}
