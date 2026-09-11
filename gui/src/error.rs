#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("{0}: path is outside the home directory")]
    OutsideHome(String),
    #[error("{0}: path is protected")]
    ProtectedPath(String),
    #[error("{0}: path is not a real directory")]
    NotDirectory(String),
    #[error("io on {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outside_home_message_names_the_path() {
        let err = AppError::OutsideHome("/etc/passwd".to_string());
        assert_eq!(err.to_string(), "/etc/passwd: path is outside the home directory");
    }

    #[test]
    fn protected_path_message_names_the_path() {
        let err = AppError::ProtectedPath("/Users/test/Documents".to_string());
        assert_eq!(err.to_string(), "/Users/test/Documents: path is protected");
    }

    #[test]
    fn not_directory_message_names_the_path() {
        let err = AppError::NotDirectory("/Users/test/link".to_string());
        assert_eq!(err.to_string(), "/Users/test/link: path is not a real directory");
    }

    #[test]
    fn io_message_includes_path_and_source() {
        let source = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = AppError::Io { path: "/Users/test/x".to_string(), source };
        assert_eq!(err.to_string(), "io on /Users/test/x: denied");
    }
}
