use std::{
    env, fs,
    io::{self, Write},
    path::PathBuf,
    process,
};

fn main() -> io::Result<()> {
    let arguments = env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() < 3 || arguments[0] != "-C" {
        process::exit(90);
    }
    let cwd = PathBuf::from(&arguments[1]);
    let command = arguments[2..]
        .iter()
        .map(|value| value.to_string_lossy())
        .collect::<Vec<_>>()
        .join("\t");
    let audit_path = cwd
        .parent()
        .expect("probe cwd has an isolated parent")
        .join("git-command-audit.log");
    let mut audit = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(audit_path)?;
    writeln!(audit, "{command}")?;

    match arguments[2].to_string_lossy().as_ref() {
        "rev-parse" => {
            let git_dir = cwd.join(".git");
            println!("{}", cwd.display());
            println!("{}", git_dir.display());
            println!("{}", git_dir.display());
        }
        "config" => {
            io::stdout().write_all(
                b"remote.origin.url\nhttps://github.com/JerrySkywalker/tabbeacon.git\0",
            )?;
        }
        "rev-list" => println!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        _ => process::exit(91),
    }
    Ok(())
}
