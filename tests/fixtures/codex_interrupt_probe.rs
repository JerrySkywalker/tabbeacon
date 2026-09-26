use std::{env, fs, thread, time::Duration};

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [flag] if flag == "--version" => println!("codex-cli 0.156.1"),
        [features, list] if features == "features" && list == "list" => {
            let stalled = env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.join("hooks-stalled")))
                .is_some_and(|marker| marker.exists());
            if stalled {
                thread::sleep(Duration::from_secs(5));
            }
            let disabled = env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(|parent| parent.join("hooks-disabled")))
                .is_some_and(|marker| marker.exists());
            println!("hooks stable {}", !disabled);
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
