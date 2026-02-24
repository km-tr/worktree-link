use std::fmt;
use std::path::PathBuf;

/// Application-specific error types.
#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    ConfigNotFound(PathBuf),
    SourceNotFound(PathBuf),
    TargetExists(PathBuf),
    GitDirectory,
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ConfigNotFound(p) => write!(f, "Config file not found: {}", p.display()),
            Error::SourceNotFound(p) => {
                write!(f, "Source directory does not exist: {}", p.display())
            }
            Error::TargetExists(p) => write!(
                f,
                "Target already exists and --force not specified: {}",
                p.display()
            ),
            Error::GitDirectory => write!(f, "Refusing to link .git directory"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
