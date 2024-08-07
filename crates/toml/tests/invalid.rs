use std::fmt::Write as _;
use std::io::Write as _;

use bumpalo::Bump;
use common::diagnostic;
use common::diagnostic::{ANSII_CLEAR, ANSII_COLOR_BLUE, ANSII_COLOR_YELLOW};
use crates_toml::map::MapInner;
use crates_toml::util::SimpleVal;
use crates_toml::{TomlCtx, TomlDiagnostics};
use libtest_mimic::Failed;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    /// Just fail the tests on mismatch
    #[default]
    Fail,
    /// Skip tests
    Skip,
    /// Skip missing tests
    SkipMissing,
    /// Ask the user on mismatch
    Review,
    /// Ask the user on mismatch, but only on if there is an existing fixture
    Update,
    /// Review correct fixtures
    Revise,
    /// Force overwrite the fixtures on mismatch
    Force,
    /// Force overwrite existing fixtures
    ForceExisting,
}

fn main() {
    let mut mode = Mode::default();
    let mut filter = None;
    let var = std::env::var("SNAPSHOT");
    if let Ok(var) = &var {
        let mut v = var.as_str();
        if let Some((m, f)) = var.split_once(':') {
            v = m;
            if f.is_empty() {
                println!("{ANSII_COLOR_YELLOW}Warning{ANSII_CLEAR}: ignoring empty filter")
            } else {
                filter = Some(f);
            }
        }

        mode = match v {
            "fail" | "" => Mode::Fail,
            "skip" => Mode::Skip,
            "skip-missing" => Mode::SkipMissing,
            "review" => Mode::Review,
            "update" => Mode::Update,
            "revise" => Mode::Revise,
            "force" => Mode::Force,
            "force-existing" => Mode::ForceExisting,
            _ => panic!("invalid mode `{v}`"),
        }
    }

    let mut args = libtest_mimic::Arguments::from_args();
    args.test_threads = Some(1);

    let tests = toml_test_data::invalid()
        .filter(|case| {
            filter
                .map(|f| case.name.to_str().unwrap().contains(f))
                .unwrap_or(true)
        })
        .map(|case| {
            libtest_mimic::Trial::test(case.name.display().to_string(), move || {
                let expect_path =
                    std::path::Path::new("tests/fixtures").join(case.name.with_extension("stderr"));

                let Ok(input) = std::str::from_utf8(case.fixture) else {
                    return Ok(());
                };
                let actual_text = match run_case(input) {
                    Ok(v) => {
                        let msg = format!("Expected error but got:\n{v:?}");
                        return Err(Failed::from(msg));
                    }
                    Err(err) => err,
                };

                let expect_text = match std::fs::read_to_string(&expect_path) {
                    Ok(t) => t,
                    Err(e) => {
                        let mut msg = String::new();
                        _ = writeln!(
                            &mut msg,
                            "Fixture `{ANSII_COLOR_YELLOW}{}{ANSII_CLEAR}` not found:\n  {e}\n",
                            expect_path.display()
                        );
                        _ = writeln!(
                            &mut msg,
                            "=========================  input   ========================="
                        );
                        _ = write!(&mut msg, "{input}");
                        _ = writeln!(
                            &mut msg,
                            "========================= message  ========================="
                        );
                        _ = write!(&mut msg, "{actual_text}");
                        _ = writeln!(
                            &mut msg,
                            "============================================================"
                        );

                        return match mode {
                            Mode::Fail
                            | Mode::SkipMissing
                            | Mode::Revise
                            | Mode::Update
                            | Mode::ForceExisting => Err(Failed::from(msg)),
                            Mode::Skip => unreachable!(),
                            Mode::Review => {
                                print!("\n{msg}");
                                match dialog(["update", "skip", "quit"]) {
                                    "update" => {
                                        let dir = expect_path.parent().unwrap();
                                        std::fs::create_dir_all(dir).unwrap();
                                        std::fs::write(expect_path, actual_text).unwrap();
                                        println!("Added fixture");
                                        Ok(())
                                    }
                                    "skip" => Err(Failed::from(msg)),
                                    "quit" => std::process::exit(0),
                                    _ => unreachable!(),
                                }
                            }
                            Mode::Force => {
                                std::fs::write(expect_path, actual_text).unwrap();
                                Ok(())
                            }
                        };
                    }
                };

                if expect_text == actual_text {
                    if let Mode::Revise = mode {
                        let mut msg = String::new();
                        _ = writeln!(
                            &mut msg,
                            "=========================  input   ========================="
                        );
                        _ = write!(&mut msg, "{input}");
                        _ = writeln!(
                            &mut msg,
                            "========================= message  ========================="
                        );
                        _ = write!(&mut msg, "{actual_text}");
                        _ = writeln!(
                            &mut msg,
                            "============================================================"
                        );

                        print!("\n{msg}");

                        return match dialog(["invalidate", "skip", "quit"]) {
                            "invalidate" => {
                                std::fs::remove_file(expect_path).unwrap();
                                println!("Deleted fixture");
                                Ok(())
                            }
                            "skip" => Ok(()),
                            "quit" => std::process::exit(0),
                            _ => unreachable!(),
                        };
                    }

                    return Ok(());
                }

                if let Mode::Force | Mode::ForceExisting = mode {
                    let dir = expect_path.parent().unwrap();
                    std::fs::create_dir_all(dir).unwrap();
                    std::fs::write(expect_path, actual_text).unwrap();
                    return Ok(());
                }

                let mut msg = String::new();
                _ = writeln!(
                    &mut msg,
                    "=========================  input   ========================="
                );
                _ = write!(&mut msg, "{input}");
                _ = writeln!(
                    &mut msg,
                    "========================= expected ========================="
                );
                _ = write!(&mut msg, "{expect_text}");
                _ = writeln!(
                    &mut msg,
                    "-------------------------  actual  -------------------------"
                );
                _ = write!(&mut msg, "{actual_text}");
                _ = writeln!(
                    &mut msg,
                    "=========================   diff   ========================="
                );
                let comp = pretty_assertions::StrComparison::new(
                    expect_text.as_str(),
                    actual_text.as_str(),
                );
                _ = write!(&mut msg, "{comp}");
                _ = writeln!(
                    &mut msg,
                    "========================= raw diff ========================="
                );
                let comp =
                    pretty_assertions::Comparison::new(expect_text.as_str(), actual_text.as_str());
                _ = write!(&mut msg, "{comp:}");
                _ = writeln!(
                    &mut msg,
                    "============================================================"
                );

                match mode {
                    Mode::Fail | Mode::Revise | Mode::SkipMissing => Err(Failed::from(msg)),
                    Mode::Skip => unreachable!(),
                    Mode::Review | Mode::Update => {
                        print!("\n{msg}");
                        match dialog(["update", "skip", "invalidate", "quit"]) {
                            "update" => {
                                let dir = expect_path.parent().unwrap();
                                std::fs::create_dir_all(dir).unwrap();
                                std::fs::write(expect_path, actual_text).unwrap();
                                println!("Updated fixture");
                                Ok(())
                            }
                            "invalidate" => {
                                std::fs::remove_file(expect_path).unwrap();
                                println!("Deleted fixture");
                                Ok(())
                            }
                            "skip" => Err(Failed::from(msg)),
                            "quit" => std::process::exit(0),
                            _ => unreachable!(),
                        }
                    }
                    Mode::Force | Mode::ForceExisting => unreachable!(),
                }
            })
            .with_ignored_flag(match mode {
                Mode::Skip => true,
                Mode::SkipMissing => {
                    let expect_path = std::path::Path::new("tests/fixtures")
                        .join(case.name.with_extension("stderr"));
                    !expect_path.exists()
                }
                _ => false,
            })
        })
        .collect();
    libtest_mimic::run(&args, tests).exit()
}

fn run_case(input: &str) -> Result<MapInner<String, SimpleVal>, String> {
    let mut ctx = TomlDiagnostics::default();
    let bump = Bump::new();
    let tokens = ctx.lex(&bump, input);
    let asts = ctx.parse(&bump, &tokens);
    let map = ctx.map(&asts);

    if !ctx.errors.is_empty() {
        ctx.sort_diagnostics();
        let lines = diagnostic::lines(input);
        let mut msg = String::new();
        for error in ctx.errors.iter() {
            _ = diagnostic::display(&mut msg, error, &lines);
        }
        return Err(msg);
    }

    Ok(crates_toml::util::map_simple(map))
}

fn dialog<const SIZE: usize>(options: [&str; SIZE]) -> &str {
    for o in options {
        // HACK: only works for ascii
        let (first, remainder) = o.split_at(1);
        println!("{ANSII_COLOR_BLUE}[{first}]{ANSII_CLEAR}{remainder}");
    }

    loop {
        print!("> ");
        _ = std::io::stdout().flush();

        let stdin = std::io::stdin();
        let mut input = String::new();
        _ = stdin.read_line(&mut input);
        let len = input.trim_end().len();
        input.truncate(len);

        for o in options {
            if input.len() == 1 && input[..1] == o[..1] {
                return o;
            }
            if input == o {
                return o;
            }
        }

        println!("Invalid input {input:?}");
    }
}
