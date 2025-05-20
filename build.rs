// Build script based on the one from https://github.com/liushuyu/xdelta3-rs
// We don't need encoding, and we need to be able to stream the data so we cannot use the rust lib
// directly at the moment

fn main() {
    let defs = vec![
        ("EXTERNAL_COMPRESSION".to_string(), "1".to_string()),
        ("SECONDARY_DJW".to_string(), "0".to_string()),
        ("SECONDARY_FGK".to_string(), "0".to_string()),
        ("SECONDARY_LZMA".to_string(), "0".to_string()),
        ("SHELL_TESTS".to_string(), "0".to_string()),
        ("XD3_ENCODER".to_string(), "0".to_string()),
        ("XD3_USE_LARGEFILE64".to_string(), "1".to_string()),
        (
            "SIZEOF_SIZE_T".to_string(),
            std::mem::size_of::<usize>().to_string(),
        ),
        (
            "SIZEOF_UNSIGNED_INT".to_string(),
            std::mem::size_of::<std::os::raw::c_uint>().to_string(),
        ),
        (
            "SIZEOF_UNSIGNED_LONG".to_string(),
            std::mem::size_of::<std::os::raw::c_ulong>().to_string(),
        ),
        (
            "SIZEOF_UNSIGNED_LONG_LONG".to_string(),
            std::mem::size_of::<std::os::raw::c_ulonglong>().to_string(),
        ),
    ];

    {
        let mut build = cc::Build::new();

        for (key, value) in &defs {
            build.define(key, value.as_str());
        }
        build.file("xdelta/xdelta3/xdelta3.c");
        build.warnings(false);
        build.compile("xdelta3");
    }

    {
        use std::path::PathBuf;

        let mut builder = bindgen::Builder::default();
        builder = builder.clang_args(defs.iter().map(|(k, v)| format!("-D{k}={v}")));
        let bindings = builder
            .header("xdelta/xdelta3/xdelta3.h")
            .allowlist_function("xd3_.*")
            .allowlist_type("xd3_.*")
            .rustified_enum("xd3_.*")
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .expect("Failed to generate xdelta bindings");
        bindings
            .write_to_file(
                PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("xdelta_bindings.rs"),
            )
            .expect("Failed to write bindings");
    }
}
