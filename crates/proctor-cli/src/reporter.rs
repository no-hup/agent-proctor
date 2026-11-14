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
}
