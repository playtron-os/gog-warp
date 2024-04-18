// Build script based on the one from https://github.com/liushuyu/xdelta3-rs
// We don't need encoding, and we need to be able to stream the data so we cannot use the rust lib
// directly at the moment

fn main() {
    let mut defs = vec![
        ("EXTERNAL_COMPRESSION".to_string(), "1".to_string()),
        ("SECONDARY_DJW".to_string(), "0".to_string()),
        ("SECONDARY_FGK".to_string(), "0".to_string()),
        ("SECONDARY_LZMA".to_string(), "0".to_string()),
        ("SHELL_TESTS".to_string(), "0".to_string()),
        ("XD3_ENCODER".to_string(), "0".to_string()),
        ("XD3_USE_LARGEFILE64".to_string(), "1".to_string()),
    ];

    for i in &[
        "size_t",
        "unsigned int",
        "unsigned long",
        "unsigned long long",
    ] {
        let def_name = format!("SIZEOF_{}", i.to_uppercase().replace(' ', "_"));
        defs.push((def_name, check_native_size(i)));
    }

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

fn check_native_size(name: &str) -> String {
    use rand::Rng;
    use std::fs::remove_file;
    use std::io::Write;

    let builder = cc::Build::new();
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let compiler = builder.get_compiler();
    let mut compile = std::process::Command::new(compiler.path().as_os_str());
    let test_code = format!("#include <stdint.h>\n#include <stdio.h>\nint main() {{printf(\"%lu\", sizeof({})); return 0;}}\n", name);
    // didn't use tempfile since tempfile was having issues on Windows
    let mut rng = rand::thread_rng();
    let test_binary_fn = format!("{}/test-{}", out_dir, rng.gen::<i32>());

    #[cfg(windows)]
    let test_binary_fn = format!("{}.exe", test_binary_fn);

    let test_source_fn = format!("{}/test-{:x}.c", out_dir, rng.gen::<i32>());
    let mut test_source =
        std::fs::File::create(&test_source_fn).expect("Error creating test compile files");

    compile.args(compiler.args()).current_dir(out_dir);
    if compiler.is_like_msvc() {
        compile.args([&test_source_fn, &format!("/Fe{}", test_binary_fn)]);
    } else {
        compile.args([&test_source_fn, "-o", &test_binary_fn]);
    }
    test_source
        .write_all(test_code.as_bytes())
        .expect("Error writing test compile files");
    drop(test_source); // close the source file, otherwise there will be problems on Windows
    for (a, b) in compiler.env().iter() {
        compile.env(a, b);
    }
    compile.output().expect("Error compiling test source");
    remove_file(test_source_fn).ok();

    compile = std::process::Command::new(&test_binary_fn);
    let output = compile
        .output()
        .expect("Error executing test binary")
        .stdout;
    let output = String::from_utf8(output).expect("Error converting Unicode sequence");
    remove_file(test_binary_fn).ok();
    output
}
