# strict_linking_rs
Enforces the requirement that all functions defined inside your crate's `extern "C"` blocks must be resolved in the final executable.

# To use

In your [`build.rs`](https://doc.rust-lang.org/cargo/reference/build-scripts.html) file add this line to your `main()` function

```rust
strict_linking::init();
```

And then in your `Cargo.toml` file add this

```toml
[build-dependencies]
strict_linking = "0.1"
```

# How does it work?

First, we use `cargo +nightly rustc --profile=check -- -Zunpretty-expanded` to expand macros inside the code for your crate.
Then we parse that with the [`syn`](https://crates.io/crates/syn) crate, and walk down the syntax tree recursively, looking for
`extern "C"` blocks. When we find them, we emit [`cargo:rustc-link-arg`](https://doc.rust-lang.org/cargo/reference/build-scripts.html#rustc-link-arg)
instructions in a platform specific manner to use flags like [`/INCLUDE`](https://docs.microsoft.com/en-us/cpp/build/reference/include-force-symbol-references?view=msvc-170)
 and `--undefined` so that the linker will not link successfully if one of those symbols is missing.
