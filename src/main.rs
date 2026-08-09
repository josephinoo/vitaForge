#[cfg(target_os = "vita")]
use vita_newlib_shims as _;
mod app;
mod data;
mod input;
mod install;
mod net;
mod shell;
use app::App;
// Consumed by the Vita loader; exporting these on a host build would collide
// with the platform's own symbols.
#[cfg(target_os = "vita")]
mod vita_runtime {
    #[used]
    #[unsafe(export_name = "sceUserMainThreadStackSize")]
    pub static SCE_USER_MAIN_THREAD_STACK_SIZE: u32 = 4 * 1024 * 1024;
    #[used]
    #[unsafe(export_name = "sceLibcHeapSize")]
    pub static SCE_LIBC_HEAP_SIZE: u32 = 24 * 1024 * 1024;
    #[used]
    #[unsafe(export_name = "_newlib_heap_size_user")]
    pub static NEWLIB_HEAP_SIZE_USER: u32 = 96 * 1024 * 1024;
}
// No attached console on real hardware, so a panic otherwise just vanishes — log it to disk.
// Doesn't cover allocation-failure aborts, only genuine panic!s.
#[cfg(target_os = "vita")]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::create_dir_all("ux0:data/vitaforge");
        use std::io::Write;
        if let Ok(mut file) =
            std::fs::OpenOptions::new().create(true).append(true).open("ux0:data/vitaforge/panic.log")
        {
            let _ = writeln!(file, "=== panic ===\n{info}");
        }
    }));
}
// Loads once at startup instead of on the search box's first open, which is otherwise the
#[cfg(target_os = "vita")]
fn preload_ime_module() {
    unsafe {
        let _ = vitasdk_sys::sceSysmoduleLoadModule(vitasdk_sys::SCE_SYSMODULE_IME);
    }
}
fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "vita")]
    install_panic_hook();
    #[cfg(target_os = "vita")]
    preload_ime_module();
    install::log_file(&format!(
        "=== VitaForge {} (built {}) starting ===",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_STAMP"),
    ));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .max_blocking_threads(2)
        .thread_stack_size(1024 * 1024)
        .build()?;
    let _guard = runtime.enter();
    let app = App::new()?;
    shell::run(app)
}
