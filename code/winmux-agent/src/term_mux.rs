//! Guest-side terminal multiplexer over a single virtio-serial port.
//!
//! One login shell per logical channel (= one WinMux tab). Bypasses
//! SSH + SLIRP entirely: the host writes framed bytes straight into the
//! virtio ring, we run a PTY-backed `bash` and pipe both ways. This is the
//! low-latency replacement for the ssh.exe transport.
//!
//! Frame format lives in `winmux_shared::termmux`.

use std::collections::HashMap;
use std::ffi::CString;
use std::io::Read;
use std::os::unix::io::RawFd;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use winmux_shared::termmux::{Frame, KIND_CLOSE, KIND_DATA, KIND_OPEN, KIND_RESIZE};

/// virtio-serial named port (preferred) or raw vportXpY fallbacks.
const PORT_CANDIDATES: &[&str] = &[
    "/dev/virtio-ports/winmux.term",
    "/dev/vport0p1",
    "/dev/vport1p1",
    "/dev/vport2p1",
];

struct Channel {
    master: RawFd,
    pid: libc::pid_t,
}

/// Shared read+write handle to the port. Writes are serialized so frames from
/// different channel threads never interleave on the wire.
#[derive(Clone)]
struct Port {
    fd: RawFd,
    wlock: Arc<Mutex<()>>,
}

impl Port {
    fn send(&self, frame: &Frame) {
        let buf = frame.encode();
        let _g = self.wlock.lock().unwrap();
        write_all_fd(self.fd, &buf);
    }
}

struct PortReader {
    fd: RawFd,
}
impl Read for PortReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(n as usize)
    }
}

fn write_all_fd(fd: RawFd, buf: &[u8]) {
    let mut off = 0;
    while off < buf.len() {
        let n = unsafe {
            libc::write(fd, buf[off..].as_ptr() as *const libc::c_void, buf.len() - off)
        };
        if n <= 0 {
            break;
        }
        off += n as usize;
    }
}

/// Entry point — spawn from the agent on a background thread.
pub fn run() {
    // Children inherit TERM (set once; safe — children only execv after fork).
    std::env::set_var("TERM", "xterm-256color");

    let port_path = loop {
        if let Some(p) = PORT_CANDIDATES.iter().find(|p| Path::new(p).exists()) {
            break p.to_string();
        }
        thread::sleep(Duration::from_millis(300));
    };
    eprintln!("[term-mux] using {port_path}");

    loop {
        match serve(&port_path) {
            Ok(()) => eprintln!("[term-mux] port closed; reopening"),
            Err(e) => eprintln!("[term-mux] error: {e}; reopening"),
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn serve(port_path: &str) -> std::io::Result<()> {
    let cpath = CString::new(port_path).unwrap();
    let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDWR) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let port = Port {
        fd,
        wlock: Arc::new(Mutex::new(())),
    };
    let channels: Arc<Mutex<HashMap<u32, Channel>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut reader = PortReader { fd };

    let result = loop {
        let frame = match Frame::read_from(&mut reader) {
            Ok(f) => f,
            Err(e) => break Err(e),
        };
        match frame.kind {
            KIND_OPEN => {
                let (cols, rows) = frame.size().unwrap_or((80, 24));
                // Replace any stale channel with the same id.
                close_channel(frame.channel, &channels);
                open_channel(frame.channel, cols, rows, port.clone(), channels.clone());
            }
            KIND_DATA => {
                if let Some(ch) = channels.lock().unwrap().get(&frame.channel) {
                    write_all_fd(ch.master, &frame.payload);
                }
            }
            KIND_RESIZE => {
                if let Some((cols, rows)) = frame.size() {
                    if let Some(ch) = channels.lock().unwrap().get(&frame.channel) {
                        set_winsize(ch.master, cols, rows);
                    }
                }
            }
            KIND_CLOSE => close_channel(frame.channel, &channels),
            _ => {}
        }
    };

    // Host disconnected / error: tear down every shell.
    let mut chans = channels.lock().unwrap();
    for (_, ch) in chans.drain() {
        unsafe {
            libc::kill(ch.pid, libc::SIGHUP);
            libc::close(ch.master);
        }
    }
    unsafe { libc::close(fd) };
    result
}

fn close_channel(channel: u32, channels: &Arc<Mutex<HashMap<u32, Channel>>>) {
    if let Some(ch) = channels.lock().unwrap().remove(&channel) {
        unsafe {
            libc::kill(ch.pid, libc::SIGHUP);
            libc::close(ch.master);
            let mut st = 0;
            libc::waitpid(ch.pid, &mut st, 0);
        }
    }
}

fn set_winsize(master: RawFd, cols: u16, rows: u16) {
    let ws = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        libc::ioctl(master, libc::TIOCSWINSZ, &ws);
    }
}

fn open_channel(
    channel: u32,
    cols: u16,
    rows: u16,
    port: Port,
    channels: Arc<Mutex<HashMap<u32, Channel>>>,
) {
    // Build exec args BEFORE fork — malloc after fork in a threaded process is
    // unsafe. Child path then only calls execv/_exit (async-signal-safe).
    let prog = CString::new("/bin/su").unwrap();
    let a0 = CString::new("su").unwrap();
    let a1 = CString::new("-").unwrap();
    let a2 = CString::new("winmux").unwrap();
    let argv = [a0.as_ptr(), a1.as_ptr(), a2.as_ptr(), std::ptr::null()];

    let mut master: libc::c_int = 0;
    let ws = libc::winsize {
        ws_row: rows,
        ws_col: cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    let pid = unsafe {
        libc::forkpty(
            &mut master,
            std::ptr::null_mut(),
            std::ptr::null(),
            &ws as *const libc::winsize,
        )
    };

    if pid < 0 {
        port.send(&Frame::close(channel));
        return;
    }
    if pid == 0 {
        // CHILD: forkpty already wired stdio to the slave pty + set controlling tty.
        unsafe {
            libc::execv(prog.as_ptr(), argv.as_ptr());
            libc::_exit(127);
        }
    }

    // PARENT
    channels
        .lock()
        .unwrap()
        .insert(channel, Channel { master, pid });

    // Pump shell output → DATA frames until the shell exits.
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let n = unsafe { libc::read(master, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n <= 0 {
                break;
            }
            port.send(&Frame::data(channel, buf[..n as usize].to_vec()));
        }
        // Shell gone: tell the host and reap.
        port.send(&Frame::close(channel));
        let pid_opt = channels.lock().unwrap().remove(&channel).map(|c| c.pid);
        if let Some(p) = pid_opt {
            unsafe {
                let mut st = 0;
                libc::waitpid(p, &mut st, 0);
                libc::close(master);
            }
        }
    });
}
