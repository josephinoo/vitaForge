use anyhow::{Result, bail};
#[cfg(target_os = "vita")]
use anyhow::Context;
#[cfg(target_os = "vita")]
use std::ffi::CString;
#[cfg(target_os = "vita")]
use std::sync::OnceLock;
#[cfg(target_os = "vita")]
use vitasdk_sys::*;
#[cfg(target_os = "vita")]
use super::installed::vita_fs;
pub const BGDL_TYPE_PSP: u32 = 0x00;
#[allow(dead_code)]
pub const BGDL_TYPE_PSM: u32 = 0x06;
pub const BGDL_TYPE_GAME: u32 = 0x16;
#[allow(dead_code)]
pub const BGDL_TYPE_DLC: u32 = 0x17;
#[cfg(target_os = "vita")]
const RIF_SIZE: usize = 512;
#[cfg(target_os = "vita")]
const PSP_RIF_SIZE: usize = 0x98;
#[cfg(target_os = "vita")]
const PSM_RIF_SIZE: usize = 1024;
#[cfg(target_os = "vita")]
const MAX_QUEUED: usize = 32;
#[cfg(target_os = "vita")]
#[repr(C)]
struct ShellSvcInitStruct {
    unk_0: u32,
    name: [u8; 0x10],
    unk_ptr: *mut std::ffi::c_void,
    unk_1: u32,
    size1: u32,
    size2: u32,
    unk_2: u32,
    unk_3: u32,
    unk_4: u32,
    unk_5: u32,
    padding: [u8; 0x84],
    unk_7: u32,
    unk_8: u32,
    unk_ptr_2: *mut std::ffi::c_void,
    padding2: [u8; 0x88],
}
#[cfg(target_os = "vita")]
#[repr(C)]
struct IpmiDownloadParam {
    type_: [u32; 2],
    unk_0x08: [u8; 0x68],
    url: [u8; 0x800],
    icon_path: [u8; 0x100],
    title: [u8; 0x33a],
    license_path: [u8; 0x100],
    unk_0xdaa: [u8; 0x16],
}
#[cfg(target_os = "vita")]
#[repr(C)]
struct SceIpmiDownloadParamInit {
    ptr_to_dc0_ptr: *mut *mut IpmiDownloadParam,
    ptr_to_2e0_ptr: *mut *mut std::ffi::c_void,
    unk_1: u32,
    unk_2: u32,
    unk_3: u32,
    addr_dc0: *mut IpmiDownloadParam,
    size_dc0: u32,
}
#[cfg(target_os = "vita")]
#[repr(C)]
struct SceIpmiDownloadParamState {
    result: *mut i32,
    unk_2: u32,
    unk_3: u32,
    unk_4: u32,
    unk_5: u32,
    unk_6: u32,
    unk_7: u32,
}
#[cfg(target_os = "vita")]
#[repr(C)]
union SceIpmiDownloadParamUnion {
    init: std::mem::ManuallyDrop<SceIpmiDownloadParamInit>,
    state: std::mem::ManuallyDrop<SceIpmiDownloadParamState>,
}
#[cfg(target_os = "vita")]
#[repr(C)]
struct SceIpmiDownloadParam {
    u: SceIpmiDownloadParamUnion,
    addr_2e0: *mut std::ffi::c_void,
    size2e0: u32,
    unk_4: u32,
    p_bgdl_id: *mut u32,
    unk_5: u32,
    result: *mut i32,
    unk_4_2: u32,
    shell_func_8: u32,
}
#[cfg(target_os = "vita")]
#[repr(C)]
struct SceDownloadClassHeader {
    unk0: u32,
    unk1: u32,
    unk2: u32,
    func_table: *mut *mut u32,
    unk3: u32,
    buf_c4: *mut u32,
    buf_10000: *mut u32,
}
#[cfg(target_os = "vita")]
type SceDownloadInit = extern "C" fn(
    *mut *mut u32,
    *mut u32,
    i32,
    *mut ShellSvcInitStruct,
    i32,
) -> i32;
#[cfg(target_os = "vita")]
type SceDownloadChangeState = extern "C" fn(
    *mut *mut u32,
    i32,
    *mut *mut IpmiDownloadParam,
    i32,
    SceIpmiDownloadParam,
) -> i32;
#[cfg(target_os = "vita")]
#[repr(C)]
struct SceDownloadClass {
    init_header: *mut ShellSvcInitStruct,
    class_header: *mut SceDownloadClassHeader,
    init: Option<SceDownloadInit>,
    change_state: Option<SceDownloadChangeState>,
}
#[cfg(target_os = "vita")]
unsafe impl Send for SceDownloadClass {}
#[cfg(target_os = "vita")]
unsafe impl Sync for SceDownloadClass {}
#[cfg(target_os = "vita")]
#[link(name = "taihen_stub")]
unsafe extern "C" {
    fn taiGetModuleExportFunc(
        module_name: *const std::ffi::c_char,
        library_nid: u32,
        func_nid: u32,
        func_ptr: *mut usize,
    ) -> i32;
}
#[cfg(target_os = "vita")]
static DOWNLOAD_CLASS_SINGLETON: OnceLock<Result<SceDownloadClass, String>> = OnceLock::new();
#[cfg(target_os = "vita")]
fn get_download_class() -> Result<&'static SceDownloadClass> {
    let res = DOWNLOAD_CLASS_SINGLETON.get_or_init(|| unsafe {
        let lib_path = CString::new("vs0:sys/external/libshellsvc.suprx").unwrap();
        let load_res = sceKernelLoadStartModule(
            lib_path.as_ptr(),
            0,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if load_res < 0 {
            eprintln!("sceKernelLoadStartModule(libshellsvc.suprx) returned {load_res:#010x}");
        }
        let mod_name = CString::new("SceShellSvc").unwrap();
        let mut func_4e255c31_addr: usize = 0;
        let mut func_b282b430_addr: usize = 0;
        let res1 = taiGetModuleExportFunc(mod_name.as_ptr(), 0xF4E34EDB, 0x4E255C31, &mut func_4e255c31_addr);
        let res2 = taiGetModuleExportFunc(mod_name.as_ptr(), 0xF4E34EDB, 0xB282B430, &mut func_b282b430_addr);
        if func_4e255c31_addr == 0 || func_b282b430_addr == 0 {
            let err = format!(
                "Failed to resolve SceShellSvc exports (module load: {load_res:#010x}, \
                 export 0x4E255C31: {} [tai rc {res1:#010x}], \
                 export 0xB282B430: {} [tai rc {res2:#010x}]) — \
                 check that taiHEN is installed and up to date for this firmware",
                if func_4e255c31_addr == 0 { "not found" } else { "found" },
                if func_b282b430_addr == 0 { "not found" } else { "found" },
            );
            log_bgdl(&err);
            return Err(err);
        }
        log_bgdl(&format!("SceShellSvc NIDs resolved! 0x4E255C31={:#010x}, 0xB282B430={:#010x}", func_4e255c31_addr, func_b282b430_addr));
        let sce_ipmi_4e255c31: extern "C" fn(*const u8, i32) -> i32 =
            std::mem::transmute(func_4e255c31_addr);
        let sce_ipmi_b282b430: extern "C" fn(
            *mut *mut *mut u32,
            *const u8,
            *mut SceDownloadClassHeader,
            *mut u32,
        ) -> i32 = std::mem::transmute(func_b282b430_addr);
        let init_header_ptr: *mut ShellSvcInitStruct = Box::into_raw(Box::new(std::mem::zeroed()));
        let mut sce_download_obj = SceDownloadClass {
            init_header: init_header_ptr,
            class_header: std::ptr::null_mut(),
            init: None,
            change_state: None,
        };
        std::ptr::copy_nonoverlapping(
            b"SceDownload\0".as_ptr(),
            (*init_header_ptr).name.as_mut_ptr(),
            12,
        );
        (*init_header_ptr).unk_1 = 1;
        (*init_header_ptr).size1 = 0x1E00;
        (*init_header_ptr).size2 = 0x1E00;
        (*init_header_ptr).unk_2 = 1;
        (*init_header_ptr).unk_3 = 0x0F00;
        (*init_header_ptr).unk_4 = 0x0F00;
        (*init_header_ptr).unk_5 = 1;
        (*init_header_ptr).unk_7 = 2;
        (*init_header_ptr).unk_8 = u32::MAX;
        let res = sce_ipmi_4e255c31((*init_header_ptr).name.as_ptr(), 0x1E00);
        if res != 0xc4 {
            return Err(format!("SceIpmi_4E255C31 failed: {:#08x}", res));
        }
        let class_header = Box::new(SceDownloadClassHeader {
            unk0: 0,
            unk1: 0,
            unk2: 0,
            func_table: std::ptr::null_mut(),
            unk3: 0,
            buf_c4: vec![0u32; (res as usize).div_ceil(4)].leak().as_mut_ptr(),
            buf_10000: vec![0u32; 0x1000 / 4].leak().as_mut_ptr(),
        });
        let class_header_ptr = Box::into_raw(class_header);
        let init_res = sce_ipmi_b282b430(
            &mut (*class_header_ptr).func_table,
            (*init_header_ptr).name.as_ptr(),
            class_header_ptr,
            (*class_header_ptr).buf_10000,
        );
        if init_res != 0 {
            return Err(format!("SceIpmi_B282B430 init failed: {:#08x}", init_res));
        }
        sce_download_obj.class_header = class_header_ptr;
        let func_table = *(*sce_download_obj.class_header).func_table;
        let init_func_ptr = *func_table.add(1);
        let change_state_func_ptr = *func_table.add(5);
        sce_download_obj.init = Some(std::mem::transmute(init_func_ptr));
        sce_download_obj.change_state = Some(std::mem::transmute(change_state_func_ptr));
        let init_res2 = sce_download_obj.init.unwrap()(
            (*sce_download_obj.class_header).func_table,
            *(*sce_download_obj.class_header).func_table,
            0x14,
            init_header_ptr,
            2,
        );
        if init_res2 != 0 {
            return Err(format!("SceDownload init failed: {:#08x}", init_res2));
        }
        Ok(sce_download_obj)
    });
    match res {
        Ok(obj) => Ok(obj),
        Err(err) => bail!("{err}"),
    }
}
#[cfg(target_os = "vita")]
fn probe_fs() {
    const PATHS: &[&str] = &[
        "ux0:",
        "ux0:data",
        "ux0:data/vitaforge",
        "ux0:app",
        "ux0:app/VITAFORGE",
        "ux0:appmeta",
        "ux0:pspemu",
        "ux0:bgdl",
        "ux0:bgdl/t",
        "ur0:",
    ];
    for path in PATHS {
        log_probe(&format!("stat({path}) -> {:#010x}", vita_fs::stat_code(path)));
    }
    let probe_dir = "ux0:vitaforge_probe";
    let mk = vita_fs::mkdir(probe_dir, 0o777);
    log_probe(&format!("mkdir({probe_dir}) -> {mk:#010x}"));
    if mk >= 0 {
        log_probe(&format!("rmdir({probe_dir}) -> {:#010x}", vita_fs::rmdir(probe_dir)));
    }
}
#[cfg(target_os = "vita")]
fn probe_fs_once() {
    static PROBED: OnceLock<()> = OnceLock::new();
    PROBED.get_or_init(probe_fs);
}
#[cfg(target_os = "vita")]
pub fn start_bgdl(title: &str, url: &str, rif: Option<&[u8]>, bgdl_type: u32) -> Result<()> {
    probe_fs_once();
    let mk = vita_fs::mkdir("ux0:bgdl", 0o777);
    log_bgdl(&format!("sceIoMkdir(ux0:bgdl) -> {mk:#010x} (advisory)"));
    match vita_fs::list_dir("ux0:bgdl/t") {
        Ok(entries) => {
            log_bgdl(&format!("ux0:bgdl/t holds {} queued entries", entries.len()));
            if entries.len() >= MAX_QUEUED {
                bail!(
                    "There are too many pending installs on this console. Install them from the \
                     LiveArea notifications (or delete them) before downloading more."
                );
            }
        }
        Err(code) => log_bgdl(&format!(
            "couldn't read ux0:bgdl/t ({code:#010x}); skipping the queue-depth check"
        )),
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let license_path = format!("ux0:bgdl/temp_{nanos}.dat");
    let rif_str = if let Some(rif_data) = rif {
        let max = match bgdl_type {
            BGDL_TYPE_PSP => PSP_RIF_SIZE,
            BGDL_TYPE_PSM => PSM_RIF_SIZE,
            _ => RIF_SIZE,
        };
        let rif_data = &rif_data[..rif_data.len().min(max)];
        vita_save_file(&license_path, rif_data).context("Failed to write temporary license file")?;
        log_bgdl(&format!("wrote {} license bytes to {license_path}", rif_data.len()));
        license_path
    } else {
        String::new()
    };
    let sce_download_obj = get_download_class()?;
    unsafe {
        let func_table = *(*sce_download_obj.class_header).func_table;
        let shell_func_8 = *func_table.add(8);
        let mut result: i32 = 0;
        let mut bgdlid: i32 = 1;
        let mut buf_dc0 = vec![0u8; 0xDC0];
        let addr_dc0 = buf_dc0.as_mut_ptr() as *mut IpmiDownloadParam;
        let mut buf_2e0 = vec![0u8; 0x2E0];
        let mut params = Box::new(SceIpmiDownloadParam {
            u: SceIpmiDownloadParamUnion {
                init: std::mem::ManuallyDrop::new(SceIpmiDownloadParamInit {
                    ptr_to_dc0_ptr: std::ptr::null_mut(),
                    ptr_to_2e0_ptr: std::ptr::null_mut(),
                    unk_1: 2,
                    unk_2: u32::MAX,
                    unk_3: 0,
                    addr_dc0,
                    size_dc0: 0xDC0,
                }),
            },
            addr_2e0: buf_2e0.as_mut_ptr() as *mut std::ffi::c_void,
            size2e0: 0x2E0,
            unk_4: 0,
            p_bgdl_id: &mut bgdlid as *mut i32 as *mut u32,
            unk_5: 4,
            result: &mut result,
            unk_4_2: 0,
            shell_func_8,
        });
        let init = &raw mut params.u as *mut SceIpmiDownloadParamInit;
        (*init).ptr_to_dc0_ptr = &raw mut (*init).addr_dc0;
        (*init).ptr_to_2e0_ptr = &raw mut params.addr_2e0;
        let copy_cstr = |src: &str, dst: &mut [u8]| {
            let bytes = src.as_bytes();
            let max_len = dst.len().saturating_sub(1);
            let len = bytes.len().min(max_len);
            dst[..len].copy_from_slice(&bytes[..len]);
            dst[len] = 0;
        };
        copy_cstr(url, &mut (*addr_dc0).url);
        if !rif_str.is_empty() {
            copy_cstr(&rif_str, &mut (*addr_dc0).license_path);
        }
        copy_cstr(title, &mut (*addr_dc0).title);
        let icon_path = if std::path::Path::new("ux0:data/vitaforge/bgdl_icon.png").exists() {
            "ux0:data/vitaforge/bgdl_icon.png"
        } else {
            "ux0:bgdl/icon0.png"
        };
        copy_cstr(icon_path, &mut (*addr_dc0).icon_path);
        (*addr_dc0).type_[0] = bgdl_type;
        (*addr_dc0).type_[1] = bgdl_type;
        let p_ptr_to_dc0_ptr = (*init).ptr_to_dc0_ptr;
        log_bgdl(&format!("start_bgdl title='{title}', url='{url}', rif='{rif_str}'"));
        let res_change = sce_download_obj.change_state.unwrap()(
            (*sce_download_obj.class_header).func_table,
            0x12340012,
            p_ptr_to_dc0_ptr,
            1,
            std::ptr::read(&*params),
        );
        log_bgdl(&format!("change_state #1 res={res_change:#010x}, result={result:#010x}, bgdlid={bgdlid}"));
        if res_change < 0 || result < 0 || bgdlid < 0 {
            let err_msg = format!("SceDownload change_state failed. res:{res_change:#010x} result:{result:#010x} bgdlid:{bgdlid}");
            log_bgdl(&err_msg);
            bail!("{err_msg}");
        }
        result = 0;
        let state_param = SceIpmiDownloadParamState {
            result: &mut result,
            unk_2: 0,
            unk_3: 0,
            unk_4: 1,
            unk_5: 0,
            unk_6: 0,
            unk_7: 0x00000A0A,
        };
        let params2 = SceIpmiDownloadParam {
            u: SceIpmiDownloadParamUnion { state: std::mem::ManuallyDrop::new(state_param) },
            addr_2e0: std::ptr::null_mut(),
            size2e0: 0,
            unk_4: 0,
            p_bgdl_id: std::ptr::null_mut(),
            unk_5: 0,
            result: std::ptr::null_mut(),
            unk_4_2: 0,
            shell_func_8: 0,
        };
        let res_change2 = sce_download_obj.change_state.unwrap()(
            (*sce_download_obj.class_header).func_table,
            0x12340007,
            std::ptr::null_mut(),
            0,
            params2,
        );
        log_bgdl(&format!("change_state #2 res={res_change2:#010x}, result={result:#010x}"));
        if res_change2 < 0 || result < 0 {
            let err_msg = format!("SceDownload second change_state failed. res:{res_change2:#010x} result:{result:#010x}");
            log_bgdl(&err_msg);
            bail!("{err_msg}");
        }
        log_bgdl("BGDL successfully queued!");
        drop(params);
        drop(buf_dc0);
        drop(buf_2e0);
    }
    Ok(())
}
pub fn log_bgdl(msg: &str) {
    log_tagged("[BGDL LOG]", msg);
}
#[cfg(target_os = "vita")]
fn log_probe(msg: &str) {
    log_tagged("[BGDL PROBE]", msg);
}
fn log_tagged(tag: &str, msg: &str) {
    let _ = std::fs::create_dir_all("ux0:data/vitaforge");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("ux0:data/vitaforge/vitaforge.log") {
        use std::io::Write;
        let _ = writeln!(f, "{tag} {msg}");
    }
}
#[cfg(target_os = "vita")]
fn vita_save_file(path: &str, data: &[u8]) -> Result<()> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, data)
        .with_context(|| format!("failed to write '{path}'"))?;
    Ok(())
}
