//! Migration checks never execute or modify reference packages.
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Component, Path},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(Deserialize)]
struct Baseline {
    sha256: String,
    file_count: usize,
    files: Vec<FileRecord>,
}
#[derive(Deserialize)]
struct FileRecord {
    path: String,
    bytes: u64,
    sha256: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("migration check failed: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    match std::env::args().nth(1).as_deref() {
        Some("check") => check(&root),
        Some("baseline") => baseline(&root),
        _ => Err("usage: cargo xtask <check|baseline>".into()),
    }
}
fn json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}
fn safe_relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.contains('\\')
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("not a normalized relative path: {path}").into());
    }
    Ok(())
}
fn read_baseline(root: &Path) -> Result<Baseline> {
    let baseline: Baseline =
        serde_json::from_slice(&fs::read(root.join("docs/migration/source-baseline.json"))?)?;
    let mut canonical = String::new();
    let mut previous = "";
    for file in &baseline.files {
        safe_relative(&file.path)?;
        if file.path.as_str() <= previous {
            return Err("baseline must have unique sorted paths".into());
        }
        previous = &file.path;
        canonical.push_str(&file.path);
        canonical.push('\0');
        canonical.push_str(&file.sha256);
        canonical.push('\n');
    }
    if baseline.files.len() != baseline.file_count
        || format!("{:x}", Sha256::digest(canonical.as_bytes())) != baseline.sha256
    {
        return Err("baseline manifest fingerprint/count mismatch".into());
    }
    Ok(baseline)
}
fn check(root: &Path) -> Result<()> {
    let baseline = read_baseline(root)?;
    let known: BTreeSet<_> = baseline.files.iter().map(|f| f.path.as_str()).collect();
    let inventory = json(&root.join("docs/migration/go-inventory.json"))?;
    let files = inventory["files"]
        .as_array()
        .ok_or("inventory needs files")?;
    let inventoried: BTreeSet<_> = files.iter().filter_map(|f| f["path"].as_str()).collect();
    let expected: BTreeSet<_> = known
        .iter()
        .copied()
        .filter(|p| p.ends_with(".go"))
        .collect();
    if inventoried != expected || files.len() != expected.len() {
        return Err("inventory does not cover exactly the baseline Go files".into());
    }
    let ledger = json(&root.join("docs/migration/parity.json"))?;
    if ledger["baseline_sha256"].as_str() != Some(&baseline.sha256) {
        return Err("ledger baseline does not match manifest".into());
    }
    let schema = json(&root.join("docs/migration/parity.schema.json"))?;
    let validator = jsonschema::validator_for(&schema)?;
    if !validator.is_valid(&ledger) {
        let errors: Vec<_> = validator
            .iter_errors(&ledger)
            .map(|e| e.to_string())
            .collect();
        return Err(format!("invalid ledger: {}", errors.join("; ")).into());
    }
    let mut ids = BTreeSet::new();
    for row in ledger["entries"].as_array().ok_or("ledger entries")? {
        let id = row["id"].as_str().ok_or("entry ID")?;
        if !ids.insert(id) {
            return Err(format!("duplicate ledger ID: {id}").into());
        }
        for path in row["go_sources"].as_array().ok_or("go_sources")? {
            let path = path.as_str().ok_or("source path")?;
            if !known.contains(path) {
                return Err(format!("{id}: source not in baseline: {path}").into());
            }
        }
        for test in row["go_tests"].as_array().ok_or("go_tests")? {
            let name = test.as_str().ok_or("test name")?;
            let found = files.iter().any(|file| {
                row["go_sources"]
                    .as_array()
                    .is_some_and(|sources| sources.contains(&file["path"]))
                    && file["tests"]
                        .as_array()
                        .is_some_and(|tests| tests.iter().any(|t| t == name))
            });
            if !found {
                return Err(format!("{id}: unknown Go test: {name}").into());
            }
        }
        for field in ["rust_sources", "evidence"] {
            for path in row[field].as_array().ok_or("path list")? {
                let path = path.as_str().ok_or("path")?;
                safe_relative(path)?;
                if !root.join(path).is_file() {
                    return Err(format!("{id}: missing {field}: {path}").into());
                }
            }
        }
        for test in row["rust_tests"].as_array().ok_or("rust_tests")? {
            let test = test.as_str().ok_or("Rust test")?;
            let (path, function) = test
                .split_once("::")
                .ok_or("Rust test must be path::function")?;
            safe_relative(path)?;
            let source = fs::read_to_string(root.join(path))?;
            let pattern = regex::Regex::new(&format!(r"\bfn\s+{}\s*\(", regex::escape(function)))?;
            if !pattern.is_match(&source) {
                return Err(format!("{id}: missing Rust test: {test}").into());
            }
        }
        if row["status"] == "verified"
            && (row["rust_tests"].as_array().is_none_or(Vec::is_empty)
                || row["evidence"].as_array().is_none_or(Vec::is_empty))
        {
            return Err(format!("{id}: verified requires tests and evidence").into());
        }
    }
    println!(
        "Validated {} parity entries, {} Go inventory files, and {} baseline files. References are structural checks, not test execution.",
        ids.len(),
        files.len(),
        baseline.file_count
    );
    Ok(())
}
fn walk(root: &Path, current: &Path, paths: &mut BTreeSet<String>) -> Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err("reference must not contain symlinks".into());
        }
        if kind.is_dir() {
            walk(root, &entry.path(), paths)?;
        } else if kind.is_file() {
            paths.insert(
                entry
                    .path()
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}
fn baseline(root: &Path) -> Result<()> {
    let manifest = read_baseline(root)?;
    let source = root.join("temp/agent-framework-go");
    let mut paths = BTreeSet::new();
    walk(&source, &source, &mut paths)?;
    let expected: BTreeSet<_> = manifest.files.iter().map(|f| f.path.clone()).collect();
    if paths != expected {
        return Err("reference has added or removed files".into());
    }
    for record in &manifest.files {
        let data = fs::read(source.join(&record.path))?;
        if data.len() as u64 != record.bytes
            || format!("{:x}", Sha256::digest(&data)) != record.sha256
        {
            return Err(format!("reference changed: {}", record.path).into());
        }
    }
    println!(
        "Reference unchanged: {} files; SHA-256 {}",
        manifest.file_count, manifest.sha256
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_escape_paths() {
        for path in ["../secret", "/etc/passwd", "C:\\secret", "", "a/../b"] {
            assert!(safe_relative(path).is_err());
        }
        assert!(safe_relative("crates/agent-core/src/lib.rs").is_ok());
    }
}
