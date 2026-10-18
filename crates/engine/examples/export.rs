//! Useful for reproducible render checks without a desktop session.
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(
            "Usage: cargo run -p blyacut-engine --example export -- project.blyacut output.mp4"
                .into(),
        );
    }
    let input = PathBuf::from(&args[0])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let project = blyacut_core::load(&input)?;
    blyacut_engine::render(
        &project,
        &PathBuf::from(&args[1]),
        Arc::new(AtomicBool::new(false)),
        |progress| eprintln!("{:.0}%", progress * 100.0),
    )
}
