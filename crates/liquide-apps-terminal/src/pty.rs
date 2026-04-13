//! Pseudo-terminal abstraction.

use serde::{Deserialize, Serialize};

/// PTY connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PtyState {
    /// PTY not yet spawned.
    Idle,
    /// Shell is running.
    Running,
    /// Shell exited normally.
    Exited(i32),
    /// Shell was killed or crashed.
    Killed,
}

/// PTY size in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtySize {
    pub rows: u32,
    pub cols: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl PtySize {
    #[must_use]
    pub fn new(rows: u32, cols: u32) -> Self {
        Self { rows, cols, pixel_width: 0, pixel_height: 0 }
    }
}

impl Default for PtySize {
    fn default() -> Self { Self::new(24, 80) }
}

// ---------------------------------------------------------------------------
// Platform-specific PTY implementations
// ---------------------------------------------------------------------------

#[cfg(unix)]
mod unix_pty {
    use std::ffi::CString;

    unsafe extern "C" {
        fn posix_openpt(flags: libc_c_int) -> libc_c_int;
        fn grantpt(fd: libc_c_int) -> libc_c_int;
        fn unlockpt(fd: libc_c_int) -> libc_c_int;
        fn ptsname_r(fd: libc_c_int, buf: *mut libc_c_char, buflen: usize) -> libc_c_int;
        fn fork() -> libc_c_int;
        fn setsid() -> libc_c_int;
        fn dup2(old: libc_c_int, new: libc_c_int) -> libc_c_int;
        fn execvp(file: *const libc_c_char, argv: *const *const libc_c_char) -> libc_c_int;
        fn close(fd: libc_c_int) -> libc_c_int;
        fn read(fd: libc_c_int, buf: *mut u8, count: usize) -> isize;
        fn write(fd: libc_c_int, buf: *const u8, count: usize) -> isize;
        fn ioctl(fd: libc_c_int, request: libc_c_ulong, ...) -> libc_c_int;
        fn waitpid(pid: libc_c_int, status: *mut libc_c_int, options: libc_c_int) -> libc_c_int;
        fn kill(pid: libc_c_int, sig: libc_c_int) -> libc_c_int;
        fn fcntl(fd: libc_c_int, cmd: libc_c_int, ...) -> libc_c_int;
        fn open(path: *const libc_c_char, flags: libc_c_int) -> libc_c_int;
        fn setenv(
            name: *const libc_c_char,
            value: *const libc_c_char,
            overwrite: libc_c_int,
        ) -> libc_c_int;
        fn chdir(path: *const libc_c_char) -> libc_c_int;
    }

    // Type aliases so the extern block is self-contained (no libc crate).
    type libc_c_int = i32;
    type libc_c_char = i8;
    #[cfg(target_os = "linux")]
    type libc_c_ulong = std::ffi::c_ulong;
    #[cfg(target_os = "macos")]
    type libc_c_ulong = std::ffi::c_ulong;

    const O_RDWR: libc_c_int = 2;
    #[cfg(target_os = "linux")]
    const O_NOCTTY: libc_c_int = 256;
    #[cfg(target_os = "macos")]
    const O_NOCTTY: libc_c_int = 0x20000;

    const F_SETFL: libc_c_int = 4;
    #[cfg(target_os = "linux")]
    const O_NONBLOCK: libc_c_int = 2048;
    #[cfg(target_os = "macos")]
    const O_NONBLOCK: libc_c_int = 4;

    const WNOHANG: libc_c_int = 1;
    const SIGTERM: libc_c_int = 15;
    const SIGKILL: libc_c_int = 9;

    #[cfg(target_os = "linux")]
    const TIOCSWINSZ: libc_c_ulong = 0x5414;
    #[cfg(target_os = "macos")]
    const TIOCSWINSZ: libc_c_ulong = 0x80087467;

    #[repr(C)]
    struct Winsize {
        ws_row: u16,
        ws_col: u16,
        ws_xpixel: u16,
        ws_ypixel: u16,
    }

    pub struct UnixPty {
        master_fd: libc_c_int,
        child_pid: libc_c_int,
    }

    impl UnixPty {
        pub fn spawn(
            shell: &str,
            rows: u16,
            cols: u16,
            working_directory: Option<&str>,
            env_vars: &[(String, String)],
        ) -> Result<Self, String> {
            unsafe {
                let master = posix_openpt(O_RDWR | O_NOCTTY);
                if master < 0 {
                    return Err("posix_openpt failed".into());
                }
                if grantpt(master) != 0 {
                    close(master);
                    return Err("grantpt failed".into());
                }
                if unlockpt(master) != 0 {
                    close(master);
                    return Err("unlockpt failed".into());
                }

                let mut ptsname_buf = [0i8; 256];
                if ptsname_r(master, ptsname_buf.as_mut_ptr(), ptsname_buf.len()) != 0 {
                    close(master);
                    return Err("ptsname_r failed".into());
                }
                let slave_name = ptsname_buf.as_ptr();

                // Set initial window size on the master.
                let ws = Winsize {
                    ws_row: rows,
                    ws_col: cols,
                    ws_xpixel: 0,
                    ws_ypixel: 0,
                };
                ioctl(master, TIOCSWINSZ, &ws);

                let pid = fork();
                if pid < 0 {
                    close(master);
                    return Err("fork failed".into());
                }

                if pid == 0 {
                    // --- child process ---
                    close(master);
                    setsid();

                    let slave = open(slave_name, O_RDWR);
                    if slave < 0 {
                        std::process::exit(1);
                    }
                    dup2(slave, 0);
                    dup2(slave, 1);
                    dup2(slave, 2);
                    if slave > 2 {
                        close(slave);
                    }

                    // Set TERM.
                    let term_key = CString::new("TERM").unwrap();
                    let term_val = CString::new("xterm-256color").unwrap();
                    setenv(term_key.as_ptr(), term_val.as_ptr(), 1);

                    // User-supplied env vars.
                    for (k, v) in env_vars {
                        if let (Ok(ck), Ok(cv)) = (CString::new(k.as_str()), CString::new(v.as_str()))
                        {
                            setenv(ck.as_ptr(), cv.as_ptr(), 1);
                        }
                    }

                    // Working directory.
                    if let Some(dir) = working_directory {
                        if let Ok(cdir) = CString::new(dir) {
                            chdir(cdir.as_ptr());
                        }
                    }

                    // Exec shell.
                    let shell_c = CString::new(shell)
                        .unwrap_or_else(|_| CString::new("/bin/sh").unwrap());
                    let args: [*const libc_c_char; 2] = [shell_c.as_ptr(), std::ptr::null()];
                    execvp(shell_c.as_ptr(), args.as_ptr());
                    std::process::exit(127);
                }

                // --- parent process ---
                // Set the master fd to non-blocking so reads don't hang.
                fcntl(master, F_SETFL, O_NONBLOCK);

                Ok(Self {
                    master_fd: master,
                    child_pid: pid,
                })
            }
        }

        pub fn read(&self, buf: &mut [u8]) -> Result<usize, String> {
            let n = unsafe { read(self.master_fd, buf.as_mut_ptr(), buf.len()) };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(0);
                }
                return Err(format!("pty read: {err}"));
            }
            Ok(n as usize)
        }

        pub fn write(&self, data: &[u8]) -> Result<usize, String> {
            let n = unsafe { write(self.master_fd, data.as_ptr(), data.len()) };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::WouldBlock {
                    return Ok(0);
                }
                return Err(format!("pty write: {err}"));
            }
            Ok(n as usize)
        }

        pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
            let ws = Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            if unsafe { ioctl(self.master_fd, TIOCSWINSZ, &ws) } < 0 {
                return Err("TIOCSWINSZ failed".into());
            }
            Ok(())
        }

        pub fn is_alive(&self) -> bool {
            let mut status: libc_c_int = 0;
            let ret = unsafe { waitpid(self.child_pid, &mut status, WNOHANG) };
            ret == 0
        }

        pub fn try_wait(&self) -> Option<i32> {
            let mut status: libc_c_int = 0;
            let ret = unsafe { waitpid(self.child_pid, &mut status, WNOHANG) };
            if ret > 0 {
                // WIFEXITED: (status & 0x7f) == 0 → WEXITSTATUS: (status >> 8) & 0xff
                if (status & 0x7f) == 0 {
                    Some((status >> 8) & 0xff)
                } else {
                    // Killed by signal.
                    Some(-(status & 0x7f))
                }
            } else {
                None
            }
        }

        pub fn kill_process(&self) {
            unsafe {
                kill(self.child_pid, SIGTERM);
            }
            // Give it a short window to exit gracefully.
            std::thread::sleep(std::time::Duration::from_millis(50));
            if self.is_alive() {
                unsafe {
                    kill(self.child_pid, SIGKILL);
                }
            }
        }
    }

    impl Drop for UnixPty {
        fn drop(&mut self) {
            unsafe {
                close(self.master_fd);
            }
            if self.is_alive() {
                self.kill_process();
            }
        }
    }
}

#[cfg(windows)]
mod windows_pty {
    use std::ffi::c_void;

    type HANDLE = *mut c_void;
    type HPCON = *mut c_void;
    type DWORD = u32;
    type BOOL = i32;
    type HRESULT = i32;
    type WCHAR = u16;

    const EXTENDED_STARTUPINFO_PRESENT: DWORD = 0x00080000;
    const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 0x00020016;
    const WAIT_OBJECT_0: DWORD = 0;
    const WAIT_TIMEOUT: DWORD = 258;
    const S_OK: HRESULT = 0;

    #[repr(C)]
    struct COORD {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SECURITY_ATTRIBUTES {
        n_length: DWORD,
        lp_security_descriptor: *mut c_void,
        b_inherit_handle: BOOL,
    }

    #[repr(C)]
    struct STARTUPINFOEXW {
        startup_info: STARTUPINFOW,
        lp_attribute_list: *mut c_void,
    }

    #[repr(C)]
    struct STARTUPINFOW {
        cb: DWORD,
        reserved: *mut WCHAR,
        desktop: *mut WCHAR,
        title: *mut WCHAR,
        x: DWORD,
        y: DWORD,
        x_size: DWORD,
        y_size: DWORD,
        x_count_chars: DWORD,
        y_count_chars: DWORD,
        fill_attribute: DWORD,
        flags: DWORD,
        show_window: u16,
        cb_reserved2: u16,
        lp_reserved2: *mut u8,
        std_input: HANDLE,
        std_output: HANDLE,
        std_error: HANDLE,
    }

    #[repr(C)]
    struct PROCESS_INFORMATION {
        process: HANDLE,
        thread: HANDLE,
        process_id: DWORD,
        thread_id: DWORD,
    }

    unsafe extern "system" {
        fn CreatePipe(
            read: *mut HANDLE,
            write: *mut HANDLE,
            sa: *const SECURITY_ATTRIBUTES,
            size: DWORD,
        ) -> BOOL;

        fn CreatePseudoConsole(
            size: COORD,
            input: HANDLE,
            output: HANDLE,
            flags: DWORD,
            phpc: *mut HPCON,
        ) -> HRESULT;

        fn ResizePseudoConsole(hpc: HPCON, size: COORD) -> HRESULT;

        fn ClosePseudoConsole(hpc: HPCON);

        fn CloseHandle(handle: HANDLE) -> BOOL;

        fn ReadFile(
            file: HANDLE,
            buf: *mut u8,
            len: DWORD,
            read: *mut DWORD,
            overlapped: *mut c_void,
        ) -> BOOL;

        fn WriteFile(
            file: HANDLE,
            buf: *const u8,
            len: DWORD,
            written: *mut DWORD,
            overlapped: *mut c_void,
        ) -> BOOL;

        fn InitializeProcThreadAttributeList(
            list: *mut c_void,
            count: DWORD,
            flags: DWORD,
            size: *mut usize,
        ) -> BOOL;

        fn UpdateProcThreadAttribute(
            list: *mut c_void,
            flags: DWORD,
            attribute: usize,
            value: *mut c_void,
            size: usize,
            prev_value: *mut c_void,
            return_size: *mut usize,
        ) -> BOOL;

        fn DeleteProcThreadAttributeList(list: *mut c_void);

        fn CreateProcessW(
            app_name: *const WCHAR,
            cmd_line: *mut WCHAR,
            proc_attrs: *const c_void,
            thread_attrs: *const c_void,
            inherit_handles: BOOL,
            creation_flags: DWORD,
            environment: *mut c_void,
            current_dir: *const WCHAR,
            startup_info: *mut STARTUPINFOEXW,
            proc_info: *mut PROCESS_INFORMATION,
        ) -> BOOL;

        fn WaitForSingleObject(handle: HANDLE, millis: DWORD) -> DWORD;

        fn GetExitCodeProcess(handle: HANDLE, code: *mut DWORD) -> BOOL;

        fn TerminateProcess(handle: HANDLE, exit_code: u32) -> BOOL;

        fn PeekNamedPipe(
            pipe: HANDLE,
            buffer: *mut c_void,
            buf_size: DWORD,
            bytes_read: *mut DWORD,
            total_avail: *mut DWORD,
            bytes_left: *mut DWORD,
        ) -> BOOL;

        fn GetLastError() -> DWORD;
    }

    /// Encode a Rust &str as a null-terminated wide string.
    fn to_wide(s: &str) -> Vec<WCHAR> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct WindowsPty {
        hpc: HPCON,
        /// Read end of the pipe connected to ConPTY output.
        pipe_out_read: HANDLE,
        /// Write end of the pipe connected to ConPTY input.
        pipe_in_write: HANDLE,
        /// Handle to the spawned process.
        process_handle: HANDLE,
        /// Handle to the spawned thread (kept for cleanup).
        thread_handle: HANDLE,
        /// Attribute list allocation (must stay alive until process is created).
        #[allow(dead_code)]
        attr_list_buf: Vec<u8>,
    }

    // HANDLE is Send-safe — they're opaque kernel handles.
    unsafe impl Send for WindowsPty {}

    impl WindowsPty {
        pub fn spawn(
            shell: &str,
            rows: u16,
            cols: u16,
            working_directory: Option<&str>,
            env_vars: &[(String, String)],
        ) -> Result<Self, String> {
            unsafe {
                // Create two pipe pairs. ConPTY reads from pipe_in and writes to pipe_out.
                let mut pipe_in_read: HANDLE = std::ptr::null_mut();
                let mut pipe_in_write: HANDLE = std::ptr::null_mut();
                let mut pipe_out_read: HANDLE = std::ptr::null_mut();
                let mut pipe_out_write: HANDLE = std::ptr::null_mut();

                let sa = SECURITY_ATTRIBUTES {
                    n_length: std::mem::size_of::<SECURITY_ATTRIBUTES>() as DWORD,
                    lp_security_descriptor: std::ptr::null_mut(),
                    b_inherit_handle: 1,
                };

                if CreatePipe(&mut pipe_in_read, &mut pipe_in_write, &sa, 0) == 0 {
                    return Err(format!(
                        "CreatePipe (input) failed: {}",
                        GetLastError()
                    ));
                }
                if CreatePipe(&mut pipe_out_read, &mut pipe_out_write, &sa, 0) == 0 {
                    CloseHandle(pipe_in_read);
                    CloseHandle(pipe_in_write);
                    return Err(format!(
                        "CreatePipe (output) failed: {}",
                        GetLastError()
                    ));
                }

                // Create the pseudo console.
                let size = COORD {
                    x: cols as i16,
                    y: rows as i16,
                };
                let mut hpc: HPCON = std::ptr::null_mut();
                let hr = CreatePseudoConsole(size, pipe_in_read, pipe_out_write, 0, &mut hpc);
                if hr != S_OK {
                    CloseHandle(pipe_in_read);
                    CloseHandle(pipe_in_write);
                    CloseHandle(pipe_out_read);
                    CloseHandle(pipe_out_write);
                    return Err(format!("CreatePseudoConsole failed: 0x{hr:08x}"));
                }

                // The ConPTY now owns the read-end of input and write-end of output.
                // Close our copies so the pipe breaks when the child exits.
                CloseHandle(pipe_in_read);
                CloseHandle(pipe_out_write);

                // Build a proc-thread attribute list containing the pseudo console.
                let mut attr_size: usize = 0;
                InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attr_size);
                let mut attr_list_buf = vec![0u8; attr_size];
                let attr_list = attr_list_buf.as_mut_ptr() as *mut c_void;
                if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut attr_size) == 0 {
                    ClosePseudoConsole(hpc);
                    CloseHandle(pipe_out_read);
                    CloseHandle(pipe_in_write);
                    return Err(format!(
                        "InitializeProcThreadAttributeList failed: {}",
                        GetLastError()
                    ));
                }
                if UpdateProcThreadAttribute(
                    attr_list,
                    0,
                    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
                    hpc,
                    std::mem::size_of::<HPCON>(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                ) == 0
                {
                    DeleteProcThreadAttributeList(attr_list);
                    ClosePseudoConsole(hpc);
                    CloseHandle(pipe_out_read);
                    CloseHandle(pipe_in_write);
                    return Err(format!(
                        "UpdateProcThreadAttribute failed: {}",
                        GetLastError()
                    ));
                }

                // Prepare STARTUPINFOEXW.
                let mut si: STARTUPINFOEXW = std::mem::zeroed();
                si.startup_info.cb = std::mem::size_of::<STARTUPINFOEXW>() as DWORD;
                si.lp_attribute_list = attr_list;

                // Build environment block if needed.
                let env_block: Option<Vec<u16>> = if !env_vars.is_empty() {
                    // A Unicode environment block is a sequence of null-terminated
                    // KEY=VALUE strings, terminated by an additional null.
                    let mut block = Vec::new();
                    for (k, v) in env_vars {
                        let entry = format!("{k}={v}");
                        block.extend(entry.encode_utf16());
                        block.push(0);
                    }
                    block.push(0);
                    Some(block)
                } else {
                    None
                };

                let mut cmd_wide = to_wide(shell);
                let cwd_wide = working_directory.map(|d| to_wide(d));

                let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

                let env_ptr = env_block
                    .as_ref()
                    .map(|b| b.as_ptr() as *mut c_void)
                    .unwrap_or(std::ptr::null_mut());
                let cwd_ptr = cwd_wide
                    .as_ref()
                    .map(|w| w.as_ptr())
                    .unwrap_or(std::ptr::null());

                let ok = CreateProcessW(
                    std::ptr::null(),
                    cmd_wide.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0, // don't inherit handles
                    EXTENDED_STARTUPINFO_PRESENT,
                    env_ptr,
                    cwd_ptr,
                    &mut si,
                    &mut pi,
                );

                DeleteProcThreadAttributeList(attr_list);

                if ok == 0 {
                    ClosePseudoConsole(hpc);
                    CloseHandle(pipe_out_read);
                    CloseHandle(pipe_in_write);
                    return Err(format!("CreateProcessW failed: {}", GetLastError()));
                }

                Ok(Self {
                    hpc,
                    pipe_out_read,
                    pipe_in_write,
                    process_handle: pi.process,
                    thread_handle: pi.thread,
                    attr_list_buf,
                })
            }
        }

        pub fn read(&self, buf: &mut [u8]) -> Result<usize, String> {
            unsafe {
                // Check how many bytes are available so we don't block.
                let mut avail: DWORD = 0;
                if PeekNamedPipe(
                    self.pipe_out_read,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut avail,
                    std::ptr::null_mut(),
                ) == 0
                {
                    return Err(format!("PeekNamedPipe failed: {}", GetLastError()));
                }
                if avail == 0 {
                    return Ok(0);
                }
                let to_read = (buf.len() as DWORD).min(avail);
                let mut bytes_read: DWORD = 0;
                if ReadFile(
                    self.pipe_out_read,
                    buf.as_mut_ptr(),
                    to_read,
                    &mut bytes_read,
                    std::ptr::null_mut(),
                ) == 0
                {
                    return Err(format!("ReadFile failed: {}", GetLastError()));
                }
                Ok(bytes_read as usize)
            }
        }

        pub fn write(&self, data: &[u8]) -> Result<usize, String> {
            unsafe {
                let mut written: DWORD = 0;
                if WriteFile(
                    self.pipe_in_write,
                    data.as_ptr(),
                    data.len() as DWORD,
                    &mut written,
                    std::ptr::null_mut(),
                ) == 0
                {
                    return Err(format!("WriteFile failed: {}", GetLastError()));
                }
                Ok(written as usize)
            }
        }

        pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
            let size = COORD {
                x: cols as i16,
                y: rows as i16,
            };
            let hr = unsafe { ResizePseudoConsole(self.hpc, size) };
            if hr != S_OK {
                return Err(format!("ResizePseudoConsole failed: 0x{hr:08x}"));
            }
            Ok(())
        }

        pub fn is_alive(&self) -> bool {
            unsafe { WaitForSingleObject(self.process_handle, 0) == WAIT_TIMEOUT }
        }

        pub fn try_wait(&self) -> Option<i32> {
            unsafe {
                if WaitForSingleObject(self.process_handle, 0) == WAIT_OBJECT_0 {
                    let mut code: DWORD = 0;
                    GetExitCodeProcess(self.process_handle, &mut code);
                    Some(code as i32)
                } else {
                    None
                }
            }
        }

        pub fn kill_process(&self) {
            unsafe {
                TerminateProcess(self.process_handle, 1);
            }
        }
    }

    impl Drop for WindowsPty {
        fn drop(&mut self) {
            unsafe {
                // Close the pseudo console first — this signals the child.
                ClosePseudoConsole(self.hpc);
                // Give the process a moment to exit.
                WaitForSingleObject(self.process_handle, 500);
                if self.is_alive() {
                    TerminateProcess(self.process_handle, 1);
                }
                CloseHandle(self.process_handle);
                CloseHandle(self.thread_handle);
                CloseHandle(self.pipe_out_read);
                CloseHandle(self.pipe_in_write);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-platform PlatformPty wrapper
// ---------------------------------------------------------------------------

/// Opaque platform PTY handle.
enum PlatformPty {
    #[cfg(unix)]
    Unix(unix_pty::UnixPty),
    #[cfg(windows)]
    Windows(windows_pty::WindowsPty),
}

impl PlatformPty {
    fn spawn(
        shell: &str,
        rows: u16,
        cols: u16,
        working_directory: Option<&str>,
        env_vars: &[(String, String)],
    ) -> Result<Self, String> {
        #[cfg(unix)]
        {
            unix_pty::UnixPty::spawn(shell, rows, cols, working_directory, env_vars)
                .map(PlatformPty::Unix)
        }
        #[cfg(windows)]
        {
            windows_pty::WindowsPty::spawn(shell, rows, cols, working_directory, env_vars)
                .map(PlatformPty::Windows)
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, String> {
        match self {
            #[cfg(unix)]
            PlatformPty::Unix(p) => p.read(buf),
            #[cfg(windows)]
            PlatformPty::Windows(p) => p.read(buf),
        }
    }

    fn write(&self, data: &[u8]) -> Result<usize, String> {
        match self {
            #[cfg(unix)]
            PlatformPty::Unix(p) => p.write(data),
            #[cfg(windows)]
            PlatformPty::Windows(p) => p.write(data),
        }
    }

    fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        match self {
            #[cfg(unix)]
            PlatformPty::Unix(p) => p.resize(rows, cols),
            #[cfg(windows)]
            PlatformPty::Windows(p) => p.resize(rows, cols),
        }
    }

    fn try_wait(&self) -> Option<i32> {
        match self {
            #[cfg(unix)]
            PlatformPty::Unix(p) => p.try_wait(),
            #[cfg(windows)]
            PlatformPty::Windows(p) => p.try_wait(),
        }
    }

    fn kill_process(&self) {
        match self {
            #[cfg(unix)]
            PlatformPty::Unix(p) => p.kill_process(),
            #[cfg(windows)]
            PlatformPty::Windows(p) => p.kill_process(),
        }
    }
}

// ---------------------------------------------------------------------------
// Public PtyBackend (preserves existing API)
// ---------------------------------------------------------------------------

/// PTY backend managing a shell process.
pub struct PtyBackend {
    shell: String,
    working_directory: Option<String>,
    state: PtyState,
    size: PtySize,
    output_buffer: Vec<u8>,
    env_vars: Vec<(String, String)>,
    /// The live platform PTY handle, present only while `state == Running`.
    platform: Option<PlatformPty>,
}

impl PtyBackend {
    /// Create a new PTY backend with the given shell.
    #[must_use]
    pub fn new(shell: String, size: PtySize) -> Self {
        Self {
            shell,
            working_directory: None,
            state: PtyState::Idle,
            size,
            output_buffer: Vec::new(),
            env_vars: Vec::new(),
            platform: None,
        }
    }

    /// Set the initial working directory.
    pub fn set_working_directory(&mut self, dir: String) {
        self.working_directory = Some(dir);
    }

    /// Add an environment variable.
    pub fn set_env(&mut self, key: String, value: String) {
        self.env_vars.push((key, value));
    }

    /// Resolve the shell command to use.
    fn resolve_shell(&self) -> String {
        if !self.shell.is_empty() {
            return self.shell.clone();
        }
        #[cfg(unix)]
        {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
        }
        #[cfg(windows)]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string())
        }
    }

    /// Spawn the shell process.
    pub fn spawn(&mut self) -> crate::Result<()> {
        if self.state == PtyState::Running {
            return Ok(());
        }

        let shell = self.resolve_shell();
        let rows = self.size.rows.min(u16::MAX as u32) as u16;
        let cols = self.size.cols.min(u16::MAX as u32) as u16;

        let pty = PlatformPty::spawn(
            &shell,
            rows,
            cols,
            self.working_directory.as_deref(),
            &self.env_vars,
        )
        .map_err(|reason| crate::TerminalError::PtySpawnFailed { reason })?;

        self.platform = Some(pty);
        self.state = PtyState::Running;
        Ok(())
    }

    /// Write input bytes to the PTY (keyboard input).
    pub fn write(&mut self, data: &[u8]) -> crate::Result<()> {
        if self.state != PtyState::Running {
            return Err(crate::TerminalError::PtySpawnFailed {
                reason: "PTY not running".into(),
            });
        }
        if let Some(ref pty) = self.platform {
            pty.write(data).map_err(|reason| crate::TerminalError::PtySpawnFailed { reason })?;
        } else {
            // Fallback echo for testing when no platform PTY is available.
            self.output_buffer.extend_from_slice(data);
        }
        Ok(())
    }

    /// Read available output bytes from the PTY.
    #[must_use]
    pub fn read(&mut self) -> Vec<u8> {
        if let Some(ref pty) = self.platform {
            let mut buf = vec![0u8; 8192];
            match pty.read(&mut buf) {
                Ok(0) => {}
                Ok(n) => {
                    buf.truncate(n);
                    return buf;
                }
                Err(_) => {}
            }
            Vec::new()
        } else {
            std::mem::take(&mut self.output_buffer)
        }
    }

    /// Resize the PTY.
    pub fn resize(&mut self, size: PtySize) {
        self.size = size;
        if let Some(ref pty) = self.platform {
            let rows = size.rows.min(u16::MAX as u32) as u16;
            let cols = size.cols.min(u16::MAX as u32) as u16;
            let _ = pty.resize(rows, cols);
        }
    }

    /// Get current state, polling the child process if running.
    #[must_use]
    pub fn state(&self) -> PtyState { self.state }

    /// Poll the child process and update state if it has exited.
    pub fn poll(&mut self) {
        if self.state != PtyState::Running {
            return;
        }
        if let Some(ref pty) = self.platform {
            if let Some(code) = pty.try_wait() {
                if code >= 0 {
                    self.state = PtyState::Exited(code);
                } else {
                    self.state = PtyState::Killed;
                }
            }
        }
    }

    /// Get current size.
    #[must_use]
    pub fn size(&self) -> PtySize { self.size }

    /// Get the shell command.
    #[must_use]
    pub fn shell(&self) -> &str { &self.shell }

    /// Signal the shell to exit.
    pub fn kill(&mut self) {
        if let Some(ref pty) = self.platform {
            pty.kill_process();
        }
        self.state = PtyState::Killed;
        self.platform = None;
    }

    /// Mark as exited with a code.
    pub fn mark_exited(&mut self, code: i32) {
        self.state = PtyState::Exited(code);
        self.platform = None;
    }
}
