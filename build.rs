use std::{env, path::PathBuf, process::Command};

use bindgen::{Builder, EnumVariation};

#[derive(Debug, Clone, Copy)]
struct Features {
    cm7: bool,
    fpu32: bool,
    fpu64: bool,
    dsp: bool,
}

impl Features {
    fn from_env() -> Self {
        Features {
            cm7: env::var("CARGO_FEATURE_CM7").is_ok(),
            fpu32: env::var("CARGO_FEATURE_F32").is_ok(),
            fpu64: env::var("CARGO_FEATURE_F64").is_ok(),
            dsp: env::var("CARGO_FEATURE_DSP").is_ok(),
        }
    }
}

fn dispatch(target: &str, endianess: &str, features: &Features) -> Option<String> {
    let be = endianess.eq("big");
    let mut tokens = target.split_terminator('-');
    let architecture = match tokens.next().unwrap() {
        "thumbv6m" => "cortexM0",
        "thumbv7m" => "cortexM3",
        "thumbv7em" if features.cm7 => "cortexM7",
        "thumbv7em" => "cortexM4",
        "thumbv8m.base" => "ARMv8MBL",
        "thumbv8m.main" => "ARMv8MML",
        _ => unimplemented!(),
    };
    match tokens.next().unwrap() {
        "none" => (),
        _ => unimplemented!(),
    };
    let has_fpu = match tokens.next().unwrap() {
        "eabi" => false,
        "eabihf" => true,
        _ => unimplemented!(),
    };
    let has_fpu64 = has_fpu && features.cm7;
    let has_dsp = target.starts_with("thumbv8m.main");
    let mut name = if target.starts_with("thumbv8m") && be {
        None
    } else {
        Some(format!("{}{}", architecture, if be { "b" } else { "l" }))
    }?;
    if has_dsp && features.dsp {
        name.push('d');
    }
    if has_fpu && features.fpu32 {
        name.push('f');
        if has_fpu64 {
            name.push_str(if features.fpu64 { "dp" } else { "sp" });
        } else if target.starts_with("thumbv8m") {
            name.push_str("sp");
        }
    }
    Some(format!("arm_{}_math", name))
}

fn main() {
    const CMSIS_5_VERSION: &str = "5.7.0";
    const MUSL_VERSION: &str = "1.2.5";

    Command::new("make")
        .arg(format!("lib/ARM.CMSIS.{}", CMSIS_5_VERSION))
        .output()
        .unwrap();
    Command::new("make")
        .arg(format!("lib/musl-{}", MUSL_VERSION))
        .output()
        .unwrap();

    let outputs = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let libraries = manifest.join("lib");
    let cmsis = libraries.join(format!("ARM.CMSIS.{}", CMSIS_5_VERSION));
    let musl = libraries.join(format!("musl-{}", MUSL_VERSION));

    let target = env::var("TARGET").unwrap();
    let endianess = env::var("CARGO_CFG_TARGET_ENDIAN").unwrap();
    let features = Features::from_env();
    let library_name = dispatch(&target, &endianess, &features).unwrap();

    println!(
        "cargo:rustc-link-search={}",
        cmsis
            .join(PathBuf::from("CMSIS/DSP/Lib/GCC"))
            .to_str()
            .unwrap()
    );
    println!("cargo:rustc-link-lib={}", library_name);

    let bindings = Builder::default()
        .header("c/arm-math-sys.h")
        .use_core()
        .default_enum_style(EnumVariation::ModuleConsts)
        .allowlist_function(r"^arm.*")
        .allowlist_var(r"^arm.*")
        .blocklist_type(r"^__u?int\\d+_t")
        .clang_arg(format!("-I{}", manifest.join("c").display()))
        .clang_arg(format!("-I{}", cmsis.join("CMSIS/DSP/Include").display()))
        .clang_arg(format!("-I{}", cmsis.join("CMSIS/Include").display()))
        .clang_arg(format!("-I{}", musl.join("include").display()))
        .clang_arg(format!("-I{}", musl.join("arch/arm").display()))
        .clang_arg(format!("-I{}", musl.join("arch/generic").display()))
        .clang_arg(format!("-I{}", musl.join("obj/include").display()))
        .generate()
        .unwrap();
    bindings
        .write_to_file(PathBuf::from(&outputs).join("bindings.rs"))
        .unwrap();

    println!(
        "cargo:rerun-if-changed={}",
        manifest.join("c/arm-math-sys.h").display()
    );
    println!("cargo:rerun-if-changed=build.rs");
}
