use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use syn::{ForeignItem, Item, Meta};

const NO_RECURSE_ENV: &str = "__STRICT_LINKING_ACTIVE";

/// Enforces strict linking for your crate. Use this from `build.rs`!
pub fn init() {
    if env::var(NO_RECURSE_ENV).is_ok() {
        return;
    }
    let mut cmd = Command::new("cargo");
    cmd.env(NO_RECURSE_ENV, "")
        .env(
            "CARGO_TARGET_DIR",
            PathBuf::from(env::var("OUT_DIR").unwrap()).join("target-strict-linking"),
        )
        .current_dir(env::var("CARGO_MANIFEST_DIR").unwrap())
        .arg(
            "+".to_string()
                + &env::var("STRICT_LINKING_TOOLCHAIN_OVERRIDE")
                    .unwrap_or_else(|_| String::from("nightly")),
        )
        .args(["rustc", "--profile=check", "--", "-Zunpretty=expanded"])
        .stdout(Stdio::piped());
    let expanded_src = cmd
        .spawn()
        .expect("Failed to execute nightly rust compiler, one must be installed for strict_linking")
        .wait_with_output()
        .expect("error executing nightly rust compiler during strict_linking")
        .stdout;
    let tree: syn::File = syn::parse_str(
        &String::from_utf8(expanded_src).expect("cargo output wasn't utf8. This is a bug!"),
    )
    .expect("strict_linking failed to parse code as token stream");
    for item in tree.items.iter() {
        call_recurse(item, &mut |item| {
            if let Item::ForeignMod(foreigners) = item {
                if foreigners.abi.name.as_ref().map(|s| s.value()).as_deref() == Some("C") {
                    for foreign_item in &foreigners.items {
                        if let ForeignItem::Fn(function) = foreign_item {
                            let c_name = function
                                .attrs
                                .iter()
                                .find(|a| {
                                    a.path.segments.len() == 1
                                        && a.path
                                            .segments
                                            .last()
                                            .map(|s| s.ident.to_string())
                                            .as_deref()
                                            == Some("link_name")
                                })
                                .and_then(|a| a.parse_meta().ok())
                                .and_then(|m| match m {
                                    Meta::NameValue(nv) => Some(nv.lit),
                                    _ => None,
                                })
                                .and_then(|l| match l {
                                    syn::Lit::Str(s) => Some(s.value()),
                                    _ => None,
                                })
                                .unwrap_or_else(|| function.sig.ident.to_string());
                            strict_link_symbol(&c_name);
                        }
                    }
                }
            }
        });
    }
}

fn strict_link_symbol(s: &str) {
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/INCLUDE:{}", s);
    } else {
        println!("cargo:rustc-link-arg=--undefined={}", s);
    }
}

// Calls a closure on a list of Rust items recursively for each module.
fn call_recurse<F: FnMut(&Item)>(item: &Item, f: &mut F) {
    if let Item::Mod(mmod) = item {
        if let Some(t) = mmod.content.as_ref() {
            for item in t.1.iter() {
                call_recurse(item, f)
            }
        }
    }
    f(item)
}
