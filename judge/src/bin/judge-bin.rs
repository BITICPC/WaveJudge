extern crate error_chain;
extern crate stderrlog;
extern crate clap;
extern crate judge;

use std::path::PathBuf;
use std::str::FromStr;

use error_chain::ChainedError;

use judge::{
    Program,
    ProgramKind,
    CompilationTaskDescriptor,
};
use judge::engine::{
    JudgeEngine,
    JudgeEngineConfig,
};
use judge::languages::{
    LanguageIdentifier,
    LanguageBranch,
};

error_chain::error_chain! {
    types {
        Error, ErrorKind, ResultExt, Result;
    }

    links {
        JudgeError(::judge::Error, ::judge::ErrorKind);
        DylibLoaderError(
            ::judge::languages::LoadDylibError, ::judge::languages::LoadDylibErrorKind);
    }

    errors {
        InvalidLanguageIdentifier {
            description("invalid language identifier")
        }
    }
}

fn get_arg_matches() -> clap::ArgMatches<'static> {
    clap::App::new("judge-bin")
        .version("0.1.0")
        .author("Lancern <msrlancern@126.com>")
        .about("A wrapper program for executing wave judge crate in CLI environment.")
        .setting(clap::AppSettings::SubcommandRequiredElseHelp)
        .arg(clap::Arg::with_name("lang_so")
            .long("load")
            .multiple(true)
            .takes_value(true)
            .value_name("LANGUAGE_PROVIDER_SOs")
            .global(true)
            .help("path to dynamic linking libraries containing language provider definitions"))
        .subcommand(clap::SubCommand::with_name("compile")
            .version("0.1.0")
            .author("Lancern <msrlancern@126.com>")
            .about("Compile a program")
            .arg(clap::Arg::with_name("lang")
                .short("l")
                .long("lang")
                .required(true)
                .multiple(false)
                .takes_value(true)
                .value_name("LANGUAGE")
                .help("language of the source program to be compiled"))
            .arg(clap::Arg::with_name("kind")
                .long("kind")
                .required(false)
                .multiple(false)
                .takes_value(true)
                .value_name("SCHEME")
                .possible_values(&["JUDGEE", "CHECKER", "INTERACTOR"])
                .default_value("JUDGEE")
                .help("program kind"))
            .arg(clap::Arg::with_name("output")
                .short("o")
                .long("output")
                .multiple(false)
                .takes_value(true)
                .value_name("OUTPUT_DIR")
                .help("output directory of the compiler"))
            .arg(clap::Arg::with_name("program")
                .required(true)
                .multiple(false)
                .takes_value(true)
                .value_name("SOURCE_FILE")
                .help("source file of the program to be compiled")))
        .subcommand(clap::SubCommand::with_name("judge")
            .version("0.1.0")
            .author("Lancern <msrlancern@126.com>")
            .about("Judge a program")
            .arg(clap::Arg::with_name("lang")
                .short("l")
                .long("lang")
                .required(true)
                .multiple(false)
                .takes_value(true)
                .value_name("LANGUAGE")
                .help("language of the program to be judged"))
            .arg(clap::Arg::with_name("mode")
                .long("mode")
                .multiple(false)
                .takes_value(true)
                .value_name("JUDGE_MODE")
                .default_value("STANDARD")
                .possible_values(&["STANDARD", "SPECIAL_JUDGE", "INTERACTIVE"])
                .help("judge mode"))
            .arg(clap::Arg::with_name("cpu_time_limit")
                .short("t")
                .long("cpu")
                .multiple(false)
                .takes_value(true)
                .value_name("CPU_TIME_LIMIT")
                .default_value("1000")
                .help("CPU time limit, in milliseconds"))
            .arg(clap::Arg::with_name("real_time_limit")
                .short("r")
                .long("real")
                .multiple(false)
                .takes_value(true)
                .value_name("REAL_TIME_LIMIT")
                .default_value("3000")
                .help("real time limit, in milliseconds"))
            .arg(clap::Arg::with_name("memory_limit")
                .short("m")
                .long("memory")
                .multiple(false)
                .takes_value(true)
                .value_name("MEMORY_LIMIT")
                .default_value("256")
                .help("memory limit, in megabytes"))
            .arg(clap::Arg::with_name("uid")
                .short("u")
                .long("uid")
                .multiple(false)
                .takes_value(true)
                .value_name("EFFECTIVE_USER_ID")
                .help("effective user ID used by the judge"))
            .arg(clap::Arg::with_name("allowed_syscalls")
                .long("syscall")
                .multiple(true)
                .takes_value(true)
                .value_name("ALLOWED_SYSCALLS")
                .value_terminator("--")
                .help("allowed system call names of the judgee"))
            .arg(clap::Arg::with_name("checker")
                .long("checker")
                .required_if("mode", "SPECIAL_JUDGE")
                .multiple(false)
                .takes_value(true)
                .value_name("CHECKER")
                .help("path to the answer checker program"))
            .arg(clap::Arg::with_name("checker_cpu_time_limit")
                .long("checker-cpu")
                .multiple(false)
                .takes_value(true)
                .value_name("CHECKER_CPU_TIME_LIMIT")
                .help("CPU time limit of the checker"))
            .arg(clap::Arg::with_name("checker_real_time_limit")
                .long("checker-real")
                .multiple(false)
                .takes_value(true)
                .value_name("CHECKER_REAL_TIME_LIMIT")
                .help("real time limit of the checker"))
            .arg(clap::Arg::with_name("checker_memory_limit")
                .long("checker-memory")
                .multiple(false)
                .takes_value(true)
                .value_name("CHECKER_MEMORY_LIMIT")
                .help("memory limit of the checker"))
            .arg(clap::Arg::with_name("interactor")
                .long("interactor")
                .required_if("mode", "INTERACTIVE")
                .multiple(false)
                .takes_value(true)
                .value_name("INTERACTOR")
                .help("path to the interactor program"))
            .arg(clap::Arg::with_name("interactor_cpu_time_limit")
                .long("interactor-cpu")
                .multiple(false)
                .takes_value(true)
                .value_name("INTERACTOR_CPU_TIME_LIMIT")
                .help("CPU time limit of the interactor"))
            .arg(clap::Arg::with_name("interactor_real_time_limit")
                .long("interactor-real")
                .multiple(false)
                .takes_value(true)
                .value_name("INTERACTOR_REAL_TIME_LIMIT")
                .help("real time limit of the interactor"))
            .arg(clap::Arg::with_name("interactor_memory_limit")
                .long("interactor-memory")
                .multiple(false)
                .takes_value(true)
                .value_name("INTERACTOR_MEMORY_LIMIT")
                .help("memory limit of the interactor"))
            .arg(clap::Arg::with_name("test_suite")
                .long("tc")
                .required(true)
                .multiple(true)
                .takes_value(true)
                .value_name("TEST_SUITE")
                .help(concat!(
                    "test suite to judge against, specified as pairs of input / answer files ",
                    "separated by colon(:), e.g.: /path/to/input:/path/to/answer")))
            .arg(clap::Arg::with_name("program")
                .required(true)
                .multiple(false)
                .takes_value(true)
                .value_name("PROGRAM")
                .help("path to the program executable file to be judged")))
        .get_matches()
}

fn parse_lang(lang: &str) -> Result<LanguageIdentifier> {
    let lang_parts = lang.split(':').collect::<Vec<&'_ str>>();
    if lang_parts.len() != 3 {
        return Err(Error::from(ErrorKind::InvalidLanguageIdentifier));
    }

    Ok(LanguageIdentifier::new(lang_parts[0], LanguageBranch::new(lang_parts[1], lang_parts[2])))
}

fn do_compile(matches: &clap::ArgMatches<'_>, engine: &mut JudgeEngine) -> Result<()> {
    let file = matches.value_of("program").unwrap();
    let lang = parse_lang(matches.value_of("lang").unwrap())?;
    let prog = Program::new(file, lang);

    let mut task = CompilationTaskDescriptor::new(prog);
    task.kind = match matches.value_of("kind").unwrap() {
        "JUDGEE" => ProgramKind::Judgee,
        "CHECKER" => ProgramKind::Checker,
        "INTERACTOR" => ProgramKind::Interactor,
        _ => unreachable!()
    };
    task.output_dir = matches.value_of("output").map(PathBuf::from);

    let res = engine.compile(task).chain_err(|| Error::from("Compilation failed"))?;

    println!("Compilation succeeded? {}", res.succeeded);
    if res.succeeded {
        let output_file = res.output_file
            .expect("failed to get output file name of compilation task");
        println!("Output file: {}", output_file.display())
    } else {
        println!("Compilation error.");
        let message = res.compiler_out.expect("failed to get compiler output.");
        println!("{}", message);
    }

    Ok(())
}

fn do_judge(matches: &clap::ArgMatches<'_>, engine: &mut JudgeEngine) -> Result<()> {
    unimplemented!()
}

fn do_main() -> Result<()> {
    stderrlog::new()
        .quiet(false)
        .verbosity(5)
        .init()
        .unwrap();
    let matches = get_arg_matches();

    // Load dynamic linking libraries that contains definitions for language proviers, if any.
    let mut engine = JudgeEngine::new();
    match matches.values_of("lang_so") {
        Some(sos) => {
            for so in sos {
                let so_path = PathBuf::from_str(so).unwrap();
                engine.languages().load_dylib(&so_path)?;
            }
        },
        None => ()
    };

    let lang = engine.languages().languages();
    log::debug!("All registered languages: {:?}", lang);

    match matches.subcommand() {
        ("compile", Some(compile_matches)) => {
            do_compile(compile_matches, &mut engine)?;
        },
        ("judge", Some(judge_matches)) => {
            do_judge(judge_matches, &mut engine)?;
        },
        _ => unreachable!()
    };

    Ok(())
}

fn main() {
    match do_main() {
        Ok(()) => (),
        Err(e) => {
            eprintln!("error: {}", e.display_chain().to_string());
            std::process::exit(1);
        }
    }
}
