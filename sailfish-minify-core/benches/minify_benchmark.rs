use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sailfish_minify_core::{minify_file_and_components, MinifyOptions};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;

fn generate_html_of_size(size: usize) -> String {
    let unit = "<div class=\"row\">\n  <p>Lorem ipsum dolor sit amet consectetur adipiscing elit.</p>\n  <span>Item</span>\n</div>\n";
    let mut s = String::with_capacity(size);
    s.push_str("<!DOCTYPE html><html><head><title>Bench</title></head><body>\n");
    let mut remaining = size;
    while remaining > unit.len() {
        s.push_str(unit);
        remaining -= unit.len();
    }
    for _ in 0..remaining {
        s.push(' ');
    }
    s.push_str("\n</body></html>");
    s
}

fn minify_via_cli(html: &str) {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.html");
    let output = dir.path().join("out.html");
    std::fs::write(&input, html).unwrap();
    let result = Command::new(sailfish_minify_core::resolve_command("html-minifier"))
        .args(["--collapse-whitespace", input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(result.status.success());
}

fn minify_via_native(html: &str) {
    // Rust-native baseline: collapse runs of ASCII whitespace into a single space.
    // A true native minifier (e.g. minify-html) is not implemented yet.
    let minified: String = html.split_whitespace().collect::<Vec<_>>().join(" ");
    black_box(minified);
}

fn compile_with_include_depth(depth: usize) {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..depth {
        let content = if i == 0 {
            "<div>leaf</div>".to_string()
        } else {
            format!("<section><% include!(\"n{}.stpl\"); %></section>", i - 1)
        };
        std::fs::write(dir.path().join(format!("n{}.stpl", i)), content).unwrap();
    }
    std::fs::write(
        dir.path().join("main.stpl"),
        format!("<main><% include!(\"n{}.stpl\"); %></main>", depth - 1),
    )
    .unwrap();
    minify_file_and_components(
        &dir.path().join("main.stpl"),
        &dir.path().join("out.min"),
        &MinifyOptions::default(),
    )
    .unwrap();
}

fn compile_with_n_includes(count: usize) {
    let dir = tempfile::tempdir().unwrap();
    let mut main = String::from("<main>");
    for i in 0..count {
        std::fs::write(
            dir.path().join(format!("s{}.stpl", i)),
            format!("<div>sibling {}</div>", i),
        )
        .unwrap();
        main.push_str(&format!("<% include!(\"s{}.stpl\"); %>", i));
    }
    main.push_str("</main>");
    std::fs::write(dir.path().join("main.stpl"), &main).unwrap();
    minify_file_and_components(
        &dir.path().join("main.stpl"),
        &dir.path().join("out.min"),
        &MinifyOptions::default(),
    )
    .unwrap();
}

static FRESH_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn compile_fresh_template() {
    let dir = tempfile::tempdir().unwrap();
    let n = FRESH_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = dir.path().join(format!("fresh{}.stpl", n));
    std::fs::write(&path, "<html><body><h1>fresh</h1></body></html>").unwrap();
    minify_file_and_components(
        &path,
        &dir.path().join(format!("out{}.min", n)),
        &MinifyOptions::default(),
    )
    .unwrap();
}

static CACHED_TEMPLATE: LazyLock<PathBuf> = LazyLock::new(|| {
    let base = std::env::temp_dir().join("sailfish-minify-bench-cache");
    std::fs::create_dir_all(&base).unwrap();
    let path = base.join("cached.stpl");
    std::fs::write(&path, "<html><body><h1>cached</h1></body></html>").unwrap();
    path
});

fn compile_cached_template() {
    let path = CACHED_TEMPLATE.clone();
    minify_file_and_components(
        &path,
        &path.with_extension("cached.min"),
        &MinifyOptions::default(),
    )
    .unwrap();
}

fn bench_template_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_size");
    let sizes = [0, 1024, 102_400, 1_048_576, 10_485_760]; // 0, 1KB, 100KB, 1MB, 10MB

    for size in sizes.iter() {
        let html = generate_html_of_size(*size);
        group.bench_with_input(BenchmarkId::new("minify-html-cli", size), &html, |b, html| {
            b.iter(|| minify_via_cli(black_box(html)))
        });
        group.bench_with_input(BenchmarkId::new("minify-html-native", size), &html, |b, html| {
            b.iter(|| minify_via_native(black_box(html)))
        });
    }
    group.finish();
}

fn bench_include_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("include_depth");
    for depth in [1, 5, 10, 20, 50].iter() {
        group.bench_with_input(BenchmarkId::new("depth", depth), depth, |b, &depth| {
            b.iter(|| compile_with_include_depth(black_box(depth)))
        });
    }
    group.finish();
}

fn bench_parallel_includes(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_includes");
    for count in [1, 10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("count", count), count, |b, &count| {
            b.iter(|| compile_with_n_includes(black_box(count)))
        });
    }
    group.finish();
}

fn bench_cache_hit_vs_miss(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache");
    group.bench_function("cache_miss_first_compile", |b| {
        b.iter(compile_fresh_template)
    });
    group.bench_function("cache_hit_second_compile", |b| {
        compile_fresh_template(); // prime cache
        b.iter(compile_cached_template)
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_template_sizes,
    bench_include_depth,
    bench_parallel_includes,
    bench_cache_hit_vs_miss
);
criterion_main!(benches);
