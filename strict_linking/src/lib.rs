extern crate core;

use std::env::VarError;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{env, mem};
use syn::{ForeignItem, Item, Meta};

const NO_RECURSE_ENV: &str = "__STRICT_LINKING_ACTIVE";

/// Enforces strict linking for your crate. Use this from `build.rs`!
pub fn init() {
    if env::var(NO_RECURSE_ENV).is_ok() {
        return;
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let mut cmd = Command::new("cargo");
    cmd.env(NO_RECURSE_ENV, "")
        .env("CARGO_TARGET_DIR", out_dir.join("target-strict-linking"))
        .current_dir(&manifest_dir)
        .arg(
            "+".to_string()
                + &env::var("STRICT_LINKING_TOOLCHAIN_OVERRIDE")
                    .unwrap_or_else(|_| String::from("nightly")),
        )
        .args(["rustc", "--profile=check", "--", "-Zunpretty=expanded"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = cmd
        .spawn()
        .expect("Failed to execute nightly rust compiler, one must be installed for strict_linking")
        .wait_with_output()
        .expect("error executing nightly rust compiler during strict_linking");
    let utf8_stdout =
        String::from_utf8(output.stdout).expect("cargo output wasn't utf8. This is a bug!");
    assert!(
        output.status.success(),
        "Compiling with nightly failed.\nstdout: {}\nstderr: {}",
        utf8_stdout,
        String::from_utf8(output.stderr).expect("cargo output wasn't utf8. This is a bug!")
    );
    let tree: syn::File =
        syn::parse_str(&utf8_stdout).expect("strict_linking failed to parse code as token stream");
    let arglist_path = out_dir.join("strict_linking_arg_list.txt");
    let mut arglist = File::create(&arglist_path).expect("Failed to create arg list file!");
    let env = env::var("CARGO_CFG_TARGET_ENV");

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
                                .and_then(|a| {
                                    let r = a.parse_meta();
                                    if let Err(e) = &r {
                                        println!("Error parsing meta for link_name {:?}", e);
                                    }
                                    r.ok()
                                })
                                .and_then(|m| match m {
                                    Meta::NameValue(nv) => Some(nv.lit),
                                    _ => None,
                                })
                                .and_then(|l| match l {
                                    syn::Lit::Str(s) => Some(
                                        convert_symbol_to_linker_format(&s.value()).to_string(),
                                    ),
                                    _ => None,
                                })
                                .unwrap_or_else(|| function.sig.ident.to_string());
                            strict_link_symbol(&mut arglist, &c_name, &env);
                        }
                    }
                }
            }
        });
    }
    arglist.flush().unwrap();
    mem::drop(arglist);
    if env.as_deref() == Ok("gnu") {
        println!("cargo:rustc-link-arg=-Wl,@{}", arglist_path.display());
    } else {
        println!("cargo:rustc-link-arg=@{}", arglist_path.display());
    }
    println!(
        "cargo:rerun-if-changed={}",
        manifest_dir.join("src").display()
    );
}

fn strict_link_symbol(out: &mut impl Write, s: &str, env: &Result<String, VarError>) {
    match env.as_deref() {
        Ok("msvc") => writeln!(out, "/INCLUDE:\"{}\"", s),
        _ => writeln!(out, "--require-defined={}", s),
    }
    .expect("Failed to write to arg file");
}

fn convert_symbol_to_linker_format(s: &str) -> &str {
    if s.is_empty() {
        return "";
    }
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        if s.chars().skip(1).next() == Some('?') {
            &s[1..]
        } else {
            s
        }
    } else {
        s
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
