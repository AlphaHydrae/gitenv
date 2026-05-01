pub fn hello_world() -> &'static str {
    "Hello, World!"
}

#[cfg(test)]
mod tests {
    use super::hello_world;

    #[test]
    fn returns_hello_world_message() {
        assert_eq!(hello_world(), "Hello, World!");
    }
}
