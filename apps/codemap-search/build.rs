use std::fs::File;
use std::path::PathBuf;

fn main() {
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
