use indicatif::{ProgressBar, ProgressStyle};

pub fn default_pull_progress_bar() -> ProgressBar {
    let style =
        ProgressStyle::with_template("[{bar:60.cyan/blue}] ({bytes}/{total_bytes}) {wide_msg}")
            .expect("failed to parse progress bar style template")
            .progress_chars("##-");
    let pb = ProgressBar::new(0);
    pb.set_style(style);
    pb
}

pub fn default_build_progress_bar() -> ProgressBar {
    let spinner_style = ProgressStyle::with_template("{spinner} Building image: `{msg}`")
        .unwrap()
        .tick_chars("⣾⣽⣻⢿⡿⣟⣯⣷");
    let pb = ProgressBar::new(1024);
    pb.set_style(spinner_style);
    pb
}
