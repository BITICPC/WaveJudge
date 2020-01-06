//! This module defines the language provider for the Java programming language.
//!

use std::path::PathBuf;
use std::sync::Once;

use serde::Deserialize;

use crate::InitLanguageError;
use crate::utils::Config;

use judge::{
    Program,
    ProgramKind,
};
use judge::languages::{
    LanguageBranch,
    LanguageProvider,
    LanguageProviderMetadata,
    LanguageProviderRegister,
    CompilationInfo,
    ExecutionInfo,
};

static mut JAVA_METADATA: Option<LanguageProviderMetadata> = None;
static JAVA_METADATA_ONCE: Once = Once::new();

fn init_metadata() {
    JAVA_METADATA_ONCE.call_once(|| {
        let mut metadata = LanguageProviderMetadata::new("java", false);
        metadata.branches.push(LanguageBranch::new("java", "7"));
        metadata.branches.push(LanguageBranch::new("java", "8"));
        metadata.branches.push(LanguageBranch::new("java", "9"));
        metadata.branches.push(LanguageBranch::new("java", "10"));
        metadata.branches.push(LanguageBranch::new("java", "11"));
        metadata.branches.push(LanguageBranch::new("java", "12"));

        unsafe {
            JAVA_METADATA = Some(metadata);
        }
    });
}

/// Get the default compile script of Java source program. This function is used during the
/// deserialization phase of `JavaLanguageConfig` values.
fn get_default_compile_script() -> PathBuf {
    PathBuf::from("./java-compile.py")
}

#[derive(Clone, Debug, Deserialize)]
struct JavaLanguageConfig {
    /// Path to the .jar file of WaveTestLib.
    #[serde(rename = "testlib_jar")]
    testlib_jar: PathBuf,

    /// Path to the compilation script of Java source programs.
    #[serde(rename = "compile_script")]
    #[serde(default = "get_default_compile_script")]
    compile_script: PathBuf,
}

impl Config for JavaLanguageConfig { }

/// Java language provider.
struct JavaLanguageProvider {
    /// The Java language configuration.
    config: JavaLanguageConfig,
}

impl JavaLanguageProvider {
    /// Create a new `JavaLanguageProvider` instance.
    fn new(config: JavaLanguageConfig) -> Self {
        JavaLanguageProvider { config }
    }
}

impl LanguageProvider for JavaLanguageProvider {
    fn metadata(&self) -> &'static LanguageProviderMetadata {
        unsafe { JAVA_METADATA.as_ref().unwrap() }
    }

    fn compile(&self, program: &Program, kind: ProgramKind, output_dir: Option<PathBuf>)
        -> Result<CompilationInfo, Box<dyn std::error::Error>> {
        let mut output_file = crate::utils::make_output_file_path(&program.file, output_dir);
        output_file.set_extension("jar");

        let output_dir = match output_file.parent() {
            Some(d) => d.to_owned(),
            None => PathBuf::from(".")
        };

        let mut ci = CompilationInfo::new(self.config.compile_script.clone(), output_file.clone());
        // The following two arguments are passed to the compiler script to specify the path to the
        // output JAR file. This two arguments should not be passed to the java compiler.
        ci.compiler.args.push(String::from("-o"));
        ci.compiler.args.push(format!("{}", output_file.display()));

        ci.compiler.args.push(String::from("-d"));
        ci.compiler.args.push(format!("{}", output_dir.display()));

        if kind.is_jury() {
            ci.compiler.args.push(String::from("-cp"));
            ci.compiler.args.push(format!("{}", self.config.testlib_jar.display()));
        }

        ci.compiler.args.push(String::from("--release"));
        ci.compiler.args.push(format!("{}", program.language.version()));
        ci.compiler.args.push(String::from("--source"));
        ci.compiler.args.push(format!("{}", program.language.version()));

        ci.compiler.args.push(format!("{}", program.file.display()));

        Ok(ci)
    }

    fn execute(&self, program: &Program, kind: ProgramKind)
        -> Result<ExecutionInfo, Box<dyn std::error::Error>> {
        let mut ei = ExecutionInfo::new("java");

        if kind.is_jury() {
            ei.args.push(String::from("-cp"));
            ei.args.push(format!("{}", self.config.testlib_jar.display()));
        }

        ei.args.push(String::from("-jar"));
        ei.args.push(format!("{}", program.file.display()));

        Ok(ei)
    }
}

/// Name of the Java language configuration file.
const JAVA_LANG_CONFIG_FILE_NAME: &'static str = "config/java-config.yaml";

/// Initialize java language provider.
pub fn init_java_providers(lang: &mut LanguageProviderRegister) -> Result<(), InitLanguageError> {
    init_metadata();

    let config = JavaLanguageConfig::from_file(JAVA_LANG_CONFIG_FILE_NAME)?;

    lang.register(Box::new(JavaLanguageProvider::new(config)));

    Ok(())
}
