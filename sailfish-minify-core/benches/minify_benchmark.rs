use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sailfish_minify_core::{extract_includes_manual, minify_file_and_components, MinifyOptions};
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

#[cfg(feature = "native-minifier")]
fn minify_via_native(html: &str) {
    // Pure-Rust minification via minify-html: in-memory, no process spawn.
    let minified =
        minify_html::minify(html.as_bytes(), &sailfish_minify_core::native_minify_cfg());
    black_box(minified);
}

#[cfg(not(feature = "native-minifier"))]
fn minify_via_native_placeholder(html: &str) {
    // Fallback when the `native-minifier` feature is disabled: no-op passthrough.
    black_box(html.as_bytes().to_vec());
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
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(BenchmarkId::new("minify-html-cli", size), &html, |b, html| {
            b.iter(|| minify_via_cli(black_box(html)))
        });
        #[cfg(feature = "native-minifier")]
        group.bench_with_input(BenchmarkId::new("minify-html-native", size), &html, |b, html| {
            b.iter(|| minify_via_native(black_box(html)))
        });
        #[cfg(not(feature = "native-minifier"))]
        group.bench_with_input(
            BenchmarkId::new("native-disabled-noop", size),
            &html,
            |b, html| b.iter(|| minify_via_native_placeholder(black_box(html))),
        );
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

fn generate_template_with_includes(count: usize) -> String {
    let mut s = String::from("<main>\n");
    for i in 0..count {
        s.push_str(&format!(
            "<section class=\"s\">\n  <p>text {}</p>\n  <% include!(\"comp{}.stpl\"); %>\n</section>\n",
            i, i
        ));
    }
    s.push_str("</main>\n");
    s
}

fn bench_include_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("include_parse");
    for count in [0, 1, 10, 100, 1000].iter() {
        let template = generate_template_with_includes(*count);
        group.throughput(Throughput::Bytes(template.len() as u64));
        group.bench_with_input(BenchmarkId::new("manual", count), &template, |b, t| {
            b.iter(|| black_box(extract_includes_manual(black_box(t)).len()))
        });
        #[cfg(feature = "regex")]
        group.bench_with_input(BenchmarkId::new("regex", count), &template, |b, t| {
            b.iter(|| black_box(sailfish_minify_core::extract_includes(black_box(t)).len()))
        });
    }
    group.finish();
}

/// A single repeating HTML chunk carrying 3 includes (~220 bytes each). A
/// large file is just many of these, giving a dense include workload that
/// stresses raw scan throughput rather than per-include call overhead.
const LARGE_CHUNK: &str = "<section class=\"item\">\n  <h2>Heading text</h2>\n  <p>Some body copy to pad the file out to a realistic size.</p>\n  <% include!(\"a.stpl\"); %>\n  <% include!(\"b.stpl\"); %>\n  <% include!(\"c.stpl\"); %>\n</section>\n";

fn generate_large_template_with_includes(target_bytes: usize) -> String {
    let mut s = String::with_capacity(target_bytes);
    while s.len() < target_bytes {
        s.push_str(LARGE_CHUNK);
    }
    s
}

fn bench_include_parse_large(c: &mut Criterion) {
    let mut group = c.benchmark_group("include_parse_large");
    for size in [1_048_576, 5_242_880, 10_485_760].iter() {
        // 1 MiB / 5 MiB / 10 MiB, each with ~3 includes per ~220-byte chunk.
        let template = generate_large_template_with_includes(*size);
        let include_count = extract_includes_manual(&template).len();
        group.throughput(Throughput::Bytes(template.len() as u64));
        group.bench_with_input(BenchmarkId::new("manual", size), &template, |b, t| {
            b.iter(|| black_box(extract_includes_manual(black_box(t)).len()))
        });
        #[cfg(feature = "regex")]
        group.bench_with_input(BenchmarkId::new("regex", size), &template, |b, t| {
            b.iter(|| black_box(sailfish_minify_core::extract_includes(black_box(t)).len()))
        });
        println!("include_parse_large: {} MiB input -> {} include matches", size / 1024 / 1024, include_count);
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_template_sizes,
    bench_include_depth,
    bench_parallel_includes,
    bench_cache_hit_vs_miss,
    bench_include_parse,
    bench_include_parse_large
);
criterion_main!(benches);
