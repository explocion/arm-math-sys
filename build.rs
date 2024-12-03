use std::{
    env::{self, VarError},
    fs::OpenOptions,
    io::{self, Seek, SeekFrom},
    path::PathBuf,
    str::FromStr,
};

use bindgen::{Builder, EnumVariation};
use git2::{build::RepoBuilder, FetchOptions, Repository};
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endianess {
    Little,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParseEndianessError;

impl FromStr for Endianess {
    type Err = ParseEndianessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "little" => Ok(Endianess::Little),
            "big" => Ok(Endianess::Big),
            _ => Err(ParseEndianessError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Features {
    cm7: bool,
    fsp: bool,
    fdp: bool,
    dsp: bool,
}

impl Default for Features {
    fn default() -> Self {
        Self {
            cm7: env::var("CARGO_FEATURE_CM7").is_ok(),
            fsp: env::var("CARGO_FEATURE_F32").is_ok(),
            fdp: env::var("CARGO_FEATURE_F64").is_ok(),
            dsp: env::var("CARGO_FEATURE_DSP").is_ok(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Config {
    target: String,
    endianess: Endianess,
    features: Features,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConfigError {
    VarError(VarError),
    ParseEndianessError(ParseEndianessError),
}

impl From<VarError> for ConfigError {
    fn from(value: VarError) -> Self {
        Self::VarError(value)
    }
}

impl From<ParseEndianessError> for ConfigError {
    fn from(value: ParseEndianessError) -> Self {
        Self::ParseEndianessError(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DispatchError {
    InvalidIsa,
    InvalidVendor,
    InvalidAbi,
    BigEndianForThumbv8m,
}

impl Config {
    fn new() -> Result<Self, ConfigError> {
        Ok(Self {
            target: env::var("TARGET")?,
            endianess: Endianess::from_str(&env::var("CARGO_CFG_TARGET_ENDIAN")?)?,
            features: Features::default(),
        })
    }

    fn dispatch(&self) -> Result<String, DispatchError> {
        let mut tokens = self.target.split('-');
        let architecture = match tokens.next().unwrap() {
            "thumbv6m" => Ok("cortexM0"),
            "thumbv7m" => Ok("cortexM3"),
            "thumbv7em" if self.features.cm7 => Ok("cortexM7"),
            "thumbv7em" => Ok("cortexM4"),
            "thumbv8m.base" => Ok("ARMv8MBL"),
            "thumbv8m.main" => Ok("ARMv8MML"),
            _ => Err(DispatchError::InvalidIsa),
        }?;
        match tokens.next().unwrap() {
            "none" => Ok(()),
            _ => Err(DispatchError::InvalidVendor),
        }?;
        let hf = match tokens.next().unwrap() {
            "eabi" => Ok(false),
            "eabihf" => Ok(true),
            _ => Err(DispatchError::InvalidAbi),
        }?;
        let thumbv8 = architecture.starts_with("thumbv8m");
        let fdp = hf && self.features.fdp;
        let dsp = self.target.starts_with("thumbv8m.main");
        let mut variant = String::from(architecture);
        match self.endianess {
            Endianess::Little => {
                variant.push('l');
                Ok(())
            }
            Endianess::Big if thumbv8 => Err(DispatchError::BigEndianForThumbv8m),
            Endianess::Big => {
                variant.push('b');
                Ok(())
            }
        }?;
        if dsp && self.features.dsp {
            variant.push('d');
        }
        if hf && self.features.fsp {
            variant.push('f');
            if thumbv8 || (fdp && !self.features.fdp) {
                variant.push_str("sp");
            }
        }
        if fdp && self.features.fdp {
            variant.push_str("dp");
        }
        Ok(variant)
    }
}

fn main() {
    let outputs = PathBuf::from(env::var("OUT_DIR").unwrap());

    const CMSIS_5_VERSION: &str = "5.7.0";

    let pack = outputs.join(format!("ARM.CMSIS.{}.pack", CMSIS_5_VERSION));
    let file = if !pack.is_file() {
        let response = ureq::get(&format!(
            "https://github.com/ARM-software/CMSIS_5/releases/download/{}/ARM.CMSIS.{}.pack",
            CMSIS_5_VERSION, CMSIS_5_VERSION
        ))
        .call()
        .unwrap();
        let bytes = response.header("Content-Length").unwrap().parse().unwrap();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .truncate(true)
            .create(true)
            .open(&pack)
            .unwrap();
        let written = io::copy(&mut response.into_reader(), &mut file).unwrap();
        assert_eq!(written, bytes);
        file.seek(SeekFrom::Start(0)).unwrap();
        file
    } else {
        OpenOptions::new().read(true).open(&pack).unwrap()
    };

    let cmsis = outputs.join(format!("ARM.CMSIS.{}", CMSIS_5_VERSION));
    if !cmsis.is_dir()
        || pack.metadata().unwrap().modified().unwrap()
            >= cmsis.metadata().unwrap().modified().unwrap()
    {
        let mut archive = ZipArchive::new(&file).unwrap();
        archive.extract(&cmsis).unwrap();
    }

    const NEWLIB_GIT: &str = "https://sourceware.org/git/newlib-cygwin.git";
    let newlib = outputs.join("newlib-cygwin");
    if !newlib.is_dir() {
        let mut fetch_options = FetchOptions::default();
        fetch_options.depth(1);
        RepoBuilder::new()
            .fetch_options(fetch_options)
            .clone(NEWLIB_GIT, &newlib)
            .unwrap();
    } else {
        let remote = Repository::open(&newlib)
            .unwrap()
            .find_remote("origin")
            .unwrap()
            .url()
            .unwrap()
            .to_owned();
        assert_eq!(remote, NEWLIB_GIT);
    }

    let variant = Config::new().unwrap().dispatch().unwrap();
    let library_name = format!("arm_{}_math", variant);

    println!(
        "cargo:rustc-link-search={}",
        cmsis.join("CMSIS/DSP/Lib/GCC").display()
    );
    println!("cargo:rustc-link-lib={}", library_name);

    let bindings = Builder::default()
        .header("c/arm-math-sys.h")
        .use_core()
        .default_enum_style(EnumVariation::ModuleConsts)
        .allowlist_function(r"^arm.*")
        .allowlist_var(r"^arm.*")
        .blocklist_type(r"^__u?int\\d+_t")
        .clang_arg("-Ic")
        .clang_arg(format!("-I{}", cmsis.join("CMSIS/DSP/Include").display()))
        .clang_arg(format!("-I{}", cmsis.join("CMSIS/Include").display()))
        .clang_arg(format!(
            "-I{}",
            newlib.join("newlib/libc/include").display()
        ))
        .generate()
        .unwrap();
    bindings
        .write_to_file(PathBuf::from(&outputs).join("bindings.rs"))
        .unwrap();

    println!("cargo:rerun-if-changed=c/arm-math-sys.h");
    println!("cargo:rerun-if-changed=build.rs");
}
