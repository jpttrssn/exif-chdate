use std::collections::VecDeque;
use std::env;
use std::ffi::OsStr;

use tokio::process::Command;

/// Print usage and exit.
fn usage() -> ! {
    eprintln!(
        "Usage: exif-film <year>::<month>::<day> <film> <process> <camera> <lens> <file1> [file2 ...]\n\
        \n\
        <year>:<month>:<day>    Example: 1999:01:01\n\
        <film>                  Type of film and ISO. Example: Ilford HP5+ @1600\n\
        <process>               Film process. Example: Rodinal 1+25 @1600\n\
        <camera>                Original camera\n\
        <lens>                  Original lens\n\
        <file…>                 One or more image files to modify\n\
        \n\
        The date will overwrite the `DateTimeOriginal` tag, while the rest of the fields will overwrite\n\
        the `UserComment` tag separated by `;`. The `@` character is a convention and meant to be used as\n\
        a marker that the following numeric token is an ISO identifier allowing you to provide\n\
        \"shot at\" and \"processed at\" ISO values."
    );
    std::process::exit(1);
}

/// Write EXIF tags using exiftool.
async fn write_exif_tags<I>(
    files: I,
    date_time_original: &str,
    user_comment: &str,
) -> std::io::Result<()>
where
    I: IntoIterator,
    I::Item: AsRef<OsStr>,
{
    let mut cmd = Command::new("exiftool");

    cmd.arg("-overwrite_original")
        .arg(format!("-DateTimeOriginal={}", date_time_original))
        .arg(format!("-UserComment={}", user_comment));

    for file in files {
        cmd.arg(file);
    }

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

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let mut args: VecDeque<String> = env::args().skip(1).collect();
    if args.len() < 6 {
        usage();
    }

    let date = args.pop_front().unwrap();
    let film = args.pop_front().unwrap();
    let process = args.pop_front().unwrap();
    let camera = args.pop_front().unwrap();
    let lens = args.pop_front().unwrap();

    // Remaining arguments are file paths.
    if args.is_empty() {
        eprintln!("No image files supplied.");
        std::process::exit(1);
    }

    let original_date_time = format!("{} 00:00:00", date);
    let comment = format!("{};{};{};{}", film, process, camera, lens);
    match write_exif_tags(&args, &original_date_time, &comment).await {
        Ok(_) => println!("OK"),
        Err(err) => println!("Error: {}", err),
    }
}
