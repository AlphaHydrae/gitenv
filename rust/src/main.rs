fn main() {
    match gitenv::run() {
        Ok(output) => println!("{}", output.message),
        Err(error) => {
            eprintln!("gitenv: {error:?}");
            std::process::exit(1);
        }
    }
}
