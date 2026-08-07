use quote::quote;
use rayon::prelude::*;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs::{create_dir_all, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::{fs, io};
use syn::ItemStruct;
use syn::Meta;

const INCLUDE_REPLACE_TOKEN_REGEX: &str = r#"<% *include!\s*\(\s*"([^"]+)"\s*\)\s*;\s*%>"#;

// Cache Regex compilation
static INCLUDE_REGEX_CACHE: OnceLock<Regex> = OnceLock::new();
static TEMPLATE_PATH_REGEX_CACHE: OnceLock<Regex> = OnceLock::new();

// Global cache to track processed components across all template compilations
//
// Only used when `minify-components` is enabled, but kept compiled in every
// configuration because the unit tests in `sailfish-minify-core` exercise it
// either way.
#[allow(dead_code)]
static GLOBAL_PROCESSED_CACHE: OnceLock<Mutex<HashMap<PathBuf, PathBuf>>> = OnceLock::new();

// Used by the unit tests to verify cache behavior.
#[doc(hidden)]
pub static MINIFIER_INVOCATIONS: AtomicUsize = AtomicUsize::new(0);

#[allow(dead_code)]
fn get_global_cache() -> &'static Mutex<HashMap<PathBuf, PathBuf>> {
    GLOBAL_PROCESSED_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn get_include_regex() -> &'static Regex {
    INCLUDE_REGEX_CACHE.get_or_init(|| Regex::new(INCLUDE_REPLACE_TOKEN_REGEX).unwrap())
}

fn get_template_path_regex() -> &'static Regex {
    TEMPLATE_PATH_REGEX_CACHE.get_or_init(|| {
        Regex::new(r#"#\[template\([^)]*path\s*=\s*"([^"]+)"[^)]*\)\]"#).unwrap()
    })
}

/// Base directory where minified templates are written.
pub fn tmp_main_path() -> PathBuf {
    std::env::temp_dir().join("sailfish-minify")
}

/// Resolve the name of a CLI tool to an absolute path on Windows.
///
/// `html-minifier` is installed through npm, which only creates `*.cmd` /
/// `*.ps1` shims. Rust's `std::process::Command` cannot execute those by bare
/// name, so we search `PATH` and hand back the full path instead.
pub fn resolve_command(name: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(path) = find_executable_in_path(name) {
            return path;
        }
    }
    name.to_string()
}

#[cfg(windows)]
fn find_executable_in_path(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path_var) {
        for ext in ["exe", "cmd", "bat", "ps1"] {
            let candidate = dir.join(format!("{name}.{ext}"));
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
        }
    }
    None
}

/// Extract the template file path from a `#[template(path = "...")]` attribute.
///
/// Relative paths are resolved against `./templates`, absolute paths are used
/// as-is.
pub fn extract_template_path(str: &str) -> syn::Result<PathBuf> {
    let template_regex = get_template_path_regex();

    if let Some(captures) = template_regex.captures(str) {
        let path = captures.get(1).expect("Cannot find path in template").as_str();
        let path = Path::new(path);
        if path.is_absolute() {
            Ok(path.to_path_buf())
        } else {
            Ok(Path::new("./templates").join(path))
        }
    } else {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "Cannot find template path in struct attributes. Make sure to use #[template(path = \"...\")] for minified templates",
        ))
    }
}

/// Compute the minified output path for a source template: the same relative
/// structure under `tmp_main_path()`, with a `.min` suffix on the file name.
pub fn modify_template_path(path: &Path) -> PathBuf {
    let mut new_path = tmp_main_path();

    if let Some(parent) = path.parent() {
        new_path.push(parent);
    }

    if let Some(file_name) = path.file_name() {
        new_path.push(format!("{}.min", file_name.to_str().unwrap()));
    }
    new_path
}

/// Rewrite the `#[template(path = "...")]` attribute to point at the minified file.
pub fn replace_path_attribute(input: proc_macro2::TokenStream, new_path: &str) -> proc_macro2::TokenStream {
    let mut struct_item = syn::parse2::<ItemStruct>(input).unwrap_or_else(|_| {
        panic!("sailfish-minify: expected a struct for #[derive(TemplateSimple)]")
    });

    for attr in &mut struct_item.attrs {
        if attr.path().is_ident("template") {
            let new_attr = syn::parse_quote! { #[template(path = #new_path)] };
            *attr = new_attr;
            break;
        }
    }

    quote! {
        #struct_item
    }
}

/// Extract the file names referenced by `<% include!("..."); %>` macros.
///
/// Only used by the `copy_referenced_template_and_includes` path (compiled when
/// `minify-components` is disabled) and by the unit tests in
/// `sailfish-minify-core`.
#[allow(dead_code)]
pub fn extract_includes(contents: &str) -> Vec<String> {
    get_include_regex()
        .captures_iter(contents)
        .map(|cap| cap[1].to_string())
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum Minifier {
    /// Minify with the `html-minifier` CLI (must be installed and on `PATH`).
    HTMLMinifier,
    Custom(String),
    CustomUnchecked(String),
    /// Pure-Rust minifier (`minify-html`), enabled by the `native-minifier`
    /// feature. No `Command` spawn, no system dependency, WASM-compatible.
    /// This is the default minifier.
    Native,
}

/// Build the `minify-html` configuration used by `Minifier::Native`.
///
/// Based on the spec-compliant preset so the output is safe, but tuned for
/// sailfish templates and for parity with the default `html-minifier
/// --collapse-whitespace` invocation: `<% ... %>` blocks are passed through
/// untouched, comments are kept, and optional closing tags are not omitted.
#[cfg(feature = "native-minifier")]
pub fn native_minify_cfg() -> minify_html::Cfg {
    let mut cfg = minify_html::Cfg::spec_compliant();
    cfg.preserve_chevron_percent_template_syntax = true;
    cfg.keep_comments = true;
    cfg.keep_closing_tags = true;
    cfg
}

#[derive(Debug, Clone, PartialEq)]
pub struct MinifyOptions {
    pub minifier: Minifier,
}

impl Default for MinifyOptions {
    fn default() -> Self {
        MinifyOptions {
            // The `Native` minifier is the default when the `native-minifier`
            // feature is enabled. Without it (e.g. `--no-default-features`),
            // fall back to the `html-minifier` CLI so templates still minify.
            #[cfg(feature = "native-minifier")]
            minifier: Minifier::Native,
            #[cfg(not(feature = "native-minifier"))]
            minifier: Minifier::HTMLMinifier,
        }
    }
}

fn run_custom_command_unchecked(cmd: &[&str]) -> Output {
    let program = resolve_command(cmd[0]);
    Command::new(program)
        .args(cmd.iter().skip(1))
        .output()
        .expect("Failed to run minifier")
}

fn run_custom_command(cmd: &[&str]) -> Output {
    let out = run_custom_command_unchecked(cmd);
    if !out.stderr.is_empty() {
        panic!(
            "Minifier ran with error  {:?}",
            String::from_utf8(out.stderr).unwrap()
        )
    }
    out
}

fn run_custom_command_unchecked_wrapper(command: &str, input: &Path, output: &Path) -> Output {
    let mut cmd: Vec<&str> = command.split([' ', '\n']).collect();
    cmd.extend(vec![
        input.to_str().unwrap(),
        "-o",
        output.to_str().unwrap(),
    ]);
    run_custom_command_unchecked(&cmd)
}

/// Extract the argument of a `Custom(...)` / `CustomUnchecked(...)` minifier.
///
/// Accepts both a quoted string literal (`Custom("cmd --flag")`) and bare
/// tokens (`Custom(cmd --flag)`).
fn parse_command_arg(tokens: &proc_macro2::TokenStream) -> String {
    if let Ok(lit) = syn::parse2::<syn::LitStr>(tokens.clone()) {
        lit.value()
    } else {
        tokens.to_string()
    }
}

impl MinifyOptions {
    pub fn minify_file(&self, input: &Path, output: &Path) -> io::Result<()> {
        MINIFIER_INVOCATIONS.fetch_add(1, Ordering::SeqCst);
        match &self.minifier {
            Minifier::HTMLMinifier => {
                run_custom_command(&[
                    "html-minifier",
                    "--collapse-whitespace",
                    input.to_str().unwrap(),
                    "-o",
                    output.to_str().unwrap(),
                ]);
            }
            Minifier::Custom(command) => {
                let out = run_custom_command_unchecked_wrapper(command, input, output);

                if !out.stderr.is_empty() {
                    panic!(
                        "Minifier ran with error  {:?}",
                        String::from_utf8(out.stderr).unwrap()
                    )
                }
            }
            Minifier::CustomUnchecked(command) => {
                run_custom_command_unchecked_wrapper(command, input, output);
            }
            #[cfg(feature = "native-minifier")]
            Minifier::Native => {
                let contents = fs::read(input)?;
                let minified = minify_html::minify(&contents, &native_minify_cfg());
                fs::write(output, minified)?;
            }
            #[cfg(not(feature = "native-minifier"))]
            Minifier::Native => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "the `Native` minifier requires the `native-minifier` feature",
                ));
            }
        }
        Ok(())
    }
}

/// Parse `#[min_with(...)]` attributes from a struct's token stream and apply
/// them to the given `MinifyOptions`.
pub fn get_minify_options_from_token_stream(
    tokens: proc_macro2::TokenStream,
    options: &mut MinifyOptions,
) -> syn::Result<()> {
    let struct_item = syn::parse2::<ItemStruct>(tokens)?;

    for attr in &struct_item.attrs {
        if !attr.path().is_ident("min_with") {
            continue;
        }

        let parsed_args = attr.parse_args()?;
        match parsed_args {
            Meta::List(nv) => {
                let kind = nv.path.segments.first().unwrap().ident.to_string();
                let inner = parse_command_arg(&nv.tokens);
                match kind.as_str() {
                    "Custom" => options.minifier = Minifier::Custom(inner),
                    "CustomUnchecked" => options.minifier = Minifier::CustomUnchecked(inner),
                    "Native" => options.minifier = Minifier::Native,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "Wrong minifier value, supported values are HTMLMinifier, Native, Custom/CustomUnchecked(\"command\")",
                        ))
                    }
                }
            }
            Meta::Path(nv) => {
                let ident = nv.segments.first().unwrap().ident.to_string();
                match ident.as_str() {
                    "HTMLMinifier" => options.minifier = Minifier::HTMLMinifier,
                    "Native" => options.minifier = Minifier::Native,
                    _ => {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "Wrong minifier value, supported values are HTMLMinifier, Native, Custom/CustomUnchecked(\"command\")",
                        ))
                    }
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "Wrong minifier value, supported values are HTMLMinifier, Native, Custom/CustomUnchecked(\"command\")",
                ))
            }
        }
    }

    Ok(())
}

#[allow(dead_code)]
fn minify_file_and_components_internal(
    file_path: &Path,
    new_path: &Path,
    minify_options: &MinifyOptions,
    processed_files: Arc<Mutex<HashSet<PathBuf>>>,
) -> io::Result<()> {
    let canonical_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());

    // Check local cache first (for same template)
    {
        let processed = processed_files.lock().unwrap();
        if processed.contains(&canonical_path) {
            return Ok(());
        }
    }

    {
        let global_cache = get_global_cache().lock().unwrap();
        if let Some(cached_output_path) = global_cache.get(&canonical_path) {
            if cached_output_path.as_path().exists() && new_path != cached_output_path.as_path() {
                create_dir_all(new_path.parent().unwrap()).expect("Cannot create dir");
                fs::copy(cached_output_path, new_path)?;
            }
            processed_files.lock().unwrap().insert(canonical_path);
            return Ok(());
        }
    }

    processed_files.lock().unwrap().insert(canonical_path.clone());

    let mut input_file = File::open(file_path)?;
    let mut contents = String::new();
    input_file.read_to_string(&mut contents)?;
    let include_regex = get_include_regex();

    let includes: Vec<_> = include_regex.captures_iter(&contents).collect();

    if !includes.is_empty() {
        let parent_output_dir = new_path.parent().unwrap().to_path_buf();
        let include_results: Vec<Result<(String, String), Box<dyn std::error::Error + Send + Sync>>> =
            includes.par_iter().map(|cap| {
                let original_str = cap[0].to_string(); // include!("file.stpl")
                let file_name = cap[1].to_string(); // file.stpl

                let component_file_path = file_path.parent().unwrap().join(&file_name);

                let mut child_new_path = tmp_main_path();
                if let Some(parent) = file_path.parent() {
                    child_new_path.push(parent);
                }
                child_new_path.push(format!("{}.min", file_name));

                minify_file_and_components_internal(
                    &component_file_path,
                    &child_new_path,
                    minify_options,
                    Arc::clone(&processed_files),
                )
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

                // Rewrite the include to a path relative to the minified parent.
                // Relative (rather than absolute) paths are required on Windows:
                // sailfish-compiler strips the first character of absolute include
                // paths, which mangles `C:\...` style paths.
                let relative_path = pathdiff::diff_paths(&child_new_path, &parent_output_dir)
                    .unwrap_or_else(|| child_new_path.clone());
                let new_include = format!(r#"<% include!("{}"); %>"#, relative_path.to_string_lossy());

                Ok((original_str, new_include))
            }).collect();

        for result in include_results {
            match result {
                Ok((original_str, new_include)) => {
                    contents = contents.replace(&original_str, &new_include);
                }
                Err(e) => {
                    eprintln!("Error processing component: {}", e);
                    return Err(io::Error::other("Component processing failed"));
                }
            }
        }
    }

    create_dir_all(new_path.parent().unwrap()).expect("Cannot create dir");
    fs::write(new_path, contents)?;
    minify_options.minify_file(new_path, new_path)?;

    {
        let mut global_cache = get_global_cache().lock().unwrap();
        global_cache.insert(canonical_path, new_path.to_path_buf());
    }

    Ok(())
}

/// Minify a template and all of its includes recursively. Only used when the
/// `minify-components` feature is enabled, but kept compiled in every
/// configuration because the unit tests in `sailfish-minify-core` exercise it
/// either way.
#[allow(dead_code)]
pub fn minify_file_and_components(
    file_path: &Path,
    new_path: &Path,
    minify_options: &MinifyOptions,
) -> io::Result<()> {
    let processed_files = Arc::new(Mutex::new(HashSet::new()));
    minify_file_and_components_internal(file_path, new_path, minify_options, processed_files)
}

/// Copy a template and every template it transitively includes into `base`,
/// preserving the relative directory structure. Used when the
/// `minify-components` feature is disabled, so only the templates that are
/// actually referenced end up in the temp dir.
#[allow(dead_code)]
pub fn copy_referenced_template_and_includes(source: &Path, base: &Path) -> io::Result<()> {
    let mut visited = HashSet::new();
    copy_referenced_template_and_includes_inner(source, base, &mut visited)
}

#[allow(dead_code)]
fn copy_referenced_template_and_includes_inner(
    source: &Path,
    base: &Path,
    visited: &mut HashSet<PathBuf>,
) -> io::Result<()> {
    let canonical = source.canonicalize().unwrap_or_else(|_| source.to_path_buf());
    if !visited.insert(canonical) {
        return Ok(());
    }

    let destination = base.join(source);
    if let Some(parent) = destination.parent() {
        create_dir_all(parent)?;
    }
    fs::copy(source, &destination)?;

    let contents = fs::read_to_string(source)?;
    for file_name in extract_includes(&contents) {
        let child = source.parent().unwrap().join(file_name);
        copy_referenced_template_and_includes_inner(&child, base, visited)?;
    }

    Ok(())
}

/// Minify a single template according to the enabled features.
///
/// * `minify-components` enabled: recursively minify the template and all of
///   its includes into the temp dir, rewriting include paths.
/// * disabled: copy only the referenced templates into the temp dir, then
///   minify the referenced file in place.
pub fn minify_template(
    file_path: &Path,
    new_path: &Path,
    minify_options: &MinifyOptions,
) -> io::Result<()> {
    #[cfg(feature = "minify-components")]
    {
        minify_file_and_components(file_path, new_path, minify_options)
    }

    #[cfg(not(feature = "minify-components"))]
    {
        copy_referenced_template_and_includes(file_path, &tmp_main_path())?;
        minify_options.minify_file(file_path, new_path)?;
        Ok(())
    }
}
