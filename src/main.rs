use vita_newlib_shims as _;

mod app;
mod data;
mod input;
mod install;
mod net;
mod shell;

use app::App;

#[used]
#[unsafe(export_name = "sceUserMainThreadStackSize")]
pub static SCE_USER_MAIN_THREAD_STACK_SIZE: u32 = 4 * 1024 * 1024;

#[used]
#[unsafe(export_name = "sceLibcHeapSize")]
pub static SCE_LIBC_HEAP_SIZE: u32 = 24 * 1024 * 1024;

#[used]
#[unsafe(export_name = "_newlib_heap_size_user")]
pub static NEWLIB_HEAP_SIZE_USER: u32 = 96 * 1024 * 1024;

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "vita")]
    unsafe {
        let _ = vitasdk_sys::sceSysmoduleLoadModule(vitasdk_sys::SCE_SYSMODULE_IME);
    }

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
