fn main() {
    match gitenv::run_cli() {
        Ok(output) => println!("{}", output.message),
        Err(error) => {
            eprintln!("gitenv: {error}");
            std::process::exit(1);
        }
    }
}
