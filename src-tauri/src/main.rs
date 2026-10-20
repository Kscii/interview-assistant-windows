#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "--check-ocr") {
        if args.len() != 4 {
            std::process::exit(2);
        }
        let ok = interview_assistant::check_installed_ocr(
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
        );
        std::process::exit(if ok { 0 } else { 1 });
    }
    interview_assistant::run();
}
