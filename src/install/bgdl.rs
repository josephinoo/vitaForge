use anyhow::{Result, bail};
#[cfg(target_os = "vita")]
use anyhow::Context;
#[cfg(target_os = "vita")]
use std::ffi::CString;
#[cfg(target_os = "vita")]
use std::sync::OnceLock;

#[cfg(target_os = "vita")]
use vitasdk_sys::*;

pub const BGDL_TYPE_PSP: u32 = 0x00;
#[allow(dead_code)]
pub const BGDL_TYPE_PSM: u32 = 0x06;
pub const BGDL_TYPE_GAME: u32 = 0x16;
#[allow(dead_code)]
pub const BGDL_TYPE_DLC: u32 = 0x17;

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
    *mut SceIpmiDownloadParam,
) -> i32;

#[cfg(target_os = "vita")]
#[repr(C)]
struct SceDownloadClass {
    // Heap-allocated and leaked so the address stays stable for the whole
    // process — SceShellSvc is handed this pointer during `init()` and may
    // retain it internally, so it must not move once initialization starts.
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
        // `load_res` is a module ID (>= 0) on success, or a negative SCE
        // error code. It was previously discarded, which meant a failure
        // here was indistinguishable from the export lookup below failing
        // for its own separate reason (missing taiHEN, wrong firmware NIDs,
        // etc.) — logging it narrows down which of the two actually failed.
        if load_res < 0 {
            eprintln!("sceKernelLoadStartModule(libshellsvc.suprx) returned {load_res:#010x}");
        }

        let mut func_4e255c31_addr: usize = 0;
        let mut func_b282b430_addr: usize = 0;
        let mod_name = CString::new("SceShellSvc").unwrap();

        let tai_res_1 = taiGetModuleExportFunc(
            mod_name.as_ptr(),
            0xF4E34EDB,
            0x4E255C31,
            &mut func_4e255c31_addr,
        );

        let tai_res_2 = taiGetModuleExportFunc(
            mod_name.as_ptr(),
            0xF4E34EDB,
            0xB282B430,
            &mut func_b282b430_addr,
        );

        if func_4e255c31_addr == 0 || func_b282b430_addr == 0 {
            return Err(format!(
                "Failed to resolve SceShellSvc exports (module load: {load_res:#010x}, \
                 export 0x4E255C31: {} [tai rc {tai_res_1:#010x}], \
                 export 0xB282B430: {} [tai rc {tai_res_2:#010x}]) — \
                 check that taiHEN is installed and up to date for this firmware",
                if func_4e255c31_addr == 0 { "not found" } else { "found" },
                if func_b282b430_addr == 0 { "not found" } else { "found" },
            ));
        }

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
            buf_c4: vec![0u32; res as usize].leak().as_mut_ptr(),
            buf_10000: vec![0u32; 0x1000].leak().as_mut_ptr(),
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

#[cfg(not(target_os = "vita"))]
pub fn start_bgdl(_title: &str, _url: &str, _rif: Option<&[u8]>, _bgdl_type: u32) -> Result<()> {
    bail!("BGDL is not supported on the host simulator.")
}

#[cfg(target_os = "vita")]
static NEXT_LICENSE_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[cfg(target_os = "vita")]
pub fn start_bgdl(title: &str, url: &str, rif: Option<&[u8]>, bgdl_type: u32) -> Result<()> {
    std::fs::create_dir_all("ux0:bgdl").ok();

    // Unique per call so two downloads queued back-to-back don't clobber
    // each other's license before the system daemon has read it.
    let id = NEXT_LICENSE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let license_path = format!("ux0:bgdl/temp_{id}.dat");

    let rif_str = if let Some(rif_data) = rif {
        std::fs::write(&license_path, rif_data).context("Failed to write temporary license file")?;
        license_path.as_str()
    } else {
        ""
    };

    let sce_download_obj = get_download_class()?;

    unsafe {
        let func_table = *(*sce_download_obj.class_header).func_table;
        let shell_func_8 = *func_table.add(8);

        let mut result: i32 = 0;
        let mut bgdlid: u32 = 1;

        let mut buf_dc0 = vec![0u8; 0xDC0];
        let addr_dc0 = buf_dc0.as_mut_ptr() as *mut IpmiDownloadParam;

        let mut buf_2e0 = vec![0u8; 0x2E0];

        let mut params = SceIpmiDownloadParam {
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
            p_bgdl_id: &mut bgdlid,
            unk_5: 4,
            result: &mut result,
            unk_4_2: 0,
            shell_func_8,
        };

        // Fix up internal self-referential pointers to match C++ usagi-pkgj bgdl.cpp exactly:
        // params.init.ptr_to_dc0_ptr = &params.init.addr_DC0
        // params.init.ptr_to_2e0_ptr = &params.addr_2E0
        let init_ref = &mut params.u.init;
        init_ref.ptr_to_dc0_ptr = &mut init_ref.addr_dc0 as *mut _;
        init_ref.ptr_to_2e0_ptr = &mut params.addr_2e0 as *mut _ as *mut _;

        let copy_cstr = |src: &str, dst: &mut [u8]| {
            let bytes = src.as_bytes();
            let max_len = dst.len().saturating_sub(1);
            let len = bytes.len().min(max_len);
            dst[..len].copy_from_slice(&bytes[..len]);
            dst[len] = 0;
        };

        copy_cstr(url, &mut (*addr_dc0).url);
        if !rif_str.is_empty() {
            copy_cstr(rif_str, &mut (*addr_dc0).license_path);
        }
        copy_cstr(title, &mut (*addr_dc0).title);
        copy_cstr("ux0:bgdl/icon0.png", &mut (*addr_dc0).icon_path);

        (*addr_dc0).type_[0] = bgdl_type;
        (*addr_dc0).type_[1] = bgdl_type;

        let p_ptr_to_dc0_ptr = init_ref.ptr_to_dc0_ptr;

        let res_change = sce_download_obj.change_state.unwrap()(
            (*sce_download_obj.class_header).func_table,
            0x12340012,
            p_ptr_to_dc0_ptr,
            1,
            &mut params,
        );

        if res_change < 0 || result < 0 || (bgdlid as i32) < 0 {
            bail!("SceDownload change_state failed. res:{:#08x} result:{:#08x}", res_change, result);
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

        let mut params2 = SceIpmiDownloadParam {
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
            &mut params2,
        );

        if res_change2 < 0 || result < 0 {
            bail!("SceDownload second change_state failed. res:{:#08x} result:{:#08x}", res_change2, result);
        }
    }

    Ok(())
}
