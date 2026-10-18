fn main() {
    let path = std::env::args_os()
        .nth(1)
        .or_else(|| std::env::var_os("BLYACUT_SESSION"));
    let result = path
        .ok_or_else(|| {
            "Usage: blyacut-mcp /path/to/session.json (enable agent access in desktop first)"
                .to_string()
        })
        .and_then(|path| blyacut_mcp::run(path.into()));
    if let Err(error) = result {
        eprintln!("BlyaCut MCP: {error}");
        std::process::exit(1);
    }
}
