use std::{env, fs};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--version" => println!("codex-cli 0.147.0"),
        [features, list] if features == "features" && list == "list" => {
            println!("hooks stable true");
        }
        [server, generate, out_flag, output]
            if server == "app-server"
                && generate == "generate-json-schema"
                && out_flag == "--out" =>
        {
            fs::create_dir_all(output).expect("schema directory");
            fs::write(format!("{output}/schema.json"), "{\"hooks\":\"command\"}")
                .expect("schema fixture");
        }
        _ => std::process::exit(2),
    }
}
