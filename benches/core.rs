//! 核心路径基准测试（criterion，黑盒 CLI 方式）。
//!
//! bcr 是纯 bin crate，benches 无法直接 import 内部模块；这里通过
//! `CARGO_BIN_EXE_bcr`（cargo bench 提供）调用构建好的二进制做端到端计时，
//! 覆盖文本 diff、文件夹对比、CSV 对比、同步计划四条核心路径。
//!
//! 运行：`cargo bench --bench core`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::fmt::Write as _;
use std::process::Command;
use tempfile::tempdir;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_bcr")
}

/// 构造 n 行的文本（每行含序号，模拟真实文件）
fn make_text(n: usize) -> String {
    let mut s = String::with_capacity(n * 40);
    for i in 0..n {
        let _ = writeln!(
            s,
            "line {i:08} value={i} payload-abcdefghijklmnopqrstuvwxyz"
        );
    }
    s
}

/// 构造 n 个文件的目录，每个文件 m 行（两侧内容不同以产生差异）
fn make_dir(root: &std::path::Path, n: usize, m: usize, prefix: &str, mutate: bool) {
    for i in 0..n {
        let p = root.join(format!("{prefix}file{i:04}.txt"));
        let mut text = make_text(m);
        if mutate && i % 2 == 0 {
            text.push_str("// modified\n");
        }
        std::fs::write(&p, text).unwrap();
    }
}

/// 黑盒执行 CLI，返回进程退出码（0=无差异 1=有差异，均视为成功）
fn run(args: &[&str]) -> i32 {
    let out = Command::new(bin())
        .args(args)
        .output()
        .expect("failed to run bcr");
    assert!(out.status.code().is_some(), "bcr crashed");
    out.status.code().unwrap()
}

fn bench_text_diff(c: &mut Criterion) {
    let d = tempdir().unwrap();
    let (pa, pb) = (d.path().join("a.txt"), d.path().join("b.txt"));
    let mut g = c.benchmark_group("diff");
    for n in [1_000usize, 10_000, 50_000] {
        let a = make_text(n);
        let b = format!("{}{}", make_text(n / 2), make_text(n - n / 2));
        std::fs::write(&pa, &a).unwrap();
        std::fs::write(&pb, &b).unwrap();
        g.bench_with_input(BenchmarkId::new("lines", n), &(), |bencher, _| {
            bencher.iter(|| {
                let _ = run(&["diff", pa.to_str().unwrap(), pb.to_str().unwrap()]);
            });
        });
    }
    g.finish();
}

fn bench_dir_compare(c: &mut Criterion) {
    let mut g = c.benchmark_group("compare");
    for n in [100usize, 1_000, 5_000] {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_dir(d1.path(), n, 20, "a", false);
        make_dir(d2.path(), n, 20, "a", true);
        g.bench_with_input(
            BenchmarkId::new("files", n),
            &(d1.path(), d2.path()),
            |bencher, (l, r)| {
                bencher.iter(|| {
                    let _ = run(&["compare", l.to_str().unwrap(), r.to_str().unwrap()]);
                });
            },
        );
    }
    g.finish();
}

fn bench_csv_align(c: &mut Criterion) {
    let d = tempdir().unwrap();
    let (pa, pb) = (d.path().join("a.csv"), d.path().join("b.csv"));
    let mut g = c.benchmark_group("csv");
    for n in [1_000usize, 10_000] {
        let a = (0..n)
            .map(|i| format!("{i},name-{i},payload-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let b = (0..n)
            .map(|i| {
                format!(
                    "{i},name-{i},payload-{}",
                    if i % 3 == 0 { i + 1 } else { i }
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&pa, &a).unwrap();
        std::fs::write(&pb, &b).unwrap();
        g.bench_with_input(BenchmarkId::new("rows", n), &(), |bencher, _| {
            bencher.iter(|| {
                let _ = run(&["csv", pa.to_str().unwrap(), pb.to_str().unwrap()]);
            });
        });
    }
    g.finish();
}

fn bench_sync_plan(c: &mut Criterion) {
    let mut g = c.benchmark_group("sync");
    for n in [100usize, 1_000] {
        let d1 = tempdir().unwrap();
        let d2 = tempdir().unwrap();
        make_dir(d1.path(), n, 10, "a", false);
        make_dir(d2.path(), n, 10, "a", true);
        g.bench_with_input(
            BenchmarkId::new("files", n),
            &(d1.path(), d2.path()),
            |bencher, (l, r)| {
                bencher.iter(|| {
                    let _ = run(&[
                        "sync",
                        "--dry-run",
                        l.to_str().unwrap(),
                        r.to_str().unwrap(),
                    ]);
                });
            },
        );
    }
    g.finish();
}

criterion_group!(
    benches,
    bench_text_diff,
    bench_dir_compare,
    bench_csv_align,
    bench_sync_plan
);
criterion_main!(benches);
