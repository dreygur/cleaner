use std::path::Path;

use crate::config::PROTECTED_NAMES;
use crate::error::{AppError, Result};

pub fn validate(home: &Path, path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(AppError::OutsideHome(path.display().to_string()));
    }
    if path == home {
        return Err(AppError::ProtectedPath(path.display().to_string()));
    }

    let rel = path
        .strip_prefix(home)
        .map_err(|_| AppError::OutsideHome(path.display().to_string()))?;
    if rel.as_os_str().is_empty() {
        return Err(AppError::OutsideHome(path.display().to_string()));
    }
    if let Some(name) = rel.to_str() {
        if PROTECTED_NAMES.contains(&name) {
            return Err(AppError::ProtectedPath(path.display().to_string()));
        }
    }

    let metadata = std::fs::symlink_metadata(path).map_err(|source| AppError::Io {
        path: path.display().to_string(),
        source,
    })?;
    if !metadata.is_dir() || metadata.is_symlink() {
        return Err(AppError::NotDirectory(path.display().to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn relative_path_is_rejected() {
        let home = tempdir().unwrap();
        let err = validate(home.path(), std::path::Path::new("relative/x")).unwrap_err();
        assert!(matches!(err, crate::error::AppError::OutsideHome(_)));
    }

    #[test]
    fn home_itself_is_rejected() {
        let home = tempdir().unwrap();
        let err = validate(home.path(), home.path()).unwrap_err();
        assert!(matches!(err, crate::error::AppError::ProtectedPath(_)));
    }

    #[test]
    fn path_outside_home_is_rejected() {
        let home = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let err = validate(home.path(), outside.path()).unwrap_err();
        assert!(matches!(err, crate::error::AppError::OutsideHome(_)));
    }

    #[test]
    fn protected_top_level_name_is_rejected() {
        let home = tempdir().unwrap();
        let docs = home.path().join("Documents");
        fs::create_dir(&docs).unwrap();
        let err = validate(home.path(), &docs).unwrap_err();
        assert!(matches!(err, crate::error::AppError::ProtectedPath(_)));
    }

    #[test]
    fn symlink_is_rejected() {
        let home = tempdir().unwrap();
        let real = home.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = home.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let err = validate(home.path(), &link).unwrap_err();
        assert!(matches!(err, crate::error::AppError::NotDirectory(_)));
    }

    #[test]
    fn nonexistent_path_is_an_io_error() {
        let home = tempdir().unwrap();
        let err = validate(home.path(), &home.path().join("Code/proj/target")).unwrap_err();
        assert!(matches!(err, crate::error::AppError::Io { .. }));
    }

    #[test]
    fn real_directory_under_home_is_accepted() {
        let home = tempdir().unwrap();
        let target = home.path().join("Code").join("proj").join("target");
        fs::create_dir_all(&target).unwrap();
        assert!(validate(home.path(), &target).is_ok());
    }

    #[test]
    fn nested_path_under_a_protected_name_is_accepted() {
        let home = tempdir().unwrap();
        let cache = home.path().join("Library").join("Caches").join("go-build");
        fs::create_dir_all(&cache).unwrap();
        assert!(validate(home.path(), &cache).is_ok());
    }
}
