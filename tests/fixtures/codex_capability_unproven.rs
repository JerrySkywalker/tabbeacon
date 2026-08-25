use std::env;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.as_slice() == ["--version"] {
        println!("codex-cli 99.99.99");
    } else {
        std::process::exit(2);
    }
}
