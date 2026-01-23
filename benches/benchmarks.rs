//! Benchmark suite for Ralph subsystems.
//!
//! This module provides performance benchmarks for:
//! - Gate execution (quality checks)
//! - Language detection (file scanning)
//! - Context building (prompt assembly)
//!
//! # Running Benchmarks
//!
//! ```bash
//! # Run all benchmarks
//! cargo bench
//!
//! # Save baseline for comparison
//! cargo bench -- --save-baseline main
//!
//! # Compare against baseline
//! cargo bench -- --baseline main
//! ```
//!
//! # Machine-Readable Output
//!
//! Criterion automatically produces JSON output in `target/criterion/`.
//! Each benchmark group has its own directory with:
//! - `new/estimates.json` - Statistical estimates
//! - `new/sample.json` - Raw sample data
//! - `report/index.html` - HTML report

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Gate Execution Benchmarks
// ============================================================================

/// Benchmark gate execution time.
///
/// Measures the time taken to run the NoAllowGate on projects of various sizes.
/// This gate is chosen because it's fast and doesn't require external tools.
fn bench_gate_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_execution");

    // Create test projects of different sizes
    for size in [10, 50, 100] {
        let temp_dir = create_rust_project_with_files(size);
        let project_path = temp_dir.path().to_path_buf();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(
            BenchmarkId::new("no_allow_gate", size),
            &project_path,
            |b, path| {
                use ralph::quality::gates::Gate;
                use ralph::quality::NoAllowGate;

                b.iter(|| {
                    let gate = NoAllowGate::new(black_box(path));
                    black_box(gate.check())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parallel vs sequential gate execution.
///
/// Compares the performance of running multiple gates in parallel vs sequentially.
fn bench_parallel_gate_execution(c: &mut Criterion) {
    use ralph::quality::gates::{GateIssue, QualityGate};
    use ralph::quality::{run_gates_parallel, EnforcerConfig};
    use std::sync::Arc;

    let mut group = c.benchmark_group("parallel_vs_sequential");

    // Create a simple mock gate that does minimal work
    struct MockGate {
        name: String,
    }

    impl QualityGate for MockGate {
        fn name(&self) -> &str {
            &self.name
        }

        fn run(&self, _project_dir: &std::path::Path) -> anyhow::Result<Vec<GateIssue>> {
            // Simulate some work
            std::thread::sleep(std::time::Duration::from_micros(100));
            Ok(vec![])
        }

        fn remediation(&self, _issues: &[GateIssue]) -> String {
            String::new()
        }
    }

    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path().to_path_buf();

    // Create runtime for async benchmarks
    let rt = tokio::runtime::Runtime::new().unwrap();

    for gate_count in [2, 4, 8] {
        let gates: Vec<Arc<dyn QualityGate>> = (0..gate_count)
            .map(|i| {
                Arc::new(MockGate {
                    name: format!("MockGate{}", i),
                }) as Arc<dyn QualityGate>
            })
            .collect();

        // Benchmark parallel execution
        let config_parallel = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(true);

        let gates_clone = gates.clone();
        let path_clone = project_path.clone();
        group.bench_function(BenchmarkId::new("parallel", gate_count), |b| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(
                        run_gates_parallel(
                            black_box(&gates_clone),
                            black_box(&path_clone),
                            black_box(&config_parallel),
                        )
                        .await,
                    )
                })
            });
        });

        // Benchmark sequential execution
        let config_sequential = EnforcerConfig::new()
            .with_clippy(false)
            .with_tests(false)
            .with_security(false)
            .with_no_allow(false)
            .with_parallel_gates(false);

        let gates_clone = gates.clone();
        let path_clone = project_path.clone();
        group.bench_function(BenchmarkId::new("sequential", gate_count), |b| {
            b.iter(|| {
                rt.block_on(async {
                    black_box(
                        run_gates_parallel(
                            black_box(&gates_clone),
                            black_box(&path_clone),
                            black_box(&config_sequential),
                        )
                        .await,
                    )
                })
            });
        });
    }

    group.finish();
}

// ============================================================================
// Language Detection Benchmarks
// ============================================================================

/// Benchmark language detection time.
///
/// Measures the time taken to detect languages in projects of various sizes.
fn bench_language_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("language_detection");

    // Test with different project sizes
    for file_count in [10, 50, 100, 200] {
        let temp_dir = create_polyglot_project(file_count);
        let project_path = temp_dir.path().to_path_buf();

        group.throughput(Throughput::Elements(file_count as u64));
        group.bench_with_input(
            BenchmarkId::new("detect", file_count),
            &project_path,
            |b, path| {
                use ralph::bootstrap::language_detector::LanguageDetector;

                b.iter(|| {
                    let detector = LanguageDetector::new(black_box(path));
                    black_box(detector.detect())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark polyglot detection.
///
/// Measures the time to determine if a project is polyglot (multiple languages).
fn bench_polyglot_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("polyglot_detection");

    // Single language project
    let single_lang = create_rust_project_with_files(50);
    group.bench_function("single_language", |b| {
        use ralph::bootstrap::language_detector::LanguageDetector;

        b.iter(|| {
            let detector = LanguageDetector::new(black_box(single_lang.path()));
            black_box(detector.is_polyglot())
        });
    });

    // Polyglot project
    let polyglot = create_polyglot_project(50);
    group.bench_function("polyglot_project", |b| {
        use ralph::bootstrap::language_detector::LanguageDetector;

        b.iter(|| {
            let detector = LanguageDetector::new(black_box(polyglot.path()));
            black_box(detector.is_polyglot())
        });
    });

    group.finish();
}

// ============================================================================
// Context Building Benchmarks
// ============================================================================

/// Benchmark context building time.
///
/// Measures the time taken to build a PromptContext from assembler state.
fn bench_context_building(c: &mut Criterion) {
    use ralph::prompt::assembler::{AssemblerConfig, PromptAssembler};
    use ralph::prompt::context::{AttemptOutcome, ErrorSeverity, TaskPhase};
    use ralph::Language;

    let mut group = c.benchmark_group("context_building");

    // Benchmark with minimal context
    group.bench_function("minimal_context", |b| {
        let assembler = PromptAssembler::new();

        b.iter(|| black_box(assembler.build_context()));
    });

    // Benchmark with typical context
    group.bench_function("typical_context", |b| {
        let config = AssemblerConfig::new().with_languages(vec![Language::Rust]);
        let mut assembler = PromptAssembler::with_config(config);
        assembler.set_current_task("1.1", "Implement feature", TaskPhase::Implementation);
        assembler.update_session_stats(5, 2, 150);
        assembler.add_error("E0308", "mismatched types", ErrorSeverity::Error);

        b.iter(|| black_box(assembler.build_context()));
    });

    // Benchmark with full context (many errors, attempts, anti-patterns)
    group.bench_function("full_context", |b| {
        let config = AssemblerConfig::new().with_languages(vec![
            Language::Rust,
            Language::Python,
            Language::TypeScript,
        ]);
        let mut assembler = PromptAssembler::with_config(config);
        assembler.set_current_task("1.1", "Complex task", TaskPhase::Implementation);
        assembler.update_session_stats(20, 5, 500);

        // Add multiple errors
        for i in 0..10 {
            assembler.add_error(
                &format!("E{:04}", i),
                &format!("Error message {}", i),
                ErrorSeverity::Error,
            );
        }

        // Add attempts
        for i in 0..5 {
            assembler.record_attempt(
                AttemptOutcome::TestFailure,
                Some(&format!("Approach {}", i)),
                vec![format!("Error {}", i)],
            );
        }

        // Add iteration history for anti-pattern detection
        for i in 0..10 {
            assembler.record_iteration_with_files(i, vec![format!("src/file{}.rs", i)], i % 3 == 0);
        }

        b.iter(|| black_box(assembler.build_context()));
    });

    group.finish();
}

/// Benchmark prompt building time.
///
/// Measures the full prompt assembly including template rendering.
fn bench_prompt_building(c: &mut Criterion) {
    use ralph::prompt::assembler::{AssemblerConfig, PromptAssembler};
    use ralph::prompt::context::TaskPhase;
    use ralph::Language;

    let mut group = c.benchmark_group("prompt_building");

    for mode in ["build", "debug", "plan"] {
        let config = AssemblerConfig::new().with_languages(vec![Language::Rust]);
        let mut assembler = PromptAssembler::with_config(config);
        assembler.set_current_task("1.1", "Implement feature", TaskPhase::Implementation);
        assembler.update_session_stats(5, 2, 150);

        group.bench_function(mode, |b| {
            b.iter(|| black_box(assembler.build_prompt(black_box(mode))));
        });
    }

    group.finish();
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a Rust project with the specified number of source files.
fn create_rust_project_with_files(file_count: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create Cargo.toml
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "bench_project"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("Failed to write Cargo.toml");

    // Create src directory
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("Failed to create src dir");

    // Create lib.rs
    fs::write(
        src_dir.join("lib.rs"),
        "//! Benchmark test library\n\npub mod utils;\n",
    )
    .expect("Failed to write lib.rs");

    // Create source files
    for i in 0..file_count {
        let content = format!(
            r#"//! Module {}
/// Function that does something.
pub fn function_{}() -> i32 {{
    {}
}}
"#,
            i, i, i
        );
        fs::write(src_dir.join(format!("file_{}.rs", i)), content)
            .expect("Failed to write source file");
    }

    // Create utils module
    fs::write(
        src_dir.join("utils.rs"),
        "//! Utility functions\npub fn helper() -> bool { true }\n",
    )
    .expect("Failed to write utils.rs");

    temp_dir
}

/// Create a polyglot project with files in multiple languages.
fn create_polyglot_project(files_per_language: usize) -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create Rust files
    let rust_dir = temp_dir.path().join("rust_src");
    fs::create_dir_all(&rust_dir).expect("Failed to create rust dir");
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("Failed to write Cargo.toml");
    for i in 0..files_per_language {
        fs::write(
            rust_dir.join(format!("mod_{}.rs", i)),
            format!("pub fn rust_func_{}() {{}}", i),
        )
        .expect("Failed to write rust file");
    }

    // Create Python files
    let python_dir = temp_dir.path().join("python_src");
    fs::create_dir_all(&python_dir).expect("Failed to create python dir");
    fs::write(
        temp_dir.path().join("pyproject.toml"),
        "[project]\nname = \"test\"\nversion = \"0.1.0\"\n",
    )
    .expect("Failed to write pyproject.toml");
    for i in 0..files_per_language {
        fs::write(
            python_dir.join(format!("module_{}.py", i)),
            format!("def python_func_{}():\n    pass", i),
        )
        .expect("Failed to write python file");
    }

    // Create TypeScript files
    let ts_dir = temp_dir.path().join("ts_src");
    fs::create_dir_all(&ts_dir).expect("Failed to create ts dir");
    fs::write(
        temp_dir.path().join("package.json"),
        "{\"name\": \"test\", \"version\": \"1.0.0\"}",
    )
    .expect("Failed to write package.json");
    fs::write(
        temp_dir.path().join("tsconfig.json"),
        "{\"compilerOptions\": {}}",
    )
    .expect("Failed to write tsconfig.json");
    for i in 0..files_per_language {
        fs::write(
            ts_dir.join(format!("module_{}.ts", i)),
            format!("export function tsFunc{}() {{}}", i),
        )
        .expect("Failed to write ts file");
    }

    // Create Go files
    let go_dir = temp_dir.path().join("go_src");
    fs::create_dir_all(&go_dir).expect("Failed to create go dir");
    fs::write(temp_dir.path().join("go.mod"), "module test\n\ngo 1.21\n")
        .expect("Failed to write go.mod");
    for i in 0..files_per_language {
        fs::write(
            go_dir.join(format!("module_{}.go", i)),
            format!("package main\n\nfunc goFunc{}() {{}}", i),
        )
        .expect("Failed to write go file");
    }

    temp_dir
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    gate_benches,
    bench_gate_execution,
    bench_parallel_gate_execution
);

criterion_group!(
    language_benches,
    bench_language_detection,
    bench_polyglot_detection
);

criterion_group!(
    context_benches,
    bench_context_building,
    bench_prompt_building
);

criterion_main!(gate_benches, language_benches, context_benches);
