fn main() {
    strict_linking::init();
    cc::Build::new()
        .file("src/main.c")
        .include("src")
        .compile("c_module");
}
