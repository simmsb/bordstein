use bindgen::{EnumVariation, callbacks::ParseCallbacks};
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn get_pebble_include_path(platform: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let output = Command::new("pebble")
        .args(["sdk", "include-path", platform])
        .output()?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Pebble command failed: {}", error_msg.trim()).into());
    }

    let path_str = str::from_utf8(&output.stdout)?;
    let trimmed_path = path_str.trim();

    Ok(PathBuf::from(trimmed_path))
}

fn pebble_include_args() -> Vec<String> {
    let mut items = vec![];
    if let Ok(dir) = env::var("PEBBLE_INCLUDE_DIRS") {
        for item in dir.split(':') {
            items.push(format!("-I{item}"));
        }
        return items;
    }
    items
}

fn pebble_cflags() -> Vec<String> {
    let Ok(cflags) = env::var("PEBBLE_CFLAGS") else {
        return vec![];
    };
    cflags
        .split_whitespace()
        .filter(|f| *f != "-Werror")
        .map(str::to_string)
        .collect()
}

fn sdk_includes() -> Vec<String> {
    let gcc_output = Command::new("arm-none-eabi-gcc")
        .args(["-E", "-v", "-xc", "/dev/null"])
        .output();

    let mut args = Vec::new();

    if let Ok(output) = gcc_output {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut inside_include_list = false;

        for line in stderr.lines() {
            let line = line.trim();

            if line.contains("#include <...> search starts here:") {
                inside_include_list = true;
                continue;
            }

            if line.contains("End of search list.") {
                inside_include_list = false;
                continue;
            }

            if inside_include_list && !line.is_empty() {
                args.push(format!("-I{line}"));
            }
        }
    }

    args
}

#[derive(Debug)]
struct ProcessComments;

impl ParseCallbacks for ProcessComments {
    fn process_comment(&self, comment: &str) -> Option<String> {
        let comment = comment
            .lines()
            .map(|l| l.trim_start().trim_start_matches('!'))
            .collect::<Vec<_>>()
            .join("\n");
        match doxygen_bindgen::transform(&comment) {
            Ok(res) => Some(res),
            Err(err) => {
                println!("cargo:warning=Problem processing doxygen comment: {comment}\n{err}");
                None
            }
        }
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=PEBBLE_INCLUDE_DIRS");
    println!("cargo:rerun-if-env-changed=PEBBLE_CFLAGS");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none") {
        return;
    }

    let mut clang_args = pebble_include_args();
    let mut wrapper = "#include <pebble.h>";
    if clang_args.is_empty() {
        wrapper = "#include <stdint.h>\n\
                   typedef int32_t time_t;\n\
                   #include <pebble.h>";
        let emulator = env::var("PEBBLE_EMULATOR").unwrap();
        let pebble_include_path = get_pebble_include_path(&emulator).unwrap();
        clang_args.push(format!("-isystem{}", pebble_include_path.display()));
        clang_args.push("-Ibuild/include/".to_string());
        clang_args.push(format!("-Ibuild/{emulator}"));
    }
    clang_args.extend(sdk_includes());
    clang_args.extend(pebble_cflags());
    clang_args.push("-std=c23".to_string());

    dbg!(&clang_args);

    let bindings = bindgen::Builder::default()
        .header_contents("wrapper.h", wrapper)
        .clang_args(clang_args)
        .use_core()
        .generate_cstr(true)
        .enable_function_attribute_detection()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .parse_callbacks(Box::new(ProcessComments))
        .clang_args(&["-E", "-CC"])
        .clang_arg("--target=arm-none-eabi")
        .clang_arg("-Wno-macro-redefined")
        .clang_arg("-D_TIME_H_")
        .clang_arg("-fparse-all-comments")
        .clang_arg("-fretain-comments-from-system-headers")
        .generate_comments(true)
        .default_enum_style(EnumVariation::Rust {
            non_exhaustive: false,
        })
        .prepend_enum_name(false)
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
