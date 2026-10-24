//! 子进程树处理：deadline 同时覆盖进程退出和管道读取。
//! 整理自 src-tauri/src/git/process.rs（未提交参考实现），行为保持一致：
//! Windows 用 Job Object 保证子孙进程一起终止、CREATE_NO_WINDOW 防闪控制台；
//! Unix 用进程组；管道按块读取，连续输出不会饿死 deadline。

use std::io::{self, Read};
use std::process::{Child, Command};

/// 子进程生成模式（N14 排障结论，见 process.rs Windows 分支注释）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnMode {
    /// 挂起入 Job：网络操作安全路径（Windows 上每次生成一次全系统线程快照）。
    Suspended,
    /// 直接生成入 Job：读操作低开销路径。
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
        let tree = Self(unsafe { OwnedHandle::from_raw_handle(job.0) });
        let suspended = matches!(mode, SpawnMode::Suspended);
        let flags = if suspended {
            CREATE_NO_WINDOW.0 | CREATE_SUSPENDED.0
        } else {
            CREATE_NO_WINDOW.0
        };
        let mut child = command.creation_flags(flags).spawn()?;
        let attached = unsafe { AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())) }
            .map_err(io::Error::other)
            .and_then(|()| {
                if suspended {
                    resume_initial_thread(child.id())
                } else {
                    Ok(())
                }
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
