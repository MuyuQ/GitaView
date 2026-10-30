//! FFI 句柄：不透明引擎包装 + 在途调用计数。
//!
//! §9 规则：request_stop / wait_stopped / destroy 等生命周期控制函数不计入
//! 它们正在等待清零的业务调用计数（避免 wait_stopped 等待自己）；destroy 不与
//! 任何其他调用并发，有在途调用或引擎未 Stopped 时返回 BUSY 且不销毁。

use gitaview_core::engine::Engine;
use std::sync::Mutex;

#[derive(Default)]
struct CallState {
    active: usize,
    destroying: bool,
}

/// 不透明引擎句柄：宿主只见 `*mut GvEngine`。
pub struct GvEngine {
    engine: Engine,
    calls: Mutex<CallState>,
}

/// 在途调用守卫：Drop 时递减计数。生命周期控制函数不创建此守卫。
pub struct CallGuard<'a> {
    calls: &'a Mutex<CallState>,
}

impl Drop for CallGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.calls.lock() {
            state.active = state.active.saturating_sub(1);
        }
    }
}

pub enum AcquireError {
    /// 正在销毁：不再接受新调用。
    Destroying,
}

impl GvEngine {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            calls: Mutex::new(CallState::default()),
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// 进入一次业务调用：销毁中拒绝；否则计数 +1。
    pub fn acquire_call(&self) -> Result<CallGuard<'_>, AcquireError> {
        let mut state = self.calls.lock().map_err(|_| AcquireError::Destroying)?;
        if state.destroying {
            return Err(AcquireError::Destroying);
        }
        state.active += 1;
        Ok(CallGuard { calls: &self.calls })
    }

    /// 开始销毁：仅当没有在途调用时置 destroying 并成功；引擎必须已 Stopped
    /// （由调用方检查生命周期，此处只保证调用互斥）。
    pub fn begin_destroy(&self) -> Result<(), DestroyBusy> {
        let mut state = self.calls.lock().map_err(|_| DestroyBusy)?;
        if state.active > 0 || state.destroying {
            return Err(DestroyBusy);
        }
        state.destroying = true;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestroyBusy;
