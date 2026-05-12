fn main() {
    match gitenv::run(std::env::args_os()) {
        Ok(output) => println!("{}", output.message),
        Err(error) => {
            eprintln!("gitenv: {error}");
            std::process::exit(1);
        }
    }
}
