use std::{fs, path::PathBuf};

use clap::CommandFactory;
use clap_complete::{
    generate_to,
    shells::{Bash, Elvish, Fish, PowerShell, Zsh},
};
use temporal_tui::cli::Cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let completions = root.join("assets").join("completions");
    let man = root.join("assets").join("man");
    fs::create_dir_all(&completions)?;
    fs::create_dir_all(&man)?;

    generate_to(Bash, &mut Cli::command(), "temporal-tui", &completions)?;
    generate_to(Zsh, &mut Cli::command(), "temporal-tui", &completions)?;
    generate_to(Fish, &mut Cli::command(), "temporal-tui", &completions)?;
    generate_to(
        PowerShell,
        &mut Cli::command(),
        "temporal-tui",
        &completions,
    )?;
    generate_to(Elvish, &mut Cli::command(), "temporal-tui", &completions)?;

    for entry in fs::read_dir(&man)? {
        let path = entry?.path();
        if path.is_file()
            && path.extension().is_some_and(|extension| extension == "1")
            && path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("temporal-tui"))
        {
            fs::remove_file(path)?;
        }
    }
    clap_mangen::generate_to(Cli::command(), &man)?;
    for entry in fs::read_dir(&man)? {
        let path = entry?.path();
        if !path.is_file() || path.extension().is_none_or(|extension| extension != "1") {
            continue;
        }
        let manpage = fs::read_to_string(&path)?;
        let normalized = manpage
            .lines()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(path, format!("{}\n", normalized.trim_end()))?;
    }
    Ok(())
}
