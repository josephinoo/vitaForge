fn main() {
    // `option_env!` bakes the value in at compile time, so without these the
    // catalog url would stay stale until something else forced a rebuild.
    println!("cargo:rerun-if-env-changed=SERVER_URL");
    println!("cargo:rerun-if-env-changed=GITHUB_API_URL");
    println!("cargo:rerun-if-changed=.env");

    // Day-of-month + time of the build, e.g. `03-1847`. Shown on screen so a stale
    // install on the device is caught at a glance instead of by symptom.
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

/// Day of the month for a unix day count, utc. Enough calendar for a build tag.
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
