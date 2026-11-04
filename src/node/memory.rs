//! Reports the native memory a JS object owns to V8, so the garbage collector
//! sees canvases and images as the pressure they are.
//!
//! A `Canvas` or `Image` is a few bytes of JS object in front of megabytes of
//! Skia raster. V8 only counts what it allocated itself, so a process can hold
//! gigabytes of orphaned surfaces at a 50 MB heap and never collect. Node-API
//! has `napi_adjust_external_memory` for exactly this: the reported total
//! raises the collector's pressure, per environment (isolate), and Node aborts
//! if a decrement takes it below zero. Neon does not bind the call, so this
//! module resolves it from the host process at load time, the way Neon binds
//! the rest of Node-API.
//!
//! `process.memoryUsage().external` does not include this counter on current
//! Node versions (V8 keeps it apart from the heap statistics), but the
//! collector's external-memory pressure does, which is what matters.
//! `externalMemory()` on the module reads the counter.
//!
//! Each owner keeps an [`ExternalMemory`] ledger. [`ExternalMemory::sync`]
//! reports the difference between what the object owns now and what it last
//! reported; the finalizer reports the remainder back. Every adjustment runs
//! on the thread and in the environment of the JS call that caused it, so
//! the per-environment total never goes negative: an object only ever takes
//! back what it added to the same environment.

use neon::{prelude::*, sys::bindings::Env};
use std::sync::OnceLock;

/// `napi_adjust_external_memory(env, change_in_bytes, *result)`.
type AdjustFn = unsafe extern "C" fn(Env, i64, *mut i64) -> i32;

/// The resolved Node-API function, or `None` when the host does not export it
/// (an embedder without Node-API, or a test harness), in which case the ledger
/// still balances but nothing is reported.
fn adjust_fn() -> Option<AdjustFn> {
    static ADJUST: OnceLock<Option<AdjustFn>> = OnceLock::new();
    *ADJUST.get_or_init(|| {
        // SAFETY: `Library::this()` wraps the running executable, which
        // outlives the addon, and the symbol's signature is Node-API's
        // documented one. The pointer is copied out of the `Symbol` guard,
        // which is sound for a symbol of the main program (it is never
        // unloaded).
        let resolved = unsafe {
            #[cfg(unix)]
            let host: libloading::Library =
                libloading::os::unix::Library::this().into();
            #[cfg(windows)]
            let host: libloading::Library =
                libloading::os::windows::Library::this().ok()?.into();
            let f = host
                .get::<AdjustFn>(b"napi_adjust_external_memory\0")
                .map(|symbol| *symbol);
            // keep the handle alive for the life of the process: dropping it
            // would try to unload the host
            std::mem::forget(host);
            f
        };
        match resolved {
            Ok(f) => Some(f),
            Err(err) => {
                debug_log(&format!(
                    "napi_adjust_external_memory not resolved: {err}"
                ));
                None
            }
        }
    })
}

/// `SKIA_CANVAS_DEBUG=1` prints why native memory is not being reported.
fn debug_log(message: &str) {
    if std::env::var_os("SKIA_CANVAS_DEBUG").is_some() {
        eprintln!("skia-canvas: {message}");
    }
}

/// `externalMemory()`: the bytes this environment currently has reported,
/// from every source that uses the same Node-API counter (this library's
/// rasters among them). `process.memoryUsage().external` does not include
/// the counter on current Node versions; this does.
pub fn external_memory(mut cx: FunctionContext) -> JsResult<JsNumber> {
    let mut total = 0i64;
    if let Some(f) = adjust_fn() {
        // SAFETY: a zero adjustment only reads the counter; `cx.to_raw()` is
        // the live environment of this call.
        unsafe {
            f(cx.to_raw(), 0, &mut total);
        }
    }
    Ok(cx.number(total as f64))
}

/// Report `delta` bytes to the environment of `cx`.
fn adjust<'a>(cx: &mut impl Context<'a>, delta: i64) {
    if delta != 0
        && let Some(f) = adjust_fn()
    {
        let mut total = 0i64;
        // SAFETY: `cx.to_raw()` is the live environment of this call, and
        // `total` outlives the call. The result is informational.
        let status = unsafe { f(cx.to_raw(), delta, &mut total) };
        if status != 0 {
            debug_log(&format!(
                "napi_adjust_external_memory({delta}) failed: status {status}"
            ));
        }
    }
}

/// Bytes an object has reported to V8 so far.
// Not `Clone`: a copied ledger would take its bytes back twice.
#[derive(Debug, Default)]
pub struct ExternalMemory {
    reported: i64,
}

impl ExternalMemory {
    /// Bring the report up to `owned`, the bytes the object holds now.
    pub fn sync<'a>(&mut self, cx: &mut impl Context<'a>, owned: i64) {
        let owned = owned.max(0);
        adjust(cx, owned - self.reported);
        self.reported = owned;
    }

    /// Take back everything reported; for the finalizer and for `release()`.
    pub fn clear<'a>(&mut self, cx: &mut impl Context<'a>) {
        self.sync(cx, 0);
    }

    /// What is currently reported.
    pub fn reported(&self) -> i64 {
        self.reported
    }
}
