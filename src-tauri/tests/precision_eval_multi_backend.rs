//! Multi-backend parity eval — Rust-side gate.
//!
//! Pairs with `scripts/eval-multi-backend-parity.ps1`. The PS1 runner is
//! the full-fidelity harness (markdown reports, cross-backend seam
//! parity, regression modes); this integration test is the CI-cargo
//! entry point that enforces the same per-backend boundary-error
//! thresholds against each committed backend output.
//!
//! Fixture layout (see `.github/skills/transcription-adapter-contract/`):
//!
//! ```text
//! src-tauri/tests/fixtures/parity/
//!   <stem>.wav
//!   <stem>.oracle.json            [{word,start_us,end_us}, ...]
//!   backend_outputs/<backend>/<stem>.result.json   NormalizedTranscriptionResult
//! ```
//!
//! Gates (mirror AGENTS.md precision guardrails):
//!
//! - G1  median boundary error <= 20 000 us per backend
//! - G2  p95    boundary error <= 40 000 us per backend
//!
//! Cross-backend seam / duration parity is tested in the PS1 runner
//! (it requires comparing two backends at once with keep-segment math
//! that the editor owns). Wiring a Rust-side equivalent to the
//! editor keep-segment path is tracked for `eval-harness-runner`.
//!
//! If a backend has no cached result for a fixture, the test logs a
//! `skip` and passes — CI enforcement of "must have all backends" is
//! the PS1 runner's `-StrictMode`. Keeping the Rust test soft on
//! skips avoids breaking `cargo test` for developers who haven't
//! cached every backend locally.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const MEDIAN_THRESHOLD_US: i64 = 20_000;
const P95_THRESHOLD_US: i64 = 40_000;

#[derive(Debug, Clone)]
struct Word {
    text: String,
    start_us: i64,
    end_us: i64,
}

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("parity")
}

fn read_oracle(path: &Path) -> Vec<Word> {
    let text = fs::read_to_string(path).expect("read oracle json");
    let arr: Value = serde_json::from_str(&text).expect("parse oracle json");
    arr.as_array()
        .expect("oracle is array")
        .iter()
        .map(|w| Word {
            text: w["word"].as_str().unwrap_or("").to_string(),
            start_us: w["start_us"].as_i64().unwrap_or(0),
            end_us: w["end_us"].as_i64().unwrap_or(0),
        })
        .collect()
}

fn read_backend_result(path: &Path) -> Vec<Word> {
    let text = fs::read_to_string(path).expect("read backend result");
    let v: Value = serde_json::from_str(&text).expect("parse backend result");
    v["words"]
        .as_array()
        .expect("result.words array")
        .iter()
        .map(|w| Word {
            text: w["text"].as_str().unwrap_or("").to_string(),
            start_us: w["start_us"].as_i64().unwrap_or(0),
            end_us: w["end_us"].as_i64().unwrap_or(0),
        })
        .collect()
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Levenshtein-anchored word alignment. Returns (ref_index, hyp_index)
/// pairs for exact-text matches only; substitutions/insertions/deletions
/// are dropped from the boundary-error computation (they are counted and
/// reported by the PS1 runner — a high sub/ins/del rate is itself a
/// reason to fail, but not one these gates enforce).
fn align(oracle: &[Word], hyp: &[Word]) -> Vec<(usize, usize)> {
    let m = oracle.len();
    let n = hyp.len();
    let ref_n: Vec<String> = oracle.iter().map(|w| normalize(&w.text)).collect();
    let hyp_n: Vec<String> = hyp.iter().map(|w| normalize(&w.text)).collect();

    let mut d = vec![vec![0i32; n + 1]; m + 1];
    #[allow(clippy::needless_range_loop)]
    for i in 0..=m {
        d[i][0] = i as i32;
    }
    #[allow(clippy::needless_range_loop)]
    for j in 0..=n {
        d[0][j] = j as i32;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = if ref_n[i - 1] == hyp_n[j - 1] { 0 } else { 1 };
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
        }
    }

    let mut pairs = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        let cur = d[i][j];
        if i > 0 && j > 0 && ref_n[i - 1] == hyp_n[j - 1] && cur == d[i - 1][j - 1] {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if i > 0 && j > 0 && cur == d[i - 1][j - 1] + 1 {
            i -= 1;
            j -= 1;
        } else if j > 0 && cur == d[i][j - 1] + 1 {
            j -= 1;
        } else {
            i -= 1;
        }
    }
    pairs.reverse();
    pairs
}

fn percentile(sorted: &[i64], p: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let n = sorted.len() as f64;
    let idx = (((p / 100.0) * n).ceil() as isize - 1).max(0) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

struct Stats {
    median_us: i64,
    p95_us: i64,
    matched: usize,
}

fn score(oracle: &[Word], hyp: &[Word]) -> Stats {
    let matches = align(oracle, hyp);
    let mut errs = Vec::with_capacity(matches.len() * 2);
    for &(ri, hi) in &matches {
        errs.push((hyp[hi].start_us - oracle[ri].start_us).abs());
        errs.push((hyp[hi].end_us - oracle[ri].end_us).abs());
    }
    errs.sort_unstable();
    Stats {
        median_us: percentile(&errs, 50.0),
        p95_us: percentile(&errs, 95.0),
        matched: matches.len(),
    }
}

fn list_fixtures() -> Vec<(String, PathBuf)> {
    let root = fixtures_root();
    if !root.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&root).expect("read parity fixtures root") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wav") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let oracle = root.join(format!("{stem}.oracle.json"));
        if oracle.exists() {
            out.push((stem, oracle));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn list_backends() -> Vec<String> {
    let dir = fixtures_root().join("backend_outputs");
    if !dir.exists() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).expect("read backend_outputs") {
        let entry = entry.expect("dir entry");
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(name) = entry.file_name().to_str() {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

#[test]
fn parity_eval_boundary_error_within_thresholds() {
    let fixtures = list_fixtures();
    if fixtures.is_empty() {
        eprintln!(
            "parity_eval: no fixtures under {:?}; regenerate with scripts/generate-parity-fixtures.ps1",
            fixtures_root()
        );
        return;
    }
    let backends = list_backends();
    if backends.is_empty() {
        eprintln!(
            "parity_eval: no cached backend outputs; see backend_outputs/README.md. Skipping."
        );
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut scored = 0usize;

    for (stem, oracle_path) in &fixtures {
        let oracle = read_oracle(oracle_path);
        for backend in &backends {
            let result_path = fixtures_root()
                .join("backend_outputs")
                .join(backend)
                .join(format!("{stem}.result.json"));
            if !result_path.exists() {
                eprintln!("parity_eval: skip {stem}/{backend} (no result.json)");
                continue;
            }
            let hyp = read_backend_result(&result_path);
            let stats = score(&oracle, &hyp);
            scored += 1;
            eprintln!(
                "parity_eval: {stem:<14} {backend:<10} matched={m:>2} median={md:>7}us p95={p95:>7}us",
                stem = stem,
                backend = backend,
                m = stats.matched,
                md = stats.median_us,
                p95 = stats.p95_us
            );
            if stats.matched == 0 {
                failures.push(format!(
                    "{stem}/{backend}: zero matched words (text mismatch?)"
                ));
                continue;
            }
            if stats.median_us > MEDIAN_THRESHOLD_US {
                failures.push(format!(
                    "{stem}/{backend}: median {}us > G1 {}us",
                    stats.median_us, MEDIAN_THRESHOLD_US
                ));
            }
            if stats.p95_us > P95_THRESHOLD_US {
                failures.push(format!(
                    "{stem}/{backend}: p95 {}us > G2 {}us",
                    stats.p95_us, P95_THRESHOLD_US
                ));
            }
        }
    }

    assert!(
        scored > 0,
        "no (fixture, backend) pairs scored — check backend_outputs/"
    );
    assert!(
        failures.is_empty(),
        "multi-backend parity gates failed:\n  - {}",
        failures.join("\n  - ")
    );
}

#[test]
fn parity_eval_alignment_matches_by_text_not_position() {
    // Sanity: if hypothesis inserts an extra word at the front, the
    // aligner must still pair every remaining word by text.
    let oracle = vec![
        Word {
            text: "hello".into(),
            start_us: 0,
            end_us: 400_000,
        },
        Word {
            text: "world".into(),
            start_us: 500_000,
            end_us: 900_000,
        },
        Word {
            text: "toaster".into(),
            start_us: 950_000,
            end_us: 1_500_000,
        },
    ];
    let hyp = vec![
        Word {
            text: "um".into(),
            start_us: 0,
            end_us: 100_000,
        },
        Word {
            text: "hello".into(),
            start_us: 100_000,
            end_us: 490_000,
        },
        Word {
            text: "world".into(),
            start_us: 500_000,
            end_us: 905_000,
        },
        Word {
            text: "toaster".into(),
            start_us: 960_000,
            end_us: 1_495_000,
        },
    ];
    let pairs = align(&oracle, &hyp);
    assert_eq!(
        pairs,
        vec![(0, 1), (1, 2), (2, 3)],
        "aligner must match by text, skipping the inserted 'um'"
    );
}

#[test]
fn parity_eval_equal_duration_synthesis_fails_gates() {
    // Negative test: a backend that spreads all words evenly across the
    // utterance (classic char-split synthesis regression) must blow past
    // both thresholds.
    let oracle = vec![
        Word {
            text: "hello".into(),
            start_us: 0,
            end_us: 420_000,
        },
        Word {
            text: "world".into(),
            start_us: 500_000,
            end_us: 860_000,
        },
        Word {
            text: "this".into(),
            start_us: 980_000,
            end_us: 1_260_000,
        },
        Word {
            text: "is".into(),
            start_us: 1_360_000,
            end_us: 1_560_000,
        },
        Word {
            text: "toaster".into(),
            start_us: 1_650_000,
            end_us: 2_190_000,
        },
    ];
    let total = oracle.last().unwrap().end_us - oracle.first().unwrap().start_us;
    let each = total / oracle.len() as i64;
    let hyp: Vec<Word> = oracle
        .iter()
        .enumerate()
        .map(|(k, w)| Word {
            text: w.text.clone(),
            start_us: (k as i64) * each,
            end_us: ((k + 1) as i64) * each,
        })
        .collect();
    let stats = score(&oracle, &hyp);
    assert!(
        stats.median_us > MEDIAN_THRESHOLD_US || stats.p95_us > P95_THRESHOLD_US,
        "equal-duration synthesis should fail one of the gates (median={}us, p95={}us)",
        stats.median_us,
        stats.p95_us
    );
}
