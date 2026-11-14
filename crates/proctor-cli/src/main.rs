use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use proctor_core::{Policy, Verdict};
use proctor_diff::SemanticDiff;
use proctor_rules::evaluate_rules;
use std::collections::HashSet;
use std::path::PathBuf;

mod git;
mod reporter;
mod sarif;

#[derive(Parser)]
#[command(
    name = "proctor",
    author = "The Proctor Team",
    version = "0.1.0",
    about = "Deterministic PR integrity and anticheat gate for autonomous AI coding agents",
    long_about = "proctor prevents specification gaming (assertion demolition, shadow mocking, and CI workflow sabotage) by autonomous coding agents like Devin, Claude Code, and Cursor."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, global = true, default_value = "main")]
    base: String,

    #[arg(long, global = true)]
    head: Option<String>,

    #[arg(short, long, global = true, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,

    #[arg(long, global = true)]
    sarif: Option<PathBuf>,

    #[arg(long, global = true, value_delimiter = ',')]
    waivers: Vec<String>,

    #[arg(long, global = true)]
    strict: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan git changes between base ref and head ref/working tree for integrity violations
    Scan {
        #[arg(short, long, default_value = "main")]
        base: String,

        #[arg(long)]
        head: Option<String>,

        #[arg(short, long, value_enum, default_value_t = OutputFormat::Pretty)]
        format: OutputFormat,

        #[arg(long)]
        sarif: Option<PathBuf>,

        #[arg(long, value_delimiter = ',')]
        waivers: Vec<String>,

        #[arg(long)]
        strict: bool,
    },
    /// Print version and supported language parsers
    Version,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
enum OutputFormat {
    Pretty,
    Json,
    Sarif,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Version) => {
            println!("agent-proctor v0.1.0 (deterministic AST verification)");
            println!("Supported languages: Python (pytest/unittest), JavaScript/TypeScript (Jest/Vitest)");
            println!("Engine: Tree-sitter 0.21, Blake3 cryptographic fingerprints");
            Ok(())
        }
        Some(Commands::Scan {
            base,
            head,
            format,
            sarif,
            waivers,
            strict,
        }) => run_scan(&base, head.as_deref(), format, sarif, &waivers, strict),
        None => run_scan(
            &cli.base,
            cli.head.as_deref(),
            cli.format,
            cli.sarif,
            &cli.waivers,
            cli.strict,
        ),
    }
}

fn run_scan(
    base: &str,
    head: Option<&str>,
    format: OutputFormat,
    sarif_path: Option<PathBuf>,
    waiver_vec: &[String],
    strict: bool,
) -> Result<()> {
    // 1. Get changed files from git
    let changed = match git::get_changed_files(base, head) {
        Ok(c) => c,
        Err(e) => {
            // Fallback: try origin/<base> or HEAD~1
            if base == "main" {
                if let Ok(c) = git::get_changed_files("origin/main", head) {
                    c
                } else if let Ok(c) = git::get_changed_files("HEAD~1", head) {
                    c
                } else {
                    eprintln!("proctor: warning: unable to diff git refs: {}", e);
                    std::process::exit(0);
                }
            } else {
                eprintln!("proctor: error: {}", e);
                std::process::exit(1);
            }
        }
    };

    if changed.head_files.is_empty() && changed.base_files.is_empty() {
        println!("proctor: No changes detected between {} and HEAD.", base);
        std::process::exit(0);
    }

    // 2. Build semantic diff
    let diff = SemanticDiff::build(&changed.base_files, &changed.head_files)
        .map_err(|e| anyhow::anyhow!("Semantic diff failed: {}", e))?;

    // 3. Configure policy & waivers
    let mut policy = Policy::default();
    policy.strict_mode = strict;

    let accepted_waivers: HashSet<String> = waiver_vec.iter().cloned().collect();

    // 4. Evaluate rules
    let findings = evaluate_rules(&diff, &policy, &accepted_waivers);

    // 5. Generate SARIF if requested
    if let Some(path) = sarif_path {
        let sarif_val = sarif::generate_sarif(&findings);
        let sarif_str = serde_json::to_string_pretty(&sarif_val)?;
        std::fs::write(&path, sarif_str)?;
    }

    // 6. Output findings based on requested format
    match format {
        OutputFormat::Pretty => {
            reporter::print_findings(&findings, base);
        }
        OutputFormat::Json => {
            let json_str = serde_json::to_string_pretty(&findings)?;
            println!("{}", json_str);
        }
        OutputFormat::Sarif => {
            let sarif_val = sarif::generate_sarif(&findings);
            println!("{}", serde_json::to_string_pretty(&sarif_val)?);
        }
    }

    // 7. Compute exit code
    let has_block = findings
        .iter()
        .any(|f| matches!(f.verdict, Verdict::Block { .. }));
    let has_review = findings
        .iter()
        .any(|f| matches!(f.verdict, Verdict::Review { .. }));

    if has_block || (strict && has_review) {
        std::process::exit(2);
    } else if has_review {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}
