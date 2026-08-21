use std::{env, ffi::OsStr, fmt::Write as _, fs, path::PathBuf};

fn main() {
    const SOURCE_DIR: &str = "content/v4/maps/builtin";
    println!("cargo::rerun-if-changed={SOURCE_DIR}");

    let source_dir = PathBuf::from(SOURCE_DIR);
    let mut sources = Vec::new();
    for entry in fs::read_dir(&source_dir).expect("read built-in map source directory") {
        let entry = entry.expect("read built-in map source entry");
        let file_type = entry.file_type().expect("read built-in map file type");
        assert!(
            !file_type.is_symlink(),
            "built-in map sources cannot be symlinks"
        );
        assert!(
            file_type.is_file(),
            "built-in map sources must be direct files"
        );
        let name = entry
            .file_name()
            .into_string()
            .expect("built-in map source names must be UTF-8");
        assert_eq!(
            entry.path().extension(),
            Some(OsStr::new("ron")),
            "built-in map sources must use .ron"
        );
        sources.push(name);
    }
    sources.sort();

    let mut generated = String::from("pub const EMBEDDED_BUILTIN_MAPS: &[(&str, &str)] = &[\n");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest directory");
    for name in sources {
        let absolute = PathBuf::from(&manifest_dir).join(SOURCE_DIR).join(&name);
        writeln!(
            generated,
            "    (\"builtin/{name}\", include_str!(r#\"{}\"#)),",
            absolute.display()
        )
        .expect("write generated built-in map row");
    }
    generated.push_str("];\n");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"))
        .join("embedded_builtin_maps.rs");
    fs::write(out, generated).expect("write embedded built-in map table");
}
