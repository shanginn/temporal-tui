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

    let mut manpage = Vec::new();
    clap_mangen::Man::new(Cli::command()).render(&mut manpage)?;
    let manpage = String::from_utf8(manpage)?;
    let normalized = manpage
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(man.join("temporal-tui.1"), format!("{normalized}\n"))?;
    Ok(())
}
