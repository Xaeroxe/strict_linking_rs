fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let success = std::process::Command::new(&args[0])
        .args(args.iter().skip(1))
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success();
    std::process::exit(if success {
       1 
    } else {
        0
    });
}
