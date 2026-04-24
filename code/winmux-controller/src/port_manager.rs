//! Менеджер прокинутих портів.
//! Тримає мапінг guest_port → host_port (за замовчуванням 1:1).
//! Викликає QMP hostfwd_add / hostfwd_remove.

use crate::qmp::CommandHandle;
use anyhow::Result;
use std::collections::HashSet;
use std::sync::Mutex;

pub struct PortManager {
    qmp: CommandHandle,
    forwarded: Mutex<HashSet<u16>>,
}

impl PortManager {
    pub fn new(qmp: CommandHandle) -> Self {
        Self {
            qmp,
            forwarded: Mutex::new(HashSet::new()),
        }
    }

    /// Прокинути порт guest_port → host:guest_port (1:1)
    pub fn add(&self, port: u16) -> Result<()> {
        let mut set = self.forwarded.lock().unwrap();
        if set.contains(&port) {
            return Ok(()); // вже прокинутий
        }
        self.qmp.hostfwd_add(port, port)?;
        set.insert(port);
        crate::log_info(&format!("hostfwd_add tcp:127.0.0.1:{port}-:{port} OK"));
        Ok(())
    }

    pub fn remove(&self, port: u16) -> Result<()> {
        let mut set = self.forwarded.lock().unwrap();
        if !set.remove(&port) {
            return Ok(()); // не було
        }
        self.qmp.hostfwd_remove(port, port)?;
        crate::log_info(&format!("hostfwd_remove tcp:127.0.0.1:{port}-:{port} OK"));
        Ok(())
    }
}
