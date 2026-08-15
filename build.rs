fn main() {
    println!("cargo:rerun-if-env-changed=SERVER_URL");
    println!("cargo:rerun-if-env-changed=GITHUB_API_URL");
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-changed=src/install/puff.c");
    println!("cargo:rerun-if-changed=src/install/puff.h");
    println!("cargo:rerun-if-changed=src/install/zrif.c");
    println!("cargo:rerun-if-changed=src/install/zrif.h");
    println!("cargo:rerun-if-changed=src/media/vita_hw_img.c");
    println!("cargo:rerun-if-changed=src/media/vita_hw_img.h");
    cc::Build::new()
        .file("src/install/puff.c")
        .file("src/install/zrif.c")
        .file("src/media/vita_hw_img.c")
        .compile("zrif_c");
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let day_of_month = day_of_month(days);
    let hour = (seconds % 86_400) / 3600;
    let minute = (seconds % 3600) / 60;
    println!("cargo:rustc-env=BUILD_STAMP={day_of_month:02}-{hour:02}{minute:02}");
}
fn day_of_month(mut days: u64) -> u64 {
    let mut year = 1970;
    loop {
        let year_len = if leap(year) { 366 } else { 365 };
        if days < year_len {
            break;
        }
        days -= year_len;
        year += 1;
    }
    let lengths = [
        31,
        if leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for len in lengths {
        if days < len {
            return days + 1;
        }
        days -= len;
    }
    days + 1
}
fn leap(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
