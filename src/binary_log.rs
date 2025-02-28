use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::error::Result;

use serde::Serialize;

pub(crate) struct BinaryLog {
    // out: File,
}

impl BinaryLog {
    fn new() -> Self {
        // let out = File::create_new(std::env::var("MONGODB_RUST_BINARY_LOG").unwrap()).unwrap();
        Self { /*out*/ }
    }

    pub(crate) fn get() -> MutexGuard<'static, Self> {
        static GLOBAL: LazyLock<Mutex<BinaryLog>> = LazyLock::new(|| Mutex::new(BinaryLog::new()));
        GLOBAL.lock().unwrap()
    }

    pub(crate) fn write<T: Serialize>(&mut self, _label: &str, _value: &T) -> Result<()> {
        // let bytes = crate::bson::to_vec(value)?;
        // let label_bytes = label.as_bytes();
        // self.out.write_all(&label_bytes.len().to_le_bytes())?;
        // self.out.write_all(label_bytes)?;
        // self.out.write_all(&bytes)?;
        // self.out.flush()?;
        Ok(())
    }
}
