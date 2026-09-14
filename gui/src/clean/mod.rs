use std::fs;
use std::path::Path;

use crate::config::PROTECTED_NAMES;
use crate::error::{AppError, Result};
use crate::types::{Group, Kind, Target};

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
    let name = rel.to_string_lossy();
    if PROTECTED_NAMES.contains(&name.as_ref()) {
        return Err(AppError::ProtectedPath(path.display().to_string()));
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

fn remove_any(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|source| AppError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let result = if metadata.is_dir() && !metadata.is_symlink() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    result.map_err(|source| AppError::Io { path: path.display().to_string(), source })
}

fn remove_contents(path: &Path) -> Result<()> {
    let entries = fs::read_dir(path).map_err(|source| AppError::Io {
        path: path.display().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| AppError::Io {
            path: path.display().to_string(),
            source,
        })?;
        remove_any(&entry.path())?;
    }
    Ok(())
}

fn remove_target(home: &Path, kind: Kind, target: &Target) -> Result<()> {
    validate(home, &target.path)?;
    if kind == Kind::Fixed {
        return remove_contents(&target.path);
    }
    fs::remove_dir_all(&target.path).map_err(|source| AppError::Io {
        path: target.path.display().to_string(),
        source,
    })
}

pub fn clean_group(home: &Path, group: &Group) -> (u64, Vec<AppError>) {
    let mut freed = 0u64;
    let mut failures = Vec::new();

    for target in &group.targets {
        match remove_target(home, group.rule.kind, target) {
            Ok(()) => freed += target.size,
            Err(err) => failures.push(err),
        }
    }
    (freed, failures)
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

    #[test]
    fn remove_contents_empties_but_keeps_the_directory() {
        let home = tempdir().unwrap();
        let cache = home.path().join("Library/Caches/go-build");
        fs::create_dir_all(cache.join("nested")).unwrap();
        fs::write(cache.join("file"), "x").unwrap();
        fs::write(cache.join("nested/file"), "y").unwrap();

        remove_contents(&cache).unwrap();

        assert!(cache.exists());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn remove_target_fixed_empties_contents_only() {
        let home = tempdir().unwrap();
        let cache = home.path().join("Library/Caches/go-build");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("file"), "x").unwrap();
        let target = crate::types::Target { path: cache.clone(), size: 1 };

        remove_target(home.path(), crate::types::Kind::Fixed, &target).unwrap();

        assert!(cache.exists());
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 0);
    }

    #[test]
    fn remove_target_artifact_removes_the_whole_directory() {
        let home = tempdir().unwrap();
        let build = home.path().join("Code/proj/target");
        fs::create_dir_all(&build).unwrap();
        let target = crate::types::Target { path: build.clone(), size: 1 };

        remove_target(home.path(), crate::types::Kind::Artifact, &target).unwrap();

        assert!(!build.exists());
        assert!(build.parent().unwrap().exists());
    }

    #[test]
    fn clean_group_keeps_going_after_one_target_fails() {
        let home = tempdir().unwrap();
        let good = home.path().join("Code/proj/target");
        fs::create_dir_all(&good).unwrap();

        let group = crate::types::Group {
            rule: crate::types::Rule {
                id: "rust-target", description: "t", kind: crate::types::Kind::Artifact,
                dir: "target", marker: "Cargo.toml", roots: vec![], paths: vec![],
                regen: "t",
            },
            targets: vec![
                crate::types::Target { path: std::path::PathBuf::from("/etc/hosts"), size: 100 },
                crate::types::Target { path: good.clone(), size: 50 },
            ],
            size: 150,
        };

        let (freed, failures) = clean_group(home.path(), &group);

        assert_eq!(freed, 50);
        assert_eq!(failures.len(), 1);
        assert!(!good.exists());
    }
}
