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
                format!("proctor: accept {}", finding_id).bold().green()
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
