use colored::*;
use proctor_core::{Finding, RuleId, Verdict};
use std::collections::BTreeMap;

const MAX_GUARD_ROWS: usize = 25;
const MAX_REPORT_ROWS: usize = 10;
/// When a single file contributes more than this many findings, collapse them into one row.
const AGGREGATE_PER_FILE: usize = 5;

/// Specific human-readable title matching what happened in this finding.
fn finding_display_title(f: &Finding) -> &'static str {
    if f.rule_id != RuleId::AssertionLock {
        return f.rule_id.title();
    }
    if f.evidence.starts_with("Deleted test:") {
        "Test deleted without removing all referenced production code"
    } else if f.evidence.starts_with("Skipped") {
        "Test marked skipped / xfailed"
    } else if f.message.contains("while production code was untouched") {
        "Test expectation changed with no production changes"
    } else if f.message.contains("weakened") {
        "Assertion weakened or removed"
    } else if f.message.contains("shifted to mirror") {
        "Test expected value shifted to match new production literal"
    } else if f.message.contains("could not be parsed") {
        "File could not be parsed safely"
    } else {
        "Assertion changed with no changed target symbol detected"
    }
}

/// Strips redundant `<file_path>::` prefixes inside finding messages/details when `Location` already shows the file.
fn compact_detail(f: &Finding) -> String {
    let prefix = format!("{}::", f.file_path);
    f.message
        .replace(&prefix, "")
        .replace('|', "\\|")
        .replace('\n', " ")
}

struct DisplayGuardEntry<'a> {
    title: &'a str,
    code: &'a str,
    loc: String,
    test_display: Option<String>,
    detail: String,
    evidence: &'a str,
    id: &'a str,
}

fn group_guard_entries<'a>(guards: &[&'a Finding]) -> Vec<DisplayGuardEntry<'a>> {
    let mut entries = Vec::new();
    let mut deleted_by_file: BTreeMap<&str, Vec<&'a Finding>> = BTreeMap::new();
    for &f in guards {
        if f.rule_id == RuleId::AssertionLock && f.evidence.starts_with("Deleted test:") {
            deleted_by_file
                .entry(f.file_path.as_str())
                .or_default()
                .push(f);
        } else {
            let loc = match f.line_number {
                Some(l) => format!("{}:{}", f.file_path, l),
                None => f.file_path.clone(),
            };
            let prefix = format!("{}::", f.file_path);
            entries.push(DisplayGuardEntry {
                title: finding_display_title(f),
                code: f.rule_id.code(),
                loc,
                test_display: f
                    .test_name
                    .as_ref()
                    .map(|t| t.strip_prefix(&prefix).unwrap_or(t).to_string()),
                detail: compact_detail(f),
                evidence: &f.evidence,
                id: &f.id,
            });
        }
    }
    for (path, group) in &deleted_by_file {
        if group.len() == 1 {
            let f = group[0];
            let loc = match f.line_number {
                Some(l) => format!("{}:{}", f.file_path, l),
                None => f.file_path.clone(),
            };
            let prefix = format!("{}::", f.file_path);
            entries.push(DisplayGuardEntry {
                title: finding_display_title(f),
                code: f.rule_id.code(),
                loc,
                test_display: f
                    .test_name
                    .as_ref()
                    .map(|t| t.strip_prefix(&prefix).unwrap_or(t).to_string()),
                detail: compact_detail(f),
                evidence: &f.evidence,
                id: &f.id,
            });
        } else {
            entries.push(DisplayGuardEntry {
                title: "Tests deleted without removing all referenced production code",
                code: RuleId::AssertionLock.code(),
                loc: (*path).to_string(),
                test_display: None,
                detail: format!(
                    "{} tests deleted in `{}` without removing all referenced production symbols",
                    group.len(),
                    path
                ),
                evidence: "grouped deleted tests (use --format json for individual fingerprints)",
                id: group[0].id.as_str(),
            });
        }
    }
    entries
}

struct DisplayReportEntry<'a> {
    code: &'a str,
    loc: String,
    title: &'a str,
    detail: String,
    evidence: Option<&'a str>,
    id: Option<&'a str>,
    test_display: Option<String>,
    aggregated: bool,
    count: usize,
}

/// Collapse repetitive review findings when a single file has more than AGGREGATE_PER_FILE.
fn group_report_entries<'a>(reports: &[&'a Finding]) -> Vec<DisplayReportEntry<'a>> {
    let mut by_file: BTreeMap<&str, Vec<&'a Finding>> = BTreeMap::new();
    for &f in reports {
        by_file.entry(f.file_path.as_str()).or_default().push(f);
    }
    let mut entries = Vec::new();
    for (path, group) in by_file {
        if group.len() > AGGREGATE_PER_FILE {
            let code = group[0].rule_id.code();
            entries.push(DisplayReportEntry {
                code,
                loc: path.to_string(),
                title: "Multiple assertion edits in one file",
                detail: format!(
                    "{} assertion edits in `{}` (review required; run proctor scan --format json for full list)",
                    group.len(),
                    path
                ),
                evidence: None,
                id: None,
                test_display: None,
                aggregated: true,
                count: group.len(),
            });
        } else {
            for f in group {
                let loc = match f.line_number {
                    Some(l) => format!("{}:{}", f.file_path, l),
                    None => f.file_path.clone(),
                };
                let prefix = format!("{}::", f.file_path);
                entries.push(DisplayReportEntry {
                    code: f.rule_id.code(),
                    loc,
                    title: finding_display_title(f),
                    detail: compact_detail(f),
                    evidence: Some(&f.evidence),
                    id: Some(&f.id),
                    test_display: f
                        .test_name
                        .as_ref()
                        .map(|t| t.strip_prefix(&prefix).unwrap_or(t).to_string()),
                    aggregated: false,
                    count: 1,
                });
            }
        }
    }
    entries
}

pub fn print_findings(findings: &[Finding], base_ref: &str, parse_error_count: usize) {
    if findings.is_empty() {
        println!(
            "\n{}",
            "✔ proctor: PASS. All assertions and test integrity constraints preserved."
                .bold()
                .green()
        );
        println!("  Base reference: {}\n", base_ref.dimmed());
        if parse_error_count > 0 {
            println!(
                "  ℹ️  {} file(s) skipped due to syntax/parse limit.\n",
                parse_error_count
            );
        }
        return;
    }

    let (guards, reports): (Vec<&Finding>, Vec<&Finding>) = findings
        .iter()
        .partition(|f| matches!(f.verdict, Verdict::Block { .. }));
    let blocks = guards.len();
    let reviews = reports.len();

    println!("\n{}", "─".repeat(70).dimmed());
    if blocks > 0 {
        println!(
            "{}",
            format!(
                "🛑 proctor: BLOCKED ({} guard finding{}, {} review{})",
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
            "  Guard-tier test changes detected without matching production deletions or waivers."
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
            "  Test assertions or coverage changed alongside production code; review or acknowledge before merge."
                .yellow()
        );
    }
    println!("{}\n", "─".repeat(70).dimmed());

    if parse_error_count > 0 {
        println!(
            "  ℹ️  {} file(s) skipped due to syntax/parse limit.\n",
            parse_error_count
        );
    }

    let guard_entries = group_guard_entries(&guards);
    for entry in guard_entries.iter().take(MAX_GUARD_ROWS) {
        println!(
            "  [{}] {} ({})",
            "BLOCK".bold().on_red(),
            entry.code.bold(),
            entry.title.dimmed()
        );
        println!("    Location : {}", entry.loc.cyan());
        if let Some(t) = &entry.test_display {
            println!("    Test     : {}", t.bold());
        }
        println!("    Reason   : {}", entry.detail);
        println!("    Evidence : {}", entry.evidence.dimmed());
        println!("    Fingerprint: {}", entry.id.bold());
        println!(
            "    {}",
            format!(
                "Note: Verify this change matches an intended spec/test update (waive with: proctor: accept {}).",
                entry.id
            )
            .italic()
            .magenta()
        );
        println!();
    }
    if guard_entries.len() > MAX_GUARD_ROWS {
        println!(
            "  … and {} more guard entries ({} total guard findings; run `proctor scan --format json` for the full list).\n",
            guard_entries.len() - MAX_GUARD_ROWS,
            blocks
        );
    }

    let report_entries = group_report_entries(&reports);
    for entry in report_entries.iter().take(MAX_REPORT_ROWS) {
        println!(
            "  [{}] {} ({})",
            "REVIEW".bold().on_yellow(),
            entry.code.bold(),
            entry.title.dimmed()
        );
        println!("    Location : {}", entry.loc.cyan());
        if let Some(t) = &entry.test_display {
            println!("    Test     : {}", t.bold());
        }
        println!("    Reason   : {}", entry.detail);
        if let Some(ev) = entry.evidence {
            println!("    Evidence : {}", ev.dimmed());
        }
        if let Some(id) = entry.id {
            println!("    Fingerprint: {}", id.bold());
            println!(
                "    Waiver   : Comment on GitHub PR -> {}",
                format!("proctor: ack {}", id).bold().green()
            );
        } else if entry.aggregated {
            println!(
                "    Note     : {} findings collapsed; run `proctor scan --format json` for fingerprints.",
                entry.count
            );
        }
        println!();
    }
    if report_entries.len() > MAX_REPORT_ROWS {
        let shown_findings: usize = report_entries
            .iter()
            .take(MAX_REPORT_ROWS)
            .map(|e| e.count)
            .sum();
        println!(
            "  … and {} more review findings ({} total; run `proctor scan --format json` for the full list).\n",
            reviews.saturating_sub(shown_findings),
            reviews
        );
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
            "### 🛑 Status: BLOCKED ({} guard finding{}, {} review{})\n",
            blocks,
            if blocks == 1 { "" } else { "s" },
            reviews,
            if reviews == 1 { "" } else { "s" }
        )?;
        writeln!(file, "> **Warning**: Guard-tier test changes detected without matching production deletions or waivers.\n")?;
    } else {
        writeln!(
            file,
            "### ⚠️ Status: REVIEW REQUIRED ({} finding{})\n",
            reviews,
            if reviews == 1 { "" } else { "s" }
        )?;
        writeln!(file, "> **Notice**: Test assertions or coverage changed alongside production code; review or acknowledge before merge.\n")?;
    }

    writeln!(
        file,
        "| Fingerprint | Rule | Location | Verdict | Waiver Command |"
    )?;
    writeln!(file, "|---|---|---|---|---|")?;

    let (guards, reports): (Vec<&Finding>, Vec<&Finding>) = findings
        .iter()
        .partition(|f| matches!(f.verdict, Verdict::Block { .. }));

    for f in &guards {
        let loc = if let Some(line) = f.line_number {
            format!("{}:{}", f.file_path, line)
        } else {
            f.file_path.clone()
        };
        writeln!(
            file,
            "| `{}` | {} | `{}` | **BLOCK** | - |",
            f.id,
            f.rule_id.code(),
            loc
        )?;
    }

    let report_entries = group_report_entries(&reports);
    for entry in report_entries {
        if entry.aggregated {
            writeln!(
                file,
                "| — ({} edits) | {} | `{}` | **REVIEW** | run `proctor scan --format json` |",
                entry.count, entry.code, entry.loc
            )?;
        } else {
            let waiver = entry
                .id
                .map(|id| format!("`proctor: accept {}`", id))
                .unwrap_or_else(|| "-".to_string());
            writeln!(
                file,
                "| `{}` | {} | `{}` | **REVIEW** | {} |",
                entry.id.unwrap_or("—"),
                entry.code,
                entry.loc,
                waiver
            )?;
        }
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
    let mut changed_assertions: Vec<(String, String)> = Vec::new();
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

        let base_set: HashSet<&str> = p
            .base_test
            .assertions
            .iter()
            .map(|a| a.text.as_str())
            .collect();
        let head_set: HashSet<&str> = p
            .head_test
            .assertions
            .iter()
            .map(|a| a.text.as_str())
            .collect();
        let removed: Vec<&str> = p
            .base_test
            .assertions
            .iter()
            .map(|a| a.text.as_str())
            .filter(|t| !head_set.contains(t))
            .collect();
        let added_a: Vec<&str> = p
            .head_test
            .assertions
            .iter()
            .map(|a| a.text.as_str())
            .filter(|t| !base_set.contains(t))
            .collect();
        let test_label = p.head_test.id.canonical_name();
        for (idx, old_txt) in removed.iter().enumerate() {
            let old_s = old_txt.replace('|', "\\|").replace('\n', " ");
            let diff_str = if let Some(new_txt) = added_a.get(idx) {
                let new_s = new_txt.replace('|', "\\|").replace('\n', " ");
                format!("`{}` → `{}`", old_s, new_s)
            } else {
                format!("removed `{}`", old_s)
            };
            changed_assertions.push((test_label.clone(), diff_str));
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

    if added + modified + deleted + unanalyzed.len() + diff.parse_errors.len() == 0
        && findings.is_empty()
    {
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
        "| **Modified tests** | {} | {} |\n",
        modified,
        if modified == 0 {
            "—".to_string()
        } else {
            format!(
                "{} {} strengthened, {} changed without findings, {} flagged",
                ok(flagged_mod),
                strengthened,
                unflagged,
                flagged_mod
            )
        }
    );
    out += &format!(
        "| **Deleted tests** | {} | {} |\n",
        deleted,
        if deleted == 0 {
            "—".to_string()
        } else {
            format!(
                "{} {} justified (every production symbol they touch was removed), {} flagged",
                ok(deleted_flagged),
                deleted - deleted_flagged,
                deleted_flagged
            )
        }
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
    if !diff.parse_errors.is_empty() {
        let mut paths: BTreeSet<&str> = BTreeSet::new();
        for (path, _) in &diff.parse_errors {
            paths.insert(path.as_str());
        }
        let list: Vec<String> = paths.iter().take(5).map(|p| format!("`{}`", p)).collect();
        let more = if paths.len() > 5 {
            format!(" +{} more", paths.len() - 5)
        } else {
            String::new()
        };
        out += &format!(
            "| **Not analyzed (syntax/parse limit)** | {} | ℹ️ skipped: {}{} |\n",
            paths.len(),
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
        (b, 0) => format!(
            "\n**Verdict:** 🔴 {} guard finding{} (would block under `--enforce`).\n",
            b,
            if b == 1 { "" } else { "s" }
        ),
        (b, r) => format!(
            "\n**Verdict:** 🔴 {} guard finding{} (would block under `--enforce`), {} review item{}.\n",
            b,
            if b == 1 { "" } else { "s" },
            r,
            if r == 1 { "" } else { "s" }
        ),
    };

    if !findings.is_empty() {
        out += "\n<details><summary>Findings</summary>\n\n| Tier | Rule | Location | Detail |\n|---|---|---|---|\n";
        let (guards, reports): (Vec<&Finding>, Vec<&Finding>) =
            findings.iter().partition(|f| f.tier == RuleTier::Guard);
        let guard_entries = group_guard_entries(&guards);
        let guard_rows_total = guard_entries.len();
        let report_entries = group_report_entries(&reports);

        for entry in guard_entries.iter().take(MAX_GUARD_ROWS) {
            out += &format!(
                "| Guard | `{}` | `{}` | {} |\n",
                entry.code, entry.loc, entry.detail
            );
        }
        for entry in report_entries.iter().take(MAX_REPORT_ROWS) {
            out += &format!(
                "| Report | `{}` | `{}` | {} |\n",
                entry.code, entry.loc, entry.detail
            );
        }
        if guard_rows_total > MAX_GUARD_ROWS {
            out += &format!(
                "\n… and {} more guard entries ({} total guard findings; full list: `proctor scan --format json`).\n",
                guard_rows_total - MAX_GUARD_ROWS,
                guards.len()
            );
        }
        if report_entries.len() > MAX_REPORT_ROWS {
            let shown: usize = report_entries
                .iter()
                .take(MAX_REPORT_ROWS)
                .map(|e| e.count)
                .sum();
            let rest = reports.len().saturating_sub(shown);
            let files: HashSet<&str> = reports.iter().map(|f| f.file_path.as_str()).collect();
            out += &format!(
                "\n… and {} more review-tier findings across {} file{} (full list: `proctor scan --format json`).\n",
                rest, files.len(), if files.len() == 1 { "" } else { "s" }
            );
        }
        out += "\nTo acknowledge a finding on a PR, comment `proctor: ack <id>` (review) or `proctor: accept <id>` (guard waiver); run `proctor scan --format json` for IDs.\n";
        out += "\n</details>\n";
    }

    if !changed_assertions.is_empty() {
        out += &format!(
            "\n<details><summary>Changed pre-existing assertions ({})</summary>\n\n| Test | Assertion Change |\n|---|---|\n",
            changed_assertions.len()
        );
        for (test_label, change) in changed_assertions.iter().take(10) {
            out += &format!("| `{}` | {} |\n", test_label, change);
        }
        if changed_assertions.len() > 10 {
            out += &format!(
                "\n… and {} more changed assertions.\n",
                changed_assertions.len() - 10
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
                println!("    Details : {}", reason.dimmed());
                if let Some(old_err) = &r.old_contract_broken_on_head {
                    println!(
                        "    {} Old (base) assertion fails on head code: {}",
                        "⚠️  CONTRACT CHANGED:".bold().yellow(),
                        old_err.yellow()
                    );
                }
                println!();
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
    let mut contract_broken_on_head = 0;

    for r in results {
        let (verdict_str, details_str) = match &r.status {
            VerificationStatus::Discriminating { reason } => {
                discriminating += 1;
                let mut detail = reason.replace('|', "\\|").replace('\n', " ");
                if let Some(old_err) = &r.old_contract_broken_on_head {
                    contract_broken_on_head += 1;
                    detail.push_str(&format!(
                        " (⚠️ old assertion fails on head: {})",
                        old_err.replace('|', "\\|").replace('\n', " ")
                    ));
                }
                ("🛡️ **Catches change**", detail)
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
    if contract_broken_on_head > 0 {
        out += &format!(
            "- ⚠️ **Changed passing contract:** {} (base version of modified test fails on head code)\n",
            contract_broken_on_head
        );
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
        // 25 report findings across 3 files (~8 each) → each file aggregates to 1 row.
        let mut findings: Vec<Finding> = (0..25).map(|i| mk(i, RuleTier::Report)).collect();
        findings.push(mk(99, RuleTier::Guard));
        let empty = HashMap::new();
        let diff = proctor_diff::SemanticDiff::build(&empty, &empty).unwrap();
        let r = render_receipt(&diff, &findings);
        let rows: Vec<&str> = r
            .lines()
            .filter(|l| l.starts_with("| Guard") || l.starts_with("| Report"))
            .collect();
        assert_eq!(rows.len(), 4, "{r}");
        assert!(rows[0].starts_with("| Guard"));
        assert!(
            rows.iter()
                .any(|l| l.contains("assertion edits in `tests/t0.py`")),
            "{r}"
        );
    }

    #[test]
    fn receipt_aggregates_when_file_exceeds_five_findings() {
        use proctor_core::{RuleId, RuleTier, Verdict};
        let mut findings = Vec::new();
        for i in 0..7 {
            findings.push(Finding {
                id: format!("agg{i}"),
                rule_id: RuleId::AssertionLock,
                tier: RuleTier::Report,
                verdict: Verdict::Review {
                    finding_id: format!("agg{i}"),
                    evidence: String::new(),
                },
                file_path: "tests/test_big.py".to_string(),
                line_number: Some(i),
                test_name: None,
                message: format!("assertion edit {i}"),
                evidence: String::new(),
            });
        }
        // One more finding in a different file stays unaggregated.
        findings.push(Finding {
            id: "solo".into(),
            rule_id: RuleId::AssertionLock,
            tier: RuleTier::Report,
            verdict: Verdict::Review {
                finding_id: "solo".into(),
                evidence: String::new(),
            },
            file_path: "tests/test_solo.py".to_string(),
            line_number: Some(1),
            test_name: None,
            message: "solo edit".into(),
            evidence: String::new(),
        });
        let empty = HashMap::new();
        let diff = proctor_diff::SemanticDiff::build(&empty, &empty).unwrap();
        let r = render_receipt(&diff, &findings);
        assert!(
            r.contains("7 assertion edits in `tests/test_big.py`"),
            "{r}"
        );
        assert!(r.contains("tests/test_solo.py"), "{r}");
        let report_rows = r.lines().filter(|l| l.starts_with("| Report")).count();
        assert_eq!(report_rows, 2, "{r}");
    }

    #[test]
    fn receipt_notes_parse_errors_quietly() {
        let mut diff =
            proctor_diff::SemanticDiff::build(&HashMap::new(), &HashMap::new()).unwrap();
        diff.parse_errors.push((
            "vendor/huge_generated.py".to_string(),
            "Syntax error detected in test file".to_string(),
        ));
        let r = render_receipt(&diff, &[]);
        assert!(r.contains("Not analyzed (syntax/parse limit)"), "{r}");
        assert!(r.contains("vendor/huge_generated.py"), "{r}");
        assert!(r.contains("ℹ️ skipped:"), "{r}");
    }

    #[test]
    fn receipt_records_coevolution_modification_without_review_finding() {
        let base = HashMap::from([
            (
                "app/cart.py".to_string(),
                "def total(x):\n    return x\n".to_string(),
            ),
            (
                "tests/test_cart.py".to_string(),
                "def test_total():\n    assert total(10) == 10\n".to_string(),
            ),
        ]);
        let head = HashMap::from([
            (
                "app/cart.py".to_string(),
                "def total(x):\n    return x * 2\n".to_string(),
            ),
            (
                "tests/test_cart.py".to_string(),
                "def test_total():\n    assert total(10) == 20\n".to_string(),
            ),
        ]);
        let diff = proctor_diff::SemanticDiff::build(&base, &head).unwrap();
        let findings = proctor_rules::evaluate_rules(
            &diff,
            &proctor_core::Policy::default(),
            &proctor_core::WaiverSet::default(),
        );
        assert!(findings.is_empty(), "Expected no findings: {findings:?}");
        let r = render_receipt(&diff, &findings);
        assert!(r.contains("1 changed without findings, 0 flagged"), "{r}");
    }

    #[test]
    fn receipt_groups_deleted_tests_and_caps_guard_rows() {
        use proctor_core::{RuleId, RuleTier, Verdict};
        let mut findings = Vec::new();
        for file_idx in 0..30 {
            for t_idx in 0..10 {
                findings.push(Finding {
                    id: format!("del_{file_idx}_{t_idx}"),
                    rule_id: RuleId::AssertionLock,
                    tier: RuleTier::Guard,
                    verdict: Verdict::Block {
                        rule: RuleId::AssertionLock,
                        evidence: "deleted test".to_string(),
                    },
                    file_path: format!("tests/test_f{file_idx:02}.py"),
                    line_number: None,
                    test_name: Some(format!("tests/test_f{file_idx:02}.py::test_{t_idx}")),
                    message: "Test removed".to_string(),
                    evidence: format!("Deleted test: tests/test_f{file_idx:02}.py::test_{t_idx}"),
                });
            }
        }
        let empty = HashMap::new();
        let diff = proctor_diff::SemanticDiff::build(&empty, &empty).unwrap();
        let r = render_receipt(&diff, &findings);
        let guard_rows = r.lines().filter(|l| l.starts_with("| Guard")).count();
        assert_eq!(guard_rows, 25, "{r}");
        assert!(r.contains("10 tests deleted in `tests/test_f00.py`"), "{r}");
        assert!(
            r.contains("5 more guard entries (300 total guard findings"),
            "{r}"
        );
    }
}
