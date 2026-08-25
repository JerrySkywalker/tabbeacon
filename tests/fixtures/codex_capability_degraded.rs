use std::env;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--version" => println!("codex-cli 99.99.99"),
        [features, list] if features == "features" && list == "list" => {
            println!("hooks stable true");
        }
        _ => std::process::exit(2),
    }
}
