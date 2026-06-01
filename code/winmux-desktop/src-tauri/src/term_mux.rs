//! Host-side client for the virtio-serial terminal mux.
//!
//! A single TCP connection to QEMU's chardev socket carries every tab as a
//! framed channel. This is the low-latency path that replaces ssh.exe: no
//! crypto, no SLIRP. If the guest daemon isn't answering we fall back to SSH
//! (see `terminal.rs`), decided once via `probe()`.

use std::collections::HashMap;
use std::io::Write;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use winmux_shared::termmux::{Frame, KIND_CLOSE, KIND_DATA};

type DataCb = Arc<dyn Fn(String) + Send + Sync>;

struct Channel {
    on_data: DataCb,
    on_close: Box<dyn Fn() + Send + Sync>,
}

pub struct MuxClient {
    writer: Arc<Mutex<TcpStream>>,
    channels: Arc<Mutex<HashMap<u32, Channel>>>,
}

impl MuxClient {
    /// Connect to the chardev socket and start the demux reader thread.
    pub fn connect(port: u16) -> std::io::Result<Arc<MuxClient>> {
        let stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.set_nodelay(true).ok();
        let reader = stream.try_clone()?;
        let channels: Arc<Mutex<HashMap<u32, Channel>>> = Arc::new(Mutex::new(HashMap::new()));
        let client = Arc::new(MuxClient {
            writer: Arc::new(Mutex::new(stream)),
            channels: channels.clone(),
        });

        // Demux reader: dispatch DATA/CLOSE frames to the right channel.
        let mut reader = reader;
        std::thread::spawn(move || loop {
            match Frame::read_from(&mut reader) {
                Ok(frame) => match frame.kind {
                    KIND_DATA => {
                        let cb = channels.lock().unwrap().get(&frame.channel).map(|c| c.on_data.clone());
                        if let Some(cb) = cb {
                            cb(String::from_utf8_lossy(&frame.payload).to_string());
                        }
                    }
                    KIND_CLOSE => {
                        if let Some(ch) = channels.lock().unwrap().remove(&frame.channel) {
                            (ch.on_close)();
                        }
                    }
                    _ => {}
                },
                Err(_) => break, // socket closed
            }
        });
        Ok(client)
    }

    fn send(&self, frame: &Frame) {
        if let Ok(mut w) = self.writer.lock() {
            let _ = w.write_all(&frame.encode());
            let _ = w.flush();
        }
    }

    pub fn open_channel<F, C>(&self, channel: u32, cols: u16, rows: u16, on_data: F, on_close: C)
    where
        F: Fn(String) + Send + Sync + 'static,
        C: Fn() + Send + Sync + 'static,
    {
        self.channels.lock().unwrap().insert(
            channel,
            Channel { on_data: Arc::new(on_data), on_close: Box::new(on_close) },
        );
        self.send(&Frame::open(channel, cols, rows));
    }

    pub fn write(&self, channel: u32, data: &str) {
        self.send(&Frame::data(channel, data.as_bytes().to_vec()));
    }

    pub fn resize(&self, channel: u32, cols: u16, rows: u16) {
        self.send(&Frame::resize(channel, cols, rows));
    }

    pub fn close_channel(&self, channel: u32) {
        self.channels.lock().unwrap().remove(&channel);
        self.send(&Frame::close(channel));
    }
}

/// One-time health check: open a throwaway channel and see if the guest daemon
/// answers with any output within `timeout`. Used to decide mux-vs-SSH once.
pub fn probe(port: u16, timeout: Duration) -> Option<Arc<MuxClient>> {
    let client = MuxClient::connect(port).ok()?;
    let seen = Arc::new(AtomicBool::new(false));
    let seen2 = seen.clone();
    // probe channel id 0 (real tabs start at 1)
    client.open_channel(
        0,
        80,
        24,
        move |_| seen2.store(true, Ordering::SeqCst),
        || {},
    );
    let start = Instant::now();
    while start.elapsed() < timeout {
        if seen.load(Ordering::SeqCst) {
            client.close_channel(0);
            return Some(client);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    client.close_channel(0);
    None
}
