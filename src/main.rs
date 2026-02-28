use std::env;
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;

/// Print usage and exit.
fn usage() -> ! {
    eprintln!(
        "Usage: exif_chdate <day> <month> [year] <file1> [file2 ...]\n\
        \n\
        <day>    Two‑digit day number   (01‑31)\n\
        <month>  Two‑digit month number (01‑12)\n\
        [year]   Optional four‑digit year (if omitted the original year is kept)\n\
        <file…>  One or more image files to modify"
    );
    std::process::exit(1);
}

/// Run `exiftool -DateTimeOriginal -s -s -s <file>` and return the raw string.
async fn get_original_datetime(file: &str) -> Option<String> {
    let output = Command::new("exiftool")
        .arg("-DateTimeOriginal")
        .arg("-s")
        .arg("-s")
        .arg("-s")
        .arg(file)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let txt = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if txt.is_empty() { None } else { Some(txt) }
}

/// Build the new DateTime string while preserving the original time‑of‑day
/// and any timezone offset.
fn build_new_datetime(
    orig: &str,
    new_year: Option<&str>,
    new_month: &str,
    new_day: &str,
) -> Option<String> {
    // Expected format: "YYYY:MM:DD HH:MM:SS[+|-]hh:mm"
    let mut parts = orig.splitn(2, ' ');
    let date_part = parts.next()?;
    let time_and_tz = parts.next()?; // e.g. "13:42:07+02:00" or "13:42:07"

    // Separate time from optional timezone offset.
    let (time_part, tz_offset) = if let Some(idx) = time_and_tz.find(['+', '-'].as_ref()) {
        (&time_and_tz[..idx], &time_and_tz[idx..])
    } else {
        (time_and_tz, "")
    };

    // Original year (keep if no new_year supplied)
    let orig_year = date_part.split(':').next()?;
    let year_to_use = new_year.unwrap_or(orig_year);

    Some(format!(
        "{}:{}:{} {}{}",
        year_to_use, new_month, new_day, time_part, tz_offset
    ))
}

/// Write the new DateTime string back to the three main EXIF tags using exiftool.
async fn write_new_datetime(file: &str, new_dt: &str) -> std::io::Result<()> {
    let status = Command::new("exiftool")
        .arg("-overwrite_original")
        .arg(format!("-DateTimeOriginal={}", new_dt))
        .arg(format!("-CreateDate={}", new_dt))
        .arg(format!("-ModifyDate={}", new_dt))
        .arg(file)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "exiftool returned non‑zero status",
        ))
    }
}

/// The async worker that processes a single file.
async fn process_file(
    file: String,
    month: String,
    day: String,
    year_opt: Option<String>,
    sem: Arc<Semaphore>,
) {
    // Acquire a permit so we don’t exceed the concurrency limit.
    let _permit = sem.acquire().await.unwrap();

    // 1️⃣ Read original timestamp.
    let orig_dt = match get_original_datetime(&file).await {
        Some(v) => v,
        None => {
            eprintln!(
                "⚠️  Could not read DateTimeOriginal from '{}'. Skipping.",
                file
            );
            return;
        }
    };

    // 2️⃣ Build the new timestamp.
    let new_dt = match build_new_datetime(&orig_dt, year_opt.as_deref(), &month, &day) {
        Some(v) => v,
        None => {
            eprintln!(
                "⚠️  Unexpected DateTimeOriginal format in '{}'. Skipping.",
                file
            );
            return;
        }
    };

    // 3️⃣ Write it back.
    match write_new_datetime(&file, &new_dt).await {
        Ok(_) => println!("✅ {} → {}", file, new_dt),
        Err(e) => eprintln!("❌ Failed to write EXIF for '{}': {}", file, e),
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // -----------------------------------------------------------------
    // Parse CLI arguments
    // -----------------------------------------------------------------
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 3 {
        usage();
    }

    // Day & month (basic validation)
    let day_num: u32 = args[0].parse().expect("invalid day");
    let month_num: u32 = args[1].parse().expect("invalid month");
    if !(1..=31).contains(&day_num) {
        eprintln!("Day must be between 1 and 31");
        std::process::exit(1);
    }
    if !(1..=12).contains(&month_num) {
        eprintln!("Month must be between 1 and 12");
        std::process::exit(1);
    }
    let month = format!("{:02}", month_num);
    let day = format!("{:02}", day_num);

    // Optional year?
    let mut idx = 2usize;
    let mut year_opt: Option<String> = None;
    if args.len() > idx && args[idx].len() == 4 && args[idx].chars().all(|c| c.is_ascii_digit()) {
        year_opt = Some(args[idx].clone());
        idx += 1;
    }

    // Remaining arguments are file paths.
    if args.len() <= idx {
        eprintln!("No image files supplied.");
        std::process::exit(1);
    }
    let files: Vec<String> = args[idx..].to_vec();

    // -----------------------------------------------------------------
    // Concurrency control – default to number of logical CPUs.
    // -----------------------------------------------------------------
    let max_concurrency = num_cpus::get(); // e.g. 8 on a typical laptop
    let semaphore = Arc::new(Semaphore::new(max_concurrency));

    // -----------------------------------------------------------------
    // Spawn a task for each file.
    // -----------------------------------------------------------------
    let mut handles: Vec<JoinHandle<()>> = Vec::with_capacity(files.len());

    for file in files {
        let month_clone = month.clone();
        let day_clone = day.clone();
        let year_clone = year_opt.clone();
        let sem_clone = semaphore.clone();

        // Each iteration creates an async task that owns its own copies of the data.
        let handle = tokio::spawn(async move {
            process_file(file, month_clone, day_clone, year_clone, sem_clone).await;
        });
        handles.push(handle);
    }

    // Wait for all tasks to finish.
    for h in handles {
        // If a task panics we surface the panic here.
        let _ = h.await;
    }
}
