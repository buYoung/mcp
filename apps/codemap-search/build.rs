use std::fs::File;
use std::path::{Path, PathBuf};

fn hash_tree(path: &Path, hasher: &mut blake3::Hasher) {
    if path.is_dir() {
        let mut children: Vec<_> = std::fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        children.sort();
        for child in children {
            hash_tree(&child, hasher);
        }
    } else {
        let name = path.to_string_lossy();
        let data = std::fs::read(path).unwrap();
        hasher.update(&(name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update(&(data.len() as u64).to_le_bytes());
        hasher.update(&data);
    }
}

fn emit_analysis_build_id() {
    // A package version can cover multiple local builds. Embed the compiled inputs,
    // rather than hashing current_exe at runtime (its path may already be replaced).
    let mut hasher = blake3::Hasher::new();
    for input in ["src", "vendor", "Cargo.toml", "Cargo.lock", "build.rs"] {
        println!("cargo:rerun-if-changed={input}");
        hash_tree(Path::new(input), &mut hasher);
    }
    let mut settings: Vec<_> = std::env::vars()
        .filter(|(name, _)| {
            name.starts_with("CARGO_CFG_")
                || name.starts_with("CARGO_FEATURE_")
                || matches!(name.as_str(), "TARGET" | "PROFILE" | "CARGO_ENCODED_RUSTFLAGS")
        })
        .collect();
    settings.sort();
    for (name, value) in settings {
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(value.as_bytes());
        hasher.update(&[0]);
    }
    println!("cargo:rustc-env=CODEMAP_ANALYSIS_BUILD_ID={}", hasher.finalize());
}

fn main() {
    emit_analysis_build_id();
    let output = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    for language in ["groovy", "zsh", "kotlin"] {
        let source = PathBuf::from(format!("vendor/tree-sitter-{language}/src"));
        let compressed = source.join("parser.c.gz");
        let parser = output.join(format!("{language}-parser.c"));
        let mut input = flate2::read::GzDecoder::new(File::open(&compressed).unwrap());
        std::io::copy(&mut input, &mut File::create(&parser).unwrap()).unwrap();
        let scanner = source.join("scanner.c");
        let mut build = cc::Build::new();
        build.include(&source).file(parser).warnings(false);
        if scanner.is_file() {
            build.file(&scanner);
        }
        build
            .flag_if_supported("-std=c11")
            .compile(&format!("codemap_{language}"));
        println!("cargo:rerun-if-changed={}", source.display());
    }
}
