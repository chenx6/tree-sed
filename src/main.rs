use std::{
    fs::{read_to_string, OpenOptions},
    io::{self, Read, Write},
};

use anyhow::Context;
use clap::{arg, App, Arg};
#[cfg(feature = "c")]
use tree_sitter_c::language as c_language;
#[cfg(feature = "cpp")]
use tree_sitter_cpp::language as cpp_language;
#[cfg(feature = "rust")]
use tree_sitter_rust::language as rust_language;

mod script_executor;
mod script_parser;

use script_executor::execute_script;
use script_parser::parse;

fn main() -> anyhow::Result<()> {
    // TODO add more options to compatible with sed
    let app = App::new("tree-sed")
        .arg(arg!([SCRIPT]).required(true))
        .arg(arg!([FILE]))
        .arg(
            Arg::new("in-place")
                .short('i')
                .long("in-place")
                .help("edit files in place"),
        )
        .arg(arg!(--language ... "set language").default_value("c"));
    let matches = app.get_matches();
    let script = matches
        .value_of("SCRIPT")
        .context("Missing [SCRIPT] argument")?;
    let script = parse(script).context("[SCRIPT] format error")?;
    let mut source_code = match matches.value_of("FILE") {
        Some(file_name) => read_to_string(file_name)?,
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };
    let lang = match matches.value_of("language") {
        #[cfg(feature = "c")]
        Some("c") => c_language(),
        #[cfg(feature = "cpp")]
        Some("cpp") => cpp_language(),
        #[cfg(feature = "rust")]
        Some("rust") => rust_language(),
        Some(other) => return Err(anyhow::format_err!("you don't have {} parser", other)),
        None => return Err(anyhow::format_err!("missing `--language` argument")),
    };
    // Start executing command
    execute_script(lang, script, &mut source_code)?;
    match matches.occurrences_of("in-place") {
        0 => println!("{}", source_code),
        1 => {
            // TODO in-place write
            let filename = match matches.value_of("FILE") {
                Some(name) => name,
                None => return Err(anyhow::format_err!("[FILE] not exist")),
            };
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(filename)?;
            file.write(source_code.as_bytes())?;
        }
        _ => (),
    }
    Ok(())
}
