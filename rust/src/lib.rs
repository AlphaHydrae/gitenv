#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramOutput {
    pub message: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramError {
    UnsupportedConfiguration,
}

pub fn run() -> Result<ProgramOutput, ProgramError> {
    Ok(ProgramOutput {
        message: "Hello, World!",
    })
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn shows_default_message() {
        let output = run().expect("run should succeed");

        assert_eq!(output.message, "Hello, World!");
    }
}
