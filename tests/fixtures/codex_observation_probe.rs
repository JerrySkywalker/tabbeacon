use std::{env, fs, io::Write, thread, time::Duration};
fn main() {
    let root = env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let args: Vec<_> = env::args().skip(1).collect();
    writeln!(fs::OpenOptions::new().create(true).append(true).open(root.join("calls.log")).unwrap(), "{}", args.first().unwrap()).unwrap();
    if root.join("slow").exists() { thread::sleep(Duration::from_secs(60)); }
    if root.join("flood").exists() { print!("{}", "x".repeat(128 * 1024)); return; }
    match args.as_slice() {
        [v] if v == "--version" => println!("codex-cli 0.147.0"),
        [a,b] if a == "features" && b == "list" => println!("hooks stable true"),
        [a,b,c,out] if a == "app-server" && b == "generate-json-schema" && c == "--out" => { fs::write(std::path::Path::new(out).join("schema.json"), "{\"hooks\":\"command\"}").unwrap(); },
        _ => std::process::exit(2),
    }
}
