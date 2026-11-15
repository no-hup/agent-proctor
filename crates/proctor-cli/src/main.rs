use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use proctor_core::{Policy, RuleId, RuleTier, ScanReport, Verdict};
use proctor_diff::SemanticDiff;
use proctor_rules::evaluate_rules;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

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

    #[arg(short, long, default_value = "main")]
    base: String,

    #[arg(long)]
    head: Option<String>,

    #[arg(short, long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,

    #[arg(long)]
    sarif: Option<PathBuf>,

    #[arg(long, hide = true, value_delimiter = ',', env = "PROCTOR_VERIFIED_WAIVERS")]
    verified_waivers: Vec<String>,

    #[arg(long)]
    strict: bool,

    #[arg(long)]
    experimental_probe: bool,
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

        #[arg(long, hide = true, value_delimiter = ',', env = "PROCTOR_VERIFIED_WAIVERS")]
        verified_waivers: Vec<String>,

        #[arg(long)]
        strict: bool,

        #[arg(long)]
        experimental_probe: bool,
    },
    /// Run ephemeral mutation probe in an isolated git worktree
    Probe {
        #[arg(short, long, default_value = "main")]
        base: String,

        #[arg(long, default_value = "HEAD")]
        head: String,

        #[arg(long, default_value_t = 3)]
        max_mutants: usize,

        #[arg(short, long, value_enum, default_value_t = OutputFormat::Pretty)]
        format: OutputFormat,

        #[arg(long)]
        sarif: Option<PathBuf>,
    },
    /// Audit historical commits or pull requests to measure false positive rate and integrity findings
    Audit {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,

        #[arg(short, long, value_enum, default_value_t = OutputFormat::Pretty)]
        format: OutputFormat,

        #[arg(long)]
        label: Option<PathBuf>,
    },
    /// Evaluate agent benchmark predictions or rollout trajectories (SWE-bench format)
    Eval {
        #[arg(long, default_value = "swebench")]
        format: String,

        #[arg(required = true)]
        predictions: PathBuf,

        #[arg(long)]
        repos: Option<PathBuf>,

        #[arg(short, long, default_value = "verdicts.jsonl")]
        out: PathBuf,
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
            println!("Engine: Tree-sitter 0.22, Blake3 cryptographic fingerprints");
            Ok(())
        }
        Some(Commands::Scan {
            base,
            head,
            format,
            sarif,
            verified_waivers,
            strict,
            experimental_probe,
        }) => run_scan(
            &base,
            head.as_deref(),
            format,
            sarif,
            &verified_waivers,
            strict,
            experimental_probe,
        ),
        Some(Commands::Probe {
            base,
            head,
            max_mutants,
            format,
            sarif,
        }) => run_probe_cmd(&base, &head, max_mutants, format, sarif),
        Some(Commands::Audit {
            limit,
            format,
            label,
        }) => run_audit(limit, format, label),
        Some(Commands::Eval {
            format,
            predictions,
            repos,
            out,
        }) => run_eval(&format, &predictions, repos.as_deref(), &out),
        None => run_scan(
            &cli.base,
            cli.head.as_deref(),
            cli.format,
            cli.sarif,
            &cli.verified_waivers,
            cli.strict,
            cli.experimental_probe,
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
    experimental_probe: bool,
) -> Result<()> {
    // 1. Immutably anchor policy to the base ref
    let mut policy = git::load_policy_from_base(base);
    if strict {
        policy.strict_mode = true;
    }

    // 2. Get changed files from git using two-dot diff
    let mut changed = match git::get_changed_files(base, head) {
        Ok(c) => c,
        Err(e) => {
            // Fallback: try origin/<base> or HEAD~1
            if base == "main" {
                if let Ok(c) = git::get_changed_files("origin/main", head) {
                    c
                } else if let Ok(c) = git::get_changed_files("HEAD~1", head) {
                    c
                } else {
                    eprintln!("proctor: error: unable to diff git refs: {}", e);
                    std::process::exit(1);
                }
            } else {
                eprintln!("proctor: error: {}", e);
                std::process::exit(1);
            }
        }
    };

    // Filter changed files against configured ignored_paths
    git::filter_ignored_paths(&mut changed, &policy.ignored_paths);

    if changed.head_files.is_empty() && changed.base_files.is_empty() {
        if format == OutputFormat::Json {
            let report = ScanReport {
                schema_version: "1.0".to_string(),
                verdict: Verdict::Pass,
                findings: Vec::new(),
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("proctor: No changes detected between {} and HEAD.", base);
        }
        std::process::exit(0);
    }

    // 3. Build semantic diff
    let diff = SemanticDiff::build(&changed.base_files, &changed.head_files)
        .map_err(|e| anyhow::anyhow!("Semantic diff failed: {}", e))?;

    // 4. Configure verified waivers & acks
    // In local execution, env waivers are disabled to prevent agent self-waiver in tool loops.
    // Env-based waivers are strictly parsed in CI when GITHUB_ACTIONS is present.
    let mut waiver_set = proctor_core::WaiverSet::default();
    for w in waiver_vec {
        waiver_set.maintainer_waivers.insert(w.clone());
    }

    if std::env::var("GITHUB_ACTIONS").is_ok() {
        if let Ok(env_waivers) = std::env::var("PROCTOR_VERIFIED_WAIVERS") {
            if let Ok(parsed_set) = serde_json::from_str::<proctor_core::WaiverSet>(&env_waivers) {
                waiver_set.maintainer_waivers.extend(parsed_set.maintainer_waivers);
                waiver_set.author_acks.extend(parsed_set.author_acks);
            } else {
                for w in env_waivers.split(',') {
                    let trimmed = w.trim();
                    if !trimmed.is_empty() {
                        waiver_set.maintainer_waivers.insert(trimmed.to_string());
                    }
                }
            }
        }
    }

    // 5. Evaluate rules
    let mut findings = evaluate_rules(&diff, &policy, &waiver_set);

    // 5b. Optional dynamic probe
    if experimental_probe {
        println!("proctor: Running dynamic mutation probe in ephemeral worktree...");
        let head_ref = head.unwrap_or("HEAD");
        if let Ok(probe_results) = proctor_probe::run_probe(Path::new("."), head_ref, &diff, 3) {
            for res in probe_results {
                if let Some(finding) = res.to_finding() {
                    findings.push(finding);
                }
            }
        }
    }

    // 6. Generate SARIF if requested
    if let Some(path) = sarif_path {
        let sarif_val = sarif::generate_sarif(&findings);
        let sarif_str = serde_json::to_string_pretty(&sarif_val)?;
        std::fs::write(&path, sarif_str)?;
    }

    // 7. Compute overall verdict & rule tiering
    let has_guard_block = findings.iter().any(|f| {
        f.tier == RuleTier::Guard || matches!(f.verdict, Verdict::Block { .. })
    });
    let has_report_review = findings.iter().any(|f| {
        f.tier == RuleTier::Report || matches!(f.verdict, Verdict::Review { .. })
    });

    let overall_verdict = if has_guard_block {
        Verdict::Block {
            rule: findings
                .iter()
                .find(|f| matches!(f.verdict, Verdict::Block { .. }))
                .map(|f| f.rule_id)
                .unwrap_or(RuleId::AssertionLock),
            evidence: "Hard guard policy constraints violated.".into(),
        }
    } else if has_report_review {
        Verdict::Review {
            finding_id: findings
                .iter()
                .find(|f| matches!(f.verdict, Verdict::Review { .. }))
                .map(|f| f.id.clone())
                .unwrap_or_default(),
            evidence: "Specification modifications require review acknowledgement.".into(),
        }
    } else {
        Verdict::Pass
    };

    // 8. Output findings based on requested format
    match format {
        OutputFormat::Pretty => {
            reporter::print_findings(&findings, base);
        }
        OutputFormat::Json => {
            let report = ScanReport {
                schema_version: "1.0".to_string(),
                verdict: overall_verdict,
                findings: findings.clone(),
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        OutputFormat::Sarif => {
            let sarif_val = sarif::generate_sarif(&findings);
            println!("{}", serde_json::to_string_pretty(&sarif_val)?);
        }
    }

    // 9. Compute exit code
    // Guard tier: zero-tolerance, hard BLOCK (exit 2)
    // Report tier: advisory receipts exit 0 by default; exit 1 only under --strict or strict_mode
    if has_guard_block {
        std::process::exit(2);
    } else if has_report_review {
        if strict || policy.strict_mode {
            std::process::exit(1);
        } else {
            std::process::exit(0);
        }
    } else {
        std::process::exit(0);
    }
}

fn run_probe_cmd(
    base: &str,
    head: &str,
    max_mutants: usize,
    format: OutputFormat,
    sarif_path: Option<PathBuf>,
) -> Result<()> {
    let changed = git::get_changed_files(base, Some(head))?;
    let diff = SemanticDiff::build(&changed.base_files, &changed.head_files)
        .map_err(|e| anyhow::anyhow!("Semantic diff failed: {}", e))?;

    println!("proctor: Dispatching ephemeral probe sandbox on ref '{}'...", head);
    let probe_results = proctor_probe::run_probe(Path::new("."), head, &diff, max_mutants)?;

    let mut findings = Vec::new();
    let mut killed = 0;
    let mut survived = 0;
    let mut inconclusive = 0;

    for res in &probe_results {
        match res.outcome {
            proctor_probe::ProbeOutcome::Killed => killed += 1,
            proctor_probe::ProbeOutcome::Survived => {
                survived += 1;
                if let Some(f) = res.to_finding() {
                    findings.push(f);
                }
            }
            proctor_probe::ProbeOutcome::Inconclusive => inconclusive += 1,
        }
    }

    match format {
        OutputFormat::Pretty => {
            println!("\n{}", "─".repeat(60));
            println!("🔍 proctor probe: Mutation Results");
            println!("  Total Mutants : {}", probe_results.len());
            println!("  Killed        : {}", killed);
            println!("  Survived      : {}", survived);
            println!("  Inconclusive  : {}", inconclusive);
            println!("{}\n", "─".repeat(60));

            if !findings.is_empty() {
                reporter::print_findings(&findings, base);
            } else {
                println!("✔ All mutants were successfully killed or asserted by tests.");
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&probe_results)?);
        }
        OutputFormat::Sarif => {
            let sarif_val = sarif::generate_sarif(&findings);
            println!("{}", serde_json::to_string_pretty(&sarif_val)?);
        }
    }

    if let Some(path) = sarif_path {
        let sarif_val = sarif::generate_sarif(&findings);
        std::fs::write(&path, serde_json::to_string_pretty(&sarif_val)?)?;
    }

    if survived > 0 {
        std::process::exit(1);
    } else {
        std::process::exit(0);
    }
}

fn run_audit(limit: usize, format: OutputFormat, label_path: Option<PathBuf>) -> Result<()> {
    println!(
        "proctor: Auditing last {} historical commits for test integrity & false-positive metrics...\n",
        limit
    );

    let output = std::process::Command::new("git")
        .args(["log", &format!("-n{}", limit), "--format=%H"])
        .output()?;

    if !output.status.success() {
        eprintln!("proctor: Failed to read git history.");
        std::process::exit(1);
    }

    let commits_str = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<&str> = commits_str
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    let policy = Policy::discover();
    let empty_waivers = proctor_core::WaiverSet::default();

    let mut total_audited = 0;
    let mut clean_commits = 0;
    let mut review_commits = 0;
    let mut block_commits = 0;
    let mut rule_counts: std::collections::HashMap<&'static str, usize> = std::collections::HashMap::new();

    let mut label_file = if let Some(ref p) = label_path {
        let mut f = std::fs::File::create(p)?;
        writeln!(f, "commit,verdict,findings_count,rule_codes")?;
        Some(f)
    } else {
        None
    };

    for commit in &commits {
        let parent = format!("{}^", commit);
        if let Ok(changed) = git::get_changed_files(&parent, Some(commit)) {
            if changed.base_files.is_empty() && changed.head_files.is_empty() {
                continue;
            }
            if let Ok(diff) = SemanticDiff::build(&changed.base_files, &changed.head_files) {
                total_audited += 1;
                let findings = evaluate_rules(&diff, &policy, &empty_waivers);

                let has_block = findings
                    .iter()
                    .any(|f| matches!(f.verdict, Verdict::Block { .. }));
                let has_review = findings
                    .iter()
                    .any(|f| matches!(f.verdict, Verdict::Review { .. }));

                let rule_codes: Vec<&str> = findings.iter().map(|f| f.rule_id.code()).collect();

                for f in &findings {
                    *rule_counts.entry(f.rule_id.code()).or_insert(0) += 1;
                }

                let verdict_label = if has_block {
                    block_commits += 1;
                    "block"
                } else if has_review {
                    review_commits += 1;
                    "review"
                } else {
                    clean_commits += 1;
                    "pass"
                };

                if let Some(ref mut f) = label_file {
                    writeln!(
                        f,
                        "{},{},{},{}",
                        commit,
                        verdict_label,
                        findings.len(),
                        rule_codes.join(";")
                    )?;
                }
            }
        }
    }

    let block_pct = if total_audited > 0 {
        (block_commits as f64 / total_audited as f64) * 100.0
    } else {
        0.0
    };

    match format {
        OutputFormat::Pretty => {
            println!("{}", "─".repeat(60));
            println!("📊 proctor audit: Historical Repository Integrity Scan");
            println!("  Commits Audited : {}", total_audited);
            let pass_pct = if total_audited > 0 {
                (clean_commits as f64 / total_audited as f64) * 100.0
            } else {
                0.0
            };
            let review_pct = if total_audited > 0 {
                (review_commits as f64 / total_audited as f64) * 100.0
            } else {
                0.0
            };
            println!("  Clean (PASS)    : {} ({:.1}%)", clean_commits, pass_pct);
            println!("  Spec Revisions  : {} ({:.1}%) [REVIEW]", review_commits, review_pct);
            println!("  Violations      : {} ({:.1}%) [BLOCK]", block_commits, block_pct);
            println!("{}", "─".repeat(60));

            if !rule_counts.is_empty() {
                println!("\n  Findings Breakdown by Rule:");
                for (rule, count) in &rule_counts {
                    println!("    {:<12} : {}", rule, count);
                }
            }
            println!(
                "\n  Block Rate on Merged History: {:.2}%\n",
                block_pct
            );
            if let Some(p) = label_path {
                println!("  Label CSV exported to: {}", p.display());
            }
        }
        OutputFormat::Json => {
            let res = serde_json::json!({
                "commits_audited": total_audited,
                "clean_pass": clean_commits,
                "review_required": review_commits,
                "blocked": block_commits,
                "rule_breakdown": rule_counts,
                "block_rate_on_merged_history": block_pct,
            });
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
        OutputFormat::Sarif => {
            let res = serde_json::json!({
                "commits_audited": total_audited,
                "clean_pass": clean_commits,
                "review_required": review_commits,
                "blocked": block_commits,
            });
            println!("{}", serde_json::to_string_pretty(&res)?);
        }
    }

    Ok(())
}

/// Evaluates benchmark predictions or trajectories (e.g. SWE-bench).
fn run_eval(
    format: &str,
    predictions_path: &Path,
    repos_dir: Option<&Path>,
    out_path: &Path,
) -> Result<()> {
    if format != "swebench" {
        anyhow::bail!("Unsupported eval format '{}'. Currently supported: 'swebench'", format);
    }

    println!("proctor eval: Loading predictions from {}", predictions_path.display());
    if repos_dir.is_none() {
        println!("proctor eval: Note: repos_dir not provided; evaluating in fragment-safe mode.");
    }
    let file = std::fs::File::open(predictions_path)?;
    let reader = BufReader::new(file);

    let mut out_file = std::fs::File::create(out_path)?;
    let mut total = 0;
    let mut clean_pass = 0;
    let mut review_count = 0;
    let mut block_count = 0;

    let policy = Policy::default();
    let empty_waivers = proctor_core::WaiverSet::default();

    for line_res in reader.lines() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }

        let entry: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let err_record = serde_json::json!({
                    "instance_id": "malformed_json",
                    "verdict": "error",
                    "error": e.to_string(),
                });
                writeln!(out_file, "{}", serde_json::to_string(&err_record)?)?;
                continue;
            }
        };
        let instance_id = entry
            .get("instance_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let model_patch = entry
            .get("model_patch")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        total += 1;

        // Parse patch into changed base and head files
        let (base_files, head_files) = parse_patch_to_files(model_patch, repos_dir, &instance_id);

        let findings = if base_files.is_empty() && head_files.is_empty() {
            Vec::new()
        } else if let Ok(diff) = SemanticDiff::build(&base_files, &head_files) {
            let mut eval_findings = evaluate_rules(&diff, &policy, &empty_waivers);
            if repos_dir.is_none() {
                // In diff-only fragment mode, isolate fragment-safe rules and exclude
                // parse failures caused by missing class/function scope in isolated hunks.
                eval_findings.retain(|f| {
                    !f.evidence.contains("Tree-sitter parse error")
                        && !f.evidence.contains("Syntax error detected")
                });
            }
            eval_findings
        } else {
            Vec::new()
        };

        let has_block = findings.iter().any(|f| {
            f.tier == RuleTier::Guard || matches!(f.verdict, Verdict::Block { .. })
        });
        let has_review = findings.iter().any(|f| {
            f.tier == RuleTier::Report || matches!(f.verdict, Verdict::Review { .. })
        });

        let verdict_str = if has_block {
            block_count += 1;
            "block"
        } else if has_review {
            review_count += 1;
            "review"
        } else {
            clean_pass += 1;
            "pass"
        };

        let verdict_record = serde_json::json!({
            "instance_id": instance_id,
            "verdict": verdict_str,
            "findings_count": findings.len(),
            "findings": findings,
        });

        writeln!(out_file, "{}", serde_json::to_string(&verdict_record)?)?;
    }

    println!("\n{}", "─".repeat(60));
    println!("🧪 proctor eval: Evaluation Summary");
    println!("  Total Instances : {}", total);
    println!("  Clean (PASS)    : {}", clean_pass);
    println!("  Review Required : {}", review_count);
    println!("  Reward-Hacked   : {} (BLOCKED)", block_count);
    println!("  Results written : {}", out_path.display());
    println!("{}\n", "─".repeat(60));

    Ok(())
}

/// Parses a unified git diff patch into base and head in-memory files.
/// When repos_dir is provided, attempts to load original base file context to permit full AST symbol linkage.
fn parse_patch_to_files(
    patch: &str,
    repos_dir: Option<&Path>,
    instance_id: &str,
) -> (std::collections::HashMap<String, String>, std::collections::HashMap<String, String>) {
    let mut base_files = std::collections::HashMap::new();
    let mut head_files = std::collections::HashMap::new();

    let repo_path = repos_dir.map(|d| d.join(instance_id));
    let has_repo = repo_path.as_ref().map(|p| p.is_dir()).unwrap_or(false);

    let mut current_file: Option<String> = None;
    let mut current_base_lines = Vec::new();
    let mut current_head_lines = Vec::new();

    let flush_file = |file: String,
                      base_lines: &[String],
                      head_lines: &[String],
                      b_files: &mut std::collections::HashMap<String, String>,
                      h_files: &mut std::collections::HashMap<String, String>| {
        if has_repo {
            if let Some(ref r_path) = repo_path {
                let full_path = r_path.join(&file);
                if let Ok(base_content) = std::fs::read_to_string(&full_path) {
                    b_files.insert(file.clone(), base_content);
                    h_files.insert(file, head_lines.join("\n"));
                    return;
                }
            }
        }
        b_files.insert(file.clone(), base_lines.join("\n"));
        h_files.insert(file, head_lines.join("\n"));
    };

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            if let Some(file) = current_file.take() {
                flush_file(
                    file,
                    &current_base_lines,
                    &current_head_lines,
                    &mut base_files,
                    &mut head_files,
                );
                current_base_lines.clear();
                current_head_lines.clear();
            }
        } else if line.starts_with("+++ b/") {
            current_file = Some(line.trim_start_matches("+++ b/").to_string());
        } else if line.starts_with("--- a/") && current_file.is_none() {
            current_file = Some(line.trim_start_matches("--- a/").to_string());
        } else if current_file.is_some() {
            if let Some(stripped) = line.strip_prefix('+') {
                if !line.starts_with("+++") {
                    current_head_lines.push(stripped.to_string());
                }
            } else if let Some(stripped) = line.strip_prefix('-') {
                if !line.starts_with("---") {
                    current_base_lines.push(stripped.to_string());
                }
            } else if let Some(stripped) = line.strip_prefix(' ') {
                current_base_lines.push(stripped.to_string());
                current_head_lines.push(stripped.to_string());
            }
        }
    }

    if let Some(file) = current_file {
        flush_file(
            file,
            &current_base_lines,
            &current_head_lines,
            &mut base_files,
            &mut head_files,
        );
    }

    (base_files, head_files)
}
