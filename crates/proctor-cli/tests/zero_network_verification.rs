//! Verification test ensuring that agent-proctor contains zero networking dependencies
//! or egress capabilities, guaranteeing that no source code, tokens, or telemetry can leave the runner.

use std::fs;
use std::path::Path;

#[test]
fn test_zero_network_dependencies_in_lockfile() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lockfile_path = Path::new(manifest_dir).join("../../Cargo.lock");

    let lock_content =
        fs::read_to_string(&lockfile_path).expect("Cargo.lock must exist at workspace root");

    // Forbidden networking and telemetry crates
    let forbidden_crates = [
        "name = \"reqwest\"",
        "name = \"hyper\"",
        "name = \"curl\"",
        "name = \"tungstenite\"",
        "name = \"tokio-tungstenite\"",
        "name = \"native-tls\"",
        "name = \"rustls\"",
        "name = \"ureq\"",
        "name = \"isahc\"",
        "name = \"surf\"",
        "name = \"h2\"",
    ];

    for forbidden in &forbidden_crates {
        assert!(
            !lock_content.contains(forbidden),
            "CRITICAL: Forbidden network dependency detected in Cargo.lock: {}. Proctor must maintain a strict zero-egress architecture.",
            forbidden
        );
    }
}
