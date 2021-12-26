use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    GenerateCover,
    ConvertFile,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(x: std::io::Error) -> Self {
        Error::IO(x)
    }
}
