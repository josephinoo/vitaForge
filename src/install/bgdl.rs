use anyhow::{Context, Result, bail};
use std::ffi::CString;

#[cfg(target_os = "vita")]
use vitasdk_sys::*;

pub const BGDL_TYPE_PSP: u32 = 0;
pub const BGDL_TYPE_PSM: u32 = 1;
pub const BGDL_TYPE_GAME: u32 = 2;
pub const BGDL_TYPE_DLC: u32 = 3;
pub const BGDL_TYPE_THEME: u32 = 4;

#[cfg(target_os = "vita")]
#[repr(C)]
struct ShellSvcInitStruct {
    unk_0: u32,
    name: [u8; 0x10],
    unk_ptr: *mut libc::c_void,
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
    unk_ptr_2: *mut libc::c_void,
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
    ptr_to_2e0_ptr: *mut *mut libc::c_void,
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
    addr_2e0: *mut libc::c_void,
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
    init_header: ShellSvcInitStruct,
    class_header: *mut SceDownloadClassHeader,
    init: Option<SceDownloadInit>,
    change_state: Option<SceDownloadChangeState>,
}

#[cfg(target_os = "vita")]
extern "C" {
    fn taiGetModuleExportFunc(
        module_name: *const libc::c_char,
        library_nid: u32,
        func_nid: u32,
        func_ptr: *mut usize,
    ) -> i32;
}

#[cfg(not(target_os = "vita"))]
pub fn start_bgdl(_title: &str, _url: &str, _rif: Option<&[u8]>, _bgdl_type: u32) -> Result<()> {
    // Simulator stub
    bail!("BGDL is not supported on the host simulator.")
}

#[cfg(target_os = "vita")]
pub fn start_bgdl(title: &str, url: &str, rif: Option<&[u8]>, bgdl_type: u32) -> Result<()> {
    if let Some(rif_data) = rif {
        let license_path = "ux0:bgdl/temp.dat";
        std::fs::create_dir_all("ux0:bgdl").ok();
        std::fs::write(license_path, rif_data).context("Failed to write temporary license file")?;
    }

    unsafe {
        let lib_path = CString::new("vs0:sys/external/libshellsvc.suprx").unwrap();
        sceKernelLoadStartModule(lib_path.as_ptr(), 0, std::ptr::null_mut(), 0, std::ptr::null_mut(), std::ptr::null_mut());

        let mut func_4e255c31_addr: usize = 0;
        let mut func_b282b430_addr: usize = 0;

        let mod_name = CString::new("SceShellSvc").unwrap();
        
        taiGetModuleExportFunc(
            mod_name.as_ptr(),
            0xF4E34EDB,
            0x4E255C31,
            &mut func_4e255c31_addr,
        );
        
        taiGetModuleExportFunc(
            mod_name.as_ptr(),
            0xF4E34EDB,
            0xB282B430,
            &mut func_b282b430_addr,
        );

        if func_4e255c31_addr == 0 || func_b282b430_addr == 0 {
            bail!("Failed to resolve SceShellSvc exports");
        }

        let sce_ipmi_4e255c31: extern "C" fn(*const u8, i32) -> i32 = std::mem::transmute(func_4e255c31_addr);
        let sce_ipmi_b282b430: extern "C" fn(*mut *mut *mut u32, *const u8, *mut SceDownloadClassHeader, *mut u32) -> i32 = std::mem::transmute(func_b282b430_addr);

        let mut sce_download_obj = Box::new(SceDownloadClass {
            init_header: std::mem::zeroed(),
            class_header: std::ptr::null_mut(),
            init: None,
            change_state: None,
        });

        std::ptr::copy_nonoverlapping(b"SceDownload\0".as_ptr(), sce_download_obj.init_header.name.as_mut_ptr(), 12);
        sce_download_obj.init_header.unk_1 = 1;
        sce_download_obj.init_header.size1 = 0x1E00;
        sce_download_obj.init_header.size2 = 0x1E00;
        sce_download_obj.init_header.unk_2 = 1;
        sce_download_obj.init_header.unk_3 = 0x0F00;
        sce_download_obj.init_header.unk_4 = 0x0F00;
        sce_download_obj.init_header.unk_5 = 1;
        sce_download_obj.init_header.unk_7 = 2;
        sce_download_obj.init_header.unk_8 = u32::MAX;

        let res = sce_ipmi_4e255c31(sce_download_obj.init_header.name.as_ptr(), 0x1E00);
        if res != 0xc4 {
            bail!("SceIpmi_4E255C31 failed: {:#08x}", res);
        }

        let mut class_header = Box::new(SceDownloadClassHeader {
            unk0: 0, unk1: 0, unk2: 0,
            func_table: std::ptr::null_mut(),
            unk3: 0,
            buf_c4: vec![0u32; res as usize].as_mut_ptr(),
            buf_10000: vec![0u32; 0x1000].as_mut_ptr(),
        });

        let init_res = sce_ipmi_b282b430(
            &mut class_header.func_table,
            sce_download_obj.init_header.name.as_ptr(),
            class_header.as_mut(),
            class_header.buf_10000,
        );
        if init_res != 0 {
            bail!("SceIpmi_B282B430 init failed: {:#08x}", init_res);
        }

        sce_download_obj.class_header = Box::into_raw(class_header);
        
        let func_table = *(*sce_download_obj.class_header).func_table;
        let init_func_ptr = *func_table.add(1);
        let change_state_func_ptr = *func_table.add(5);
        let shell_func_8 = *func_table.add(8);

        sce_download_obj.init = Some(std::mem::transmute(init_func_ptr));
        sce_download_obj.change_state = Some(std::mem::transmute(change_state_func_ptr));

        let init_res2 = sce_download_obj.init.unwrap()(
            (*sce_download_obj.class_header).func_table,
            *(*sce_download_obj.class_header).func_table,
            0x14,
            &mut sce_download_obj.init_header,
            2,
        );
        if init_res2 != 0 {
            bail!("SceDownload init failed: {:#08x}", init_res2);
        }

        let mut result: i32 = 0;
        let mut bgdlid: u32 = 1;

        let mut buf_dc0 = vec![0u8; 0xDC0];
        let addr_dc0 = buf_dc0.as_mut_ptr() as *mut IpmiDownloadParam;
        
        let mut buf_2e0 = vec![0u8; 0x2E0];

        let mut init_param = SceIpmiDownloadParamInit {
            ptr_to_dc0_ptr: &mut (addr_dc0 as *mut IpmiDownloadParam),
            ptr_to_2e0_ptr: &mut (buf_2e0.as_mut_ptr() as *mut libc::c_void),
            unk_1: 2,
            unk_2: u32::MAX,
            unk_3: 0,
            addr_dc0,
            size_dc0: 0xDC0,
        };

        let mut params = SceIpmiDownloadParam {
            u: SceIpmiDownloadParamUnion { init: std::mem::ManuallyDrop::new(init_param) },
            addr_2e0: buf_2e0.as_mut_ptr() as *mut libc::c_void,
            size2e0: 0x2E0,
            unk_4: 0,
            p_bgdl_id: &mut bgdlid,
            unk_5: 4,
            result: &mut result,
            unk_4_2: 0,
            shell_func_8,
        };

        std::ptr::copy_nonoverlapping(url.as_ptr(), (*addr_dc0).url.as_mut_ptr(), url.len().min(0x7FF));
        if rif.is_some() {
            std::ptr::copy_nonoverlapping(b"ux0:bgdl/temp.dat\0".as_ptr(), (*addr_dc0).license_path.as_mut_ptr(), 18);
        }
        std::ptr::copy_nonoverlapping(title.as_ptr(), (*addr_dc0).title.as_mut_ptr(), title.len().min(0x339));
        std::ptr::copy_nonoverlapping(b"ux0:bgdl/icon0.png\0".as_ptr(), (*addr_dc0).icon_path.as_mut_ptr(), 19);

        (*addr_dc0).type_[0] = bgdl_type;
        (*addr_dc0).type_[1] = bgdl_type;

        let mut p_ptr_to_dc0_ptr = params.u.init.ptr_to_dc0_ptr;

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
        
        let mut state_param = SceIpmiDownloadParamState {
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
