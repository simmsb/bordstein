use bindgen::{
    EnumVariation,
    callbacks::{ItemInfo, ItemKind, ParseCallbacks, TypeKind},
};
use std::env;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

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

#[derive(Debug)]
struct AddDerives;

impl ParseCallbacks for AddDerives {
    fn add_derives(&self, info: &bindgen::callbacks::DeriveInfo<'_>) -> Vec<String> {
        if info.kind == TypeKind::Union {
            return vec![];
        }

        if info.name.starts_with('_')
            || info.name.contains("bindgen")
            || info.name.contains("Handlers")
            || info.name.contains("Callbacks")
            || info.name.contains("AnimationImplementation")
        {
            return vec![];
        }

        match info.name {
            "ActionMenuConfig"
            | "MenuLayerCallbacks"
            | "TouchEvent"
            | "HealthMinuteData"
            | "max_align_t"
            | "Tuple"
            | "Tuplet"
            | "AppSync"
            | "PropertyAnimationAccessors"
            | "ContentIndicatorConfig"
            | "SimpleMenuItem"
            | "sxAPI"  => return vec![],
            _ => {}
        }

        vec!["::ufmt_macros_fork::uDebug".to_owned()]
    }
}

#[derive(Debug, Clone)]
struct CollectMessageKeys {
    keys: Arc<Mutex<Vec<String>>>,
}

impl Default for CollectMessageKeys {
    fn default() -> Self {
        Self {
            keys: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ParseCallbacks for CollectMessageKeys {
    fn generated_name_override(&self, item_info: ItemInfo<'_>) -> Option<String> {
        if matches!(item_info.kind, ItemKind::Var) {
            if let Some(name) = item_info.name.strip_prefix("MESSAGE_KEY_") {
                self.keys.lock().unwrap().push(name.to_owned());
            }
        }
        None
    }
}

fn write_messages_rs(path: &Path, keys: &[String]) -> std::io::Result<()> {
    let mut keys: Vec<_> = keys.to_vec();
    keys.sort();
    keys.dedup();

    let mut file = std::fs::File::create(path)?;
    for name in keys {
        writeln!(file, "#[allow(non_snake_case)]")?;
        writeln!(file, "pub fn {name}() -> u32 {{")?;
        writeln!(
            file,
            "    unsafe {{ crate::bindings::MESSAGE_KEY_{name} }}"
        )?;
        writeln!(file, "}}")?;
        writeln!(file)?;
    }

    Ok(())
}

fn main() {
    println!("cargo:rerun-if-env-changed=PEBBLE_INCLUDE_DIRS");
    println!("cargo:rerun-if-env-changed=PEBBLE_CFLAGS");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none") {
        return;
    }

    let mut clang_args = pebble_include_args();
    let mut wrapper = "#include <pebble.h>\n\
                            ";
    if clang_args.is_empty() {
        wrapper = "#include <stdint.h>\n\
                   typedef int32_t time_t;\n\
                   #include <pebble.h>
                  ";
        let emulator = env::var("PEBBLE_EMULATOR").unwrap();
        let pebble_include_path = get_pebble_include_path(&emulator).unwrap();
        println!("cargo:rerun-if-changed={}", pebble_include_path.display());
        clang_args.push(format!("-isystem{}", pebble_include_path.display()));
        clang_args.push("-Ibuild/include/".to_string());
        clang_args.push(format!("-Ibuild/{emulator}"));
    }
    clang_args.extend(sdk_includes());
    clang_args.extend(pebble_cflags());
    clang_args.push("-std=c23".to_string());

    dbg!(&clang_args);

    let message_keys = CollectMessageKeys::default();

    let bindings = bindgen::Builder::default()
        .header_contents("wrapper.h", wrapper)
        .clang_args(clang_args)
        .use_core()
        .generate_cstr(true)
        .enable_function_attribute_detection()
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .parse_callbacks(Box::new(ProcessComments))
        .parse_callbacks(Box::new(AddDerives))
        .parse_callbacks(Box::new(message_keys.clone()))
        .bitfield_enum("TimeUnits")
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

    let keys = message_keys.keys.lock().unwrap().clone();
    write_messages_rs(&out_path.join("messages.rs"), &keys)
        .expect("Couldn't write messages!");
}
