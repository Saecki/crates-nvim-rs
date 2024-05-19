use std::process::ExitCode;

use bumpalo::Bump;
use common::diagnostic;
use toml::diagnostic::display_error;
use toml::{TomlCtx, TomlDiagnostics};

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Missing argument <file>");
        return ExitCode::FAILURE;
    };

    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        _ => {
            eprintln!("Error reading from file");
            return ExitCode::FAILURE;
        }
    };
    let lines = diagnostic::lines(&text);

    let start = std::time::SystemTime::now();
    let mut ctx = TomlDiagnostics::default();
    let bump = Bump::new();
    let tokens = ctx.lex(&bump, &text);
    let lexing = std::time::SystemTime::now();
    let asts = ctx.parse(&bump, &tokens);
    let parsing = std::time::SystemTime::now();
    let map = ctx.map(&asts);
    let mapping = std::time::SystemTime::now();
    let simple = toml::map::simple::map_table(map);
    let end = std::time::SystemTime::now();

    println!("{:#?}", simple);
    ctx.sort_diagnostics();
    let mut msg = String::new();
    for error in ctx.errors.iter() {
        display_error(&mut msg, error, &lines).unwrap();
        println!("{msg}");
        msg.clear()
    }

    let us_lexing = lexing.duration_since(start).unwrap().as_micros();
    let us_parsing = parsing.duration_since(lexing).unwrap().as_micros();
    let us_mapping = mapping.duration_since(parsing).unwrap().as_micros();
    let us_simple = end.duration_since(mapping).unwrap().as_micros();
    let us_total = end.duration_since(start).unwrap().as_micros();

    {
        println!("lexing {}us", us_lexing);
        println!("parsing {}us", us_parsing);
        println!("mapping {}us", us_mapping);
        println!("simple {}us", us_simple);
        println!("total {}us", us_total);
    };

    ExitCode::SUCCESS
}