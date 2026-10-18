const USAGE: &str = "Usage:
  blyacut-mcp /path/to/session.json            control the running desktop app (enable agent access first)
  blyacut-mcp --headless DIR [--autonomous]    standalone studio limited to DIR; --autonomous applies removals without human review";
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if args.first().is_some_and(|a| a == "--headless") {
        let autonomous = args.iter().any(|a| a == "--autonomous");
        let dirs: Vec<_> = args[1..].iter().filter(|a| *a != "--autonomous").collect();
        match dirs.as_slice() {
            [dir] => blyacut_mcp::run_headless(dir.into(), autonomous),
            _ => Err(USAGE.to_string()),
        }
    } else {
        args.first()
            .cloned()
            .or_else(|| std::env::var_os("BLYACUT_SESSION"))
            .ok_or_else(|| USAGE.to_string())
            .and_then(|path| blyacut_mcp::run(path.into()))
    };
    if let Err(error) = result {
        eprintln!("BlyaCut MCP: {error}");
        std::process::exit(1);
    }
}
