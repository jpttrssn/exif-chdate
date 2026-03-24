use std::env;
use std::ffi::OsStr;

use futures::future::join_all;
use tokio::process::Command;
use tokio::task::JoinHandle;

/// Print usage and exit.
fn usage() -> ! {
    eprintln!(
        "Usage: exif-film <year>-<month>-<day> <film> <process> <camera> <lens> <file1> [file2 ...]\n\
        \n\
        <year>-<month>-<day>    Example: 1999-01-01\n\
        <film>                  Type of film and ISO. Example: Ilford HP5+ @1600\n\
        <process>               Film process. Example: Rodinal 1+25 @1600\n\
        <camera>                Original camera\n\
        <lens>                  Original lens\n\
        <file…>                 One or more image files to modify\n\
        \n\
        The date will overwrite the `DateTimeOriginal` tag starting at time 00:00:00 and incremeting\n\
        by 1 second in order of the filnames, while the rest of the fields will overwrite the\n\
        `UserComment` tag separated by `;`. The `@` character is a convention and meant to be used as\n\
        a marker that the following numeric token is an ISO identifier allowing you to provide\n\
        \"shot at\" and \"processed at\" ISO values.\n\
        \n\
        Will also update the `DateTimeOriginal` in any correspodning `XMP` sidecar files. You may need\n\
        to re-import your photos into which ever photo library you use afterwards."
    );
    std::process::exit(1);
}

/// Write EXIF tags using exiftool.
async fn write_exif_tags<T>(
    file: T,
    date_time_original: &str,
    user_comment: &str,
) -> std::io::Result<()>
where
    T: AsRef<OsStr>,
{
    let mut cmd = Command::new("exiftool");

    cmd.arg("-overwrite_original")
        .arg(format!("-DateTimeOriginal={}", date_time_original))
        .arg(format!("-UserComment={}", user_comment))
        .arg(file);

    let status = cmd.status().await?;

    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "exiftool returned non‑zero status",
        ))
    }
}

/// Update xmp file if it exists
async fn update_xmp<T>(file: T, date_time_original: &str)
where
    T: AsRef<OsStr>,
{
    let mut cmd = Command::new("sed");

    let mut xmp = file.as_ref().to_os_string();
    xmp.push(".xmp");

    let sed_status = cmd
        .arg("-i")
        .arg(format!(
            "s/exif:DateTimeOriginal=\"[^\"]*\"/exif:DateTimeOriginal=\"{}\"/g",
            date_time_original
        ))
        .arg(&xmp)
        .status()
        .await;

    if let Some(_) = sed_status.ok() {
        println!("Updated XMP: {}", xmp.display());
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() < 6 {
        usage();
    }

    let date = args.get(0).unwrap().clone();
    let comment = {
        let film = args.get(1).unwrap();
        let process = args.get(2).unwrap();
        let camera = args.get(3).unwrap();
        let lens = args.get(4).unwrap();

        format!("{};{};{};{}", film, process, camera, lens)
    };

    let mut files = args[5..].to_vec();
    files.sort_by(|a, b| a.cmp(b));

    let handles: Vec<JoinHandle<_>> = files
        .into_iter()
        .enumerate()
        .map(|(i, file)| {
            let original_date_time = format!("{} {}", date, seconds_to_time(i));
            let comment = comment.clone();

            tokio::spawn(async move {
                match write_exif_tags(&file, &original_date_time, &comment).await {
                    Ok(_) => {
                        println!("OK: {}", file);
                        update_xmp(&file, &original_date_time).await;
                        Ok(())
                    }
                    Err(err) => {
                        println!("Error: {}", err);
                        Err(err)
                    }
                }
            })
        })
        .collect();

    // Wait for all tasks to finish
    let results = join_all(handles).await;

    // Count successes
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    let total_count = results.len();

    println!("\n--- Summary ---");
    println!(
        "Successfully wrote tags to {} of {} files.",
        success_count, total_count
    );
}

fn seconds_to_time(total_seconds: usize) -> String {
    // Ensure we don't exceed 24 hours (86400 seconds)
    // If the input represents seconds past midnight, we usually want to wrap around
    let seconds_in_day = total_seconds % 86_400;

    let hours = seconds_in_day / 3600;
    let remaining = seconds_in_day % 3600;
    let minutes = remaining / 60;
    let seconds = remaining % 60;

    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
