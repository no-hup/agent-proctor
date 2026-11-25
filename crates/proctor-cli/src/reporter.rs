use colored::*;
use proctor_core::{Finding, Verdict};

pub fn print_findings(findings: &[Finding], base_ref: &str) {
    if findings.is_empty() {
        println!(
            "\n{}",
            "✔ proctor: PASS. All assertions and test integrity constraints preserved."
                .bold()
                .green()
        );
        println!("  Base reference: {}\n", base_ref.dimmed());
        return;
    }

    let mut blocks = 0;
    let mut reviews = 0;

    for f in findings {
        match f.verdict {
            Verdict::Block { .. } => blocks += 1,
            Verdict::Review { .. } => reviews += 1,
            Verdict::Pass => {}
        }
    }

    println!("\n{}", "─".repeat(70).dimmed());
    if blocks > 0 {
        println!(
            "{}",
            format!(
                "🛑 proctor: BLOCKED ({} violation{}, {} review{})",
                blocks,
                if blocks == 1 { "" } else { "s" },
                reviews,
                if reviews == 1 { "" } else { "s" }
            )
            .bold()
            .red()
        );
        println!(
            "{}",
            "  Specification gaming detected. Changes must not weaken tests to bypass CI.".red()
        );
    } else {
        println!(
            "{}",
            format!(
                "⚠️  proctor: REVIEW REQUIRED ({} finding{})",
                reviews,
                if reviews == 1 { "" } else { "s" }
            )
            .bold()
            .yellow()
        );
        println!(
            "{}",
            "  Changes weaken test coverage but touch production code. Human signoff required."
                .yellow()
        );
    }
    println!("{}\n", "─".repeat(70).dimmed());

    for f in findings {
        let tag = match f.verdict {
            Verdict::Block { .. } => "BLOCK".bold().on_red(),
            Verdict::Review { .. } => "REVIEW".bold().on_yellow(),
            Verdict::Pass => "PASS".green(),
        };

        let loc = if let Some(line) = f.line_number {
            format!("{}:{}", f.file_path, line)
        } else {
            f.file_path.clone()
        };

        println!(
            "  [{}] {} ({})",
            tag,
            f.rule_id.code().bold(),
            f.rule_id.title().dimmed()
        );
        println!("    Location : {}", loc.cyan());
        if let Some(test_name) = &f.test_name {
            println!("    Test     : {}", test_name.bold());
        }
        println!("    Reason   : {}", f.message);
        println!("    Evidence : {}", f.evidence.dimmed());
        println!("    Fingerprint: {}", f.id.bold());

        if let Verdict::Review { finding_id, .. } = &f.verdict {
            println!(
                "    Waiver   : Comment on GitHub PR -> {}",
                format!("proctor: ack {}", finding_id).bold().green()
            );
        } else if let Verdict::Block { .. } = &f.verdict {
            println!(
                "    {}",
                "Agent note: Do not delete assertions or mock in-repo functions. Fix the underlying bug in production code."
                    .italic()
                    .magenta()
            );
        }
        println!();
    }

    // Automatically append to $GITHUB_STEP_SUMMARY if running in GitHub Actions
    if let Ok(summary_path) = std::env::var("GITHUB_STEP_SUMMARY") {
        let _ = write_github_step_summary(&summary_path, findings, base_ref);
    }
}

pub fn write_github_step_summary(
    path: &str,
    findings: &[Finding],
    base_ref: &str,
) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    writeln!(file, "## 🛡️ agent-proctor Verification Report")?;
    writeln!(file, "\n**Base Reference:** `{}`\n", base_ref)?;

    if findings.is_empty() {
        writeln!(
            file,
            "### ✔️ Status: PASS\nAll assertions and test integrity constraints preserved."
        )?;
        return Ok(());
    }

    let blocks = findings
        .iter()
        .filter(|f| matches!(f.verdict, Verdict::Block { .. }))
        .count();
    let reviews = findings
        .iter()
        .filter(|f| matches!(f.verdict, Verdict::Review { .. }))
        .count();

    if blocks > 0 {
        writeln!(
            file,
            "### 🛑 Status: BLOCKED ({} violation{}, {} review{})\n",
            blocks,
            if blocks == 1 { "" } else { "s" },
            reviews,
            if reviews == 1 { "" } else { "s" }
        )?;
        writeln!(file, "> **Warning**: Specification gaming detected. Changes must not weaken tests to bypass CI.\n")?;
    } else {
        writeln!(
            file,
            "### ⚠️ Status: REVIEW REQUIRED ({} finding{})\n",
            reviews,
            if reviews == 1 { "" } else { "s" }
        )?;
        writeln!(file, "> **Notice**: Changes weaken test coverage but touch production code. Human signoff required.\n")?;
    }

    writeln!(
        file,
        "| Fingerprint | Rule | Location | Verdict | Waiver Command |"
    )?;
    writeln!(file, "|---|---|---|---|---|")?;

    for f in findings {
        let verdict_str = match f.verdict {
            Verdict::Block { .. } => "**BLOCK**",
            Verdict::Review { .. } => "**REVIEW**",
            Verdict::Pass => "PASS",
        };
        let loc = if let Some(line) = f.line_number {
            format!("{}:{}", f.file_path, line)
        } else {
            f.file_path.clone()
        };
        let waiver_cmd = if let Verdict::Review { finding_id, .. } = &f.verdict {
            format!("`proctor: accept {}`", finding_id)
        } else {
            "-".to_string()
        };

        writeln!(
            file,
            "| `{}` | {} | `{}` | {} | {} |",
            f.id,
            f.rule_id.code(),
            loc,
            verdict_str,
            waiver_cmd
        )?;
    }

    writeln!(file, "\n#### Reviewer Instructions")?;
    writeln!(
        file,
        "To approve legitimate refactors or flakiness quarantines, an authorized repository maintainer (non-author) can comment:\n"
    )?;
    writeln!(file, "```text\nproctor: accept <fingerprint>\n```\n")?;

    Ok(())
}

/// Test files in languages the engine does not parse (reported, never silently counted as clean).
fn is_unanalyzed_test(path: &str) -> bool {
    let p = std::path::Path::new(path);
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
    const UNSUPPORTED: [&str; 12] = [
        "go", "rs", "rb", "java", "kt", "php", "cs", "swift", "scala", "ex", "exs", "cpp",
    ];
    if !UNSUPPORTED.contains(&ext) {
        return false;
    }
    let name = p
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("")
        .to_lowercase();
    let in_test_dir = p.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("tests" | "test" | "spec" | "__tests__")
        )
    });
    in_test_dir
        || name.contains("_test.")
        || name.contains("_spec.")
        || name.starts_with("test_")
        || name.contains("test.")
}

/// Renders the PR Test-Change Receipt: what happened to tests, gates, and suppressions in this diff.
pub fn render_receipt(diff: &proctor_diff::SemanticDiff, findings: &[Finding]) -> String {
    use proctor_core::{AssertionStrength, RuleId, RuleTier};
    use std::collections::BTreeSet;
    use std::collections::HashSet;

    let flagged_tests: HashSet<&str> = findings
        .iter()
        .filter_map(|f| f.test_name.as_deref())
        .collect();
    let exact = |t: &proctor_parse::ParsedTest| {
        t.assertions
            .iter()
            .filter(|a| a.strength == AssertionStrength::Exact)
            .count()
    };
    let count_rule = |rules: &[RuleId]| {
        findings
            .iter()
            .filter(|f| rules.contains(&f.rule_id))
            .count()
    };

    let unanalyzed: BTreeSet<&str> = diff
        .raw_base_files
        .keys()
        .chain(diff.raw_head_files.keys())
        .map(String::as_str)
        .filter(|p| is_unanalyzed_test(p))
        .collect();

    let added = diff.new_tests.len();
    let added_no_exact = diff.new_tests.iter().filter(|t| exact(t) == 0).count();

    let (mut strengthened, mut flagged_mod, mut unflagged) = (0, 0, 0);
    for p in diff
        .paired_tests
        .iter()
        .filter(|p| p.base_test.id.body_content_hash != p.head_test.id.body_content_hash)
    {
        if flagged_tests.contains(p.head_test.id.canonical_name().as_str()) {
            flagged_mod += 1;
        } else if exact(&p.head_test) > exact(&p.base_test) {
            strengthened += 1;
        } else {
            unflagged += 1;
        }
    }
    let modified = strengthened + flagged_mod + unflagged;

    let deleted = diff.deleted_tests.len();
    let deleted_flagged = diff
        .deleted_tests
        .iter()
        .filter(|t| flagged_tests.contains(t.id.canonical_name().as_str()))
        .count();

    let gates = count_rule(&[
        RuleId::CiWorkflowSabotage,
        RuleId::RunnerConfigTamper,
        RuleId::PolicyTamper,
        RuleId::SnapshotIntegrity,
    ]);
    let suppressions = count_rule(&[RuleId::BlanketSuppression]);
    let mocks = count_rule(&[RuleId::ShadowMock]);

    if added + modified + deleted + unanalyzed.len() == 0 && findings.is_empty() {
        return "**agent-proctor:** no test changes and no gate loosening found in this PR.\n"
            .to_string();
    }

    let ok = |n: usize| if n == 0 { "✅" } else { "⚠️" };
    let mut out = String::from("### 📋 Test-Change Receipt (agent-proctor)\n\n");
    out += "| Category | Count | Status |\n|---|---|---|\n";
    out += &format!(
        "| **Added tests** | {} | {} |\n",
        added,
        if added == 0 {
            "—".to_string()
        } else if added_no_exact == 0 {
            "all contain an exact-value assertion (not executed)".to_string()
        } else {
            format!("⚠️ {} without an exact-value assertion", added_no_exact)
        }
    );
    out += &format!(
        "| **Modified tests** | {} | {} {} strengthened, {} changed without findings, {} flagged |\n",
        modified, ok(flagged_mod), strengthened, unflagged, flagged_mod
    );
    out += &format!(
        "| **Deleted tests** | {} | {} {} justified (every production symbol they touch was removed), {} flagged |\n",
        deleted, ok(deleted_flagged), deleted - deleted_flagged, deleted_flagged
    );
    out += &format!(
        "| **Configs & CI gates** | {} | {} |\n",
        gates,
        if gates == 0 {
            "✅ no loosening found"
        } else {
            "⚠️ loosened"
        }
    );
    out += &format!(
        "| **Blanket suppressions** | {} | {} |\n",
        suppressions,
        if suppressions == 0 {
            "✅ none added"
        } else {
            "⚠️ added"
        }
    );
    out += &format!(
        "| **Internal mocks** | {} | {} |\n",
        mocks,
        if mocks == 0 {
            "✅ none added"
        } else {
            "⚠️ added"
        }
    );
    if !unanalyzed.is_empty() {
        let list: Vec<String> = unanalyzed
            .iter()
            .take(5)
            .map(|p| format!("`{}`", p))
            .collect();
        let more = if unanalyzed.len() > 5 {
            format!(" +{} more", unanalyzed.len() - 5)
        } else {
            String::new()
        };
        out += &format!(
            "| **Not analyzed (unsupported language)** | {} | ⚠️ review by hand: {}{} |\n",
            unanalyzed.len(),
            list.join(", "),
            more
        );
    }

    let blocks = findings
        .iter()
        .filter(|f| f.tier == RuleTier::Guard)
        .count();
    let reviews = findings.len() - blocks;
    out += &match (blocks, reviews) {
        (0, 0) => "\n**Verdict:** 🟢 No findings in analyzed test files.\n".to_string(),
        (0, r) => format!(
            "\n**Verdict:** 🟡 {} item{} worth a look.\n",
            r,
            if r == 1 { "" } else { "s" }
        ),
        (b, _) => format!(
            "\n**Verdict:** 🔴 {} blocking finding{}.\n",
            b,
            if b == 1 { "" } else { "s" }
        ),
    };

    if !findings.is_empty() {
        out += "\n<details><summary>Findings</summary>\n\n| Tier | Rule | Location | Detail |\n|---|---|---|---|\n";
        // Guard findings always listed in full; Report findings capped so huge PRs stay readable.
        const MAX_REPORT_ROWS: usize = 10;
        let (guards, reports): (Vec<&Finding>, Vec<&Finding>) =
            findings.iter().partition(|f| f.tier == RuleTier::Guard);
        for f in guards.iter().chain(reports.iter().take(MAX_REPORT_ROWS)) {
            let loc = match f.line_number {
                Some(l) => format!("{}:{}", f.file_path, l),
                None => f.file_path.clone(),
            };
            let detail = f.message.replace('|', "\\|").replace('\n', " ");
            out += &format!(
                "| {:?} | `{}` | `{}` | {} |\n",
                f.tier,
                f.rule_id.code(),
                loc,
                detail
            );
        }
        if reports.len() > MAX_REPORT_ROWS {
            let rest = &reports[MAX_REPORT_ROWS..];
            let files: HashSet<&str> = rest.iter().map(|f| f.file_path.as_str()).collect();
            out += &format!(
                "\n… and {} more review-tier findings across {} file{} (full list: `proctor scan --format json`).\n",
                rest.len(), files.len(), if files.len() == 1 { "" } else { "s" }
            );
        }
        out += "\n</details>\n";
    }
    out += "\n<sub>Static analysis of the diff only; no tests were executed. Dynamic red/green verification is opt-in.</sub>\n";
    out
}

pub fn print_verification_results(results: &[proctor_probe::TestVerificationResult]) {
    use proctor_probe::VerificationStatus;

    if results.is_empty() {
        println!(
            "\n{}",
            "ℹ️ proctor verify: No changed tests found to verify.".dimmed()
        );
        return;
    }

    println!("\n{}", "─".repeat(70).dimmed());
    println!(
        "{}",
        "🧪 proctor verify: Dynamic Regression Verification (experimental)"
            .bold()
            .cyan()
    );
    println!("{}\n", "─".repeat(70).dimmed());

    let mut discriminating = 0;
    let mut missing = 0;
    let mut non_discriminating = 0;
    let mut broken = 0;
    let mut inconclusive = 0;

    for r in results {
        match &r.status {
            VerificationStatus::Discriminating { reason } => {
                discriminating += 1;
                println!(
                    "  {} {}",
                    "🛡️ CATCHES CHANGE".bold().green(),
                    r.test_id.bold()
                );
                println!("    Runner  : {} ({}ms)", r.runner.cyan(), r.duration_ms);
                println!("    Status  : Passed 2x on head, failed 2x on base with assertion error");
                println!("    Details : {}\n", reason.dimmed());
            }
            VerificationStatus::MissingOnBase { error } => {
                missing += 1;
                println!(
                    "  {} {}",
                    "⚠️  MISSING ON BASE".bold().yellow(),
                    r.test_id.bold()
                );
                println!("    Runner  : {} ({}ms)", r.runner.cyan(), r.duration_ms);
                println!("    Status  : Passed on head, failed on base with missing symbol/module");
                println!("    Details : {}\n", error.dimmed());
            }
            VerificationStatus::DoesNotDiscriminate => {
                non_discriminating += 1;
                println!(
                    "  {} {}",
                    "⚪ PASSES ON BOTH".bold().dimmed(),
                    r.test_id.bold()
                );
                println!("    Runner  : {} ({}ms)", r.runner.cyan(), r.duration_ms);
                println!("    Status  : Passed on both head and base (expected for refactor/coverage tests)\n");
            }
            VerificationStatus::Broken { error } => {
                broken += 1;
                println!("  {} {}", "❌ BROKEN".bold().red(), r.test_id.bold());
                println!("    Runner  : {} ({}ms)", r.runner.cyan(), r.duration_ms);
                println!("    Status  : Failed on head revision");
                println!("    Details : {}\n", error.red());
            }
            VerificationStatus::Inconclusive { reason } => {
                inconclusive += 1;
                println!(
                    "  {} {}",
                    "ℹ️  INCONCLUSIVE".bold().blue(),
                    r.test_id.bold()
                );
                println!("    Runner  : {} ({}ms)", r.runner.cyan(), r.duration_ms);
                println!("    Reason  : {}\n", reason.dimmed());
            }
        }
    }

    println!("{}", "─".repeat(70).dimmed());
    println!(
        "Summary: {} catches change, {} missing on base, {} passes on both, {} broken, {} inconclusive",
        discriminating.to_string().green().bold(),
        missing.to_string().yellow(),
        non_discriminating,
        broken.to_string().red(),
        inconclusive
    );
    println!("{}\n", "─".repeat(70).dimmed());
}

pub fn render_verification_receipt(results: &[proctor_probe::TestVerificationResult]) -> String {
    use proctor_probe::VerificationStatus;

    if results.is_empty() {
        return "### 🧪 Dynamic Test Verification Receipt (experimental)\n\nNo changed tests were verified.\n".to_string();
    }

    let mut out = String::from("### 🧪 Dynamic Test Verification Receipt (experimental)\n\n");
    out += "| Test | Verdict | Details | Runner | Duration |\n";
    out += "|---|---|---|---|---|\n";

    let mut discriminating = 0;
    let mut missing = 0;
    let mut non_discriminating = 0;
    let mut broken = 0;
    let mut inconclusive = 0;

    for r in results {
        let (verdict_str, details_str) = match &r.status {
            VerificationStatus::Discriminating { reason } => {
                discriminating += 1;
                (
                    "🛡️ **Catches change**",
                    reason.replace('|', "\\|").replace('\n', " "),
                )
            }
            VerificationStatus::MissingOnBase { error } => {
                missing += 1;
                (
                    "⚠️ **Missing on base**",
                    error.replace('|', "\\|").replace('\n', " "),
                )
            }
            VerificationStatus::DoesNotDiscriminate => {
                non_discriminating += 1;
                (
                    "⚪ **Passes on both**",
                    "Passed on both head and base".to_string(),
                )
            }
            VerificationStatus::Broken { error } => {
                broken += 1;
                (
                    "❌ **Broken**",
                    error.replace('|', "\\|").replace('\n', " "),
                )
            }
            VerificationStatus::Inconclusive { reason } => {
                inconclusive += 1;
                (
                    "ℹ️ **Inconclusive**",
                    reason.replace('|', "\\|").replace('\n', " "),
                )
            }
        };

        out += &format!(
            "| `{}` | {} | {} | `{}` | {}ms |\n",
            r.test_id, verdict_str, details_str, r.runner, r.duration_ms
        );
    }

    out += "\n**Verification Summary:**\n";
    out += &format!(
        "- 🛡️ **Catches change:** {} (passed 2x on head, failed 2x on base with assertion failure)\n",
        discriminating
    );
    out += &format!(
        "- ⚠️ **Missing on base:** {} (import or missing-symbol error on base)\n",
        missing
    );
    out += &format!(
        "- ⚪ **Passes on both:** {} (expected for refactor/coverage tests; check if testing a behavior change)\n",
        non_discriminating
    );
    if broken > 0 {
        out += &format!("- ❌ **Broken:** {} (failed on head)\n", broken);
    }
    if inconclusive > 0 {
        out += &format!(
            "- ℹ️ **Inconclusive:** {} (timeout, flaky, dependency change, or setup crash)\n",
            inconclusive
        );
    }

    out += "\n<sub>Verified via inverted file swap in the host checkout using structured runner outputs; dynamic verification results do not change the static scan exit code.</sub>\n";
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn receipt_flags_unanalyzed_go_tests_and_is_quiet_when_nothing_changed() {
        let base = HashMap::from([("pkg/a_test.go".to_string(), "package pkg\n".to_string())]);
        let head = HashMap::new();
        let diff = proctor_diff::SemanticDiff::build(&base, &head).unwrap();
        let r = render_receipt(&diff, &[]);
        assert!(r.contains("Not analyzed"), "{r}");
        assert!(r.contains("pkg/a_test.go"));
        assert!(!r.contains("Nothing to review"));
        assert!(r.contains("no tests were executed"));

        let same = HashMap::from([("README.md".to_string(), "x".to_string())]);
        let diff = proctor_diff::SemanticDiff::build(&same, &same).unwrap();
        assert_eq!(render_receipt(&diff, &[]).lines().count(), 1);
    }

    #[test]
    fn receipt_caps_report_rows_and_lists_guards_first() {
        use proctor_core::{RuleId, RuleTier, Verdict};
        let mk = |i: usize, tier: RuleTier| Finding {
            id: format!("id{i}"),
            rule_id: RuleId::AssertionLock,
            tier,
            verdict: match tier {
                RuleTier::Guard => Verdict::Block {
                    rule: RuleId::AssertionLock,
                    evidence: String::new(),
                },
                RuleTier::Report => Verdict::Review {
                    finding_id: format!("id{i}"),
                    evidence: String::new(),
                },
            },
            file_path: format!("tests/t{}.py", i % 3),
            line_number: Some(i),
            test_name: None,
            message: format!("msg{i}"),
            evidence: String::new(),
        };
        let mut findings: Vec<Finding> = (0..25).map(|i| mk(i, RuleTier::Report)).collect();
        findings.push(mk(99, RuleTier::Guard));
        let empty = HashMap::new();
        let diff = proctor_diff::SemanticDiff::build(&empty, &empty).unwrap();
        let r = render_receipt(&diff, &findings);
        let rows: Vec<&str> = r
            .lines()
            .filter(|l| l.starts_with("| Guard") || l.starts_with("| Report"))
            .collect();
        assert_eq!(rows.len(), 11);
        assert!(rows[0].starts_with("| Guard"));
        assert!(
            r.contains("15 more review-tier findings across 3 files"),
            "{r}"
        );
    }
}
