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
        println!(
            "  Base reference: {}\n",
            base_ref.dimmed()
        );
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
            "  Specification gaming detected. Changes must not weaken tests to bypass CI."
                .red()
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

pub fn write_github_step_summary(path: &str, findings: &[Finding], base_ref: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    writeln!(file, "## 🛡️ agent-proctor Verification Report")?;
    writeln!(file, "\n**Base Reference:** `{}`\n", base_ref)?;

    if findings.is_empty() {
        writeln!(file, "### ✔️ Status: PASS\nAll assertions and test integrity constraints preserved.")?;
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

    writeln!(file, "| Fingerprint | Rule | Location | Verdict | Waiver Command |")?;
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

/// Renders the PR Test-Change Receipt: what happened to tests, gates, and suppressions in this diff.
pub fn render_receipt(diff: &proctor_diff::SemanticDiff, findings: &[Finding]) -> String {
    use proctor_core::{AssertionStrength, RuleId, RuleTier};
    use std::collections::HashSet;

    let flagged_tests: HashSet<&str> = findings.iter().filter_map(|f| f.test_name.as_deref()).collect();
    let exact = |t: &proctor_parse::ParsedTest| t.assertions.iter().filter(|a| a.strength == AssertionStrength::Exact).count();
    let count_rule = |rules: &[RuleId]| findings.iter().filter(|f| rules.contains(&f.rule_id)).count();

    // Added
    let added = diff.new_tests.len();
    let added_no_exact = diff.new_tests.iter().filter(|t| exact(t) == 0).count();

    // Modified
    let (mut strengthened, mut flagged_mod, mut neutral) = (0, 0, 0);
    for p in diff.paired_tests.iter().filter(|p| p.base_test.id.body_content_hash != p.head_test.id.body_content_hash) {
        if flagged_tests.contains(p.head_test.id.canonical_name().as_str()) {
            flagged_mod += 1;
        } else if exact(&p.head_test) > exact(&p.base_test) {
            strengthened += 1;
        } else {
            neutral += 1;
        }
    }
    let modified = strengthened + flagged_mod + neutral;

    // Deleted
    let deleted = diff.deleted_tests.len();
    let deleted_flagged = diff.deleted_tests.iter().filter(|t| flagged_tests.contains(t.id.canonical_name().as_str())).count();

    let gates = count_rule(&[RuleId::CiWorkflowSabotage, RuleId::RunnerConfigTamper, RuleId::PolicyTamper, RuleId::SnapshotIntegrity]);
    let suppressions = count_rule(&[RuleId::BlanketSuppression]);
    let mocks = count_rule(&[RuleId::ShadowMock]);

    let ok = |n: usize| if n == 0 { "✅" } else { "⚠️" };
    let mut out = String::from("### 📋 Test-Change Receipt (agent-proctor)\n\n");
    out += "| Category | Count | Status |\n|---|---|---|\n";
    out += &format!(
        "| **Added tests** | {} | {} |\n",
        added,
        if added == 0 { "—".to_string() } else if added_no_exact == 0 { "✅ all have exact assertions".to_string() } else { format!("⚠️ {} without an exact assertion", added_no_exact) }
    );
    out += &format!(
        "| **Modified tests** | {} | {} {} strengthened, {} neutral, {} flagged |\n",
        modified, ok(flagged_mod), strengthened, neutral, flagged_mod
    );
    out += &format!(
        "| **Deleted tests** | {} | {} {} obsolete (production symbols removed in this PR), {} flagged |\n",
        deleted, ok(deleted_flagged), deleted - deleted_flagged, deleted_flagged
    );
    out += &format!("| **Configs & CI gates** | {} | {} |\n", gates, if gates == 0 { "✅ no loosening" } else { "⚠️ loosened" });
    out += &format!("| **Blanket suppressions** | {} | {} |\n", suppressions, if suppressions == 0 { "✅ none added" } else { "⚠️ added" });
    out += &format!("| **Internal mocks** | {} | {} |\n", mocks, if mocks == 0 { "✅ none added" } else { "⚠️ added" });

    let blocks = findings.iter().filter(|f| f.tier == RuleTier::Guard).count();
    let reviews = findings.len() - blocks;
    out += &match (blocks, reviews) {
        (0, 0) => "\n**Verdict:** 🟢 Nothing to review in the test diff.\n".to_string(),
        (0, r) => format!("\n**Verdict:** 🟡 {} item{} worth a look.\n", r, if r == 1 { "" } else { "s" }),
        (b, _) => format!("\n**Verdict:** 🔴 {} blocking finding{}.\n", b, if b == 1 { "" } else { "s" }),
    };

    if !findings.is_empty() {
        out += "\n<details><summary>Findings</summary>\n\n| Tier | Rule | Location | Detail |\n|---|---|---|---|\n";
        for f in findings {
            let loc = match f.line_number {
                Some(l) => format!("{}:{}", f.file_path, l),
                None => f.file_path.clone(),
            };
            let detail = f.message.replace('|', "\\|").replace('\n', " ");
            out += &format!("| {:?} | `{}` | `{}` | {} |\n", f.tier, f.rule_id.code(), loc, detail);
        }
        out += "\n</details>\n";
    }
    out
}
