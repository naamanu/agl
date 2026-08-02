use crate::runtime::content_fingerprint;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("package I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid package manifest: {0}")]
    Manifest(String),
    #[error("package JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("dependency '{name}' content did not match integrity {expected}; got {actual}")]
    Integrity {
        name: String,
        expected: String,
        actual: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub name: String,
    pub version: String,
    pub language: String,
    pub entry: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Dependency {
    Local {
        path: String,
        #[serde(default)]
        integrity: Option<String>,
    },
    Git {
        git: String,
        rev: String,
        integrity: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockFile {
    pub format: u32,
    pub package: String,
    pub version: String,
    pub dependencies: BTreeMap<String, LockedDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockedDependency {
    pub source: String,
    pub revision: String,
    pub integrity: String,
}

impl PackageManifest {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PackageError> {
        let path = path.as_ref();
        let manifest: Self =
            serde_json::from_str(&fs::read_to_string(path).map_err(|source| {
                PackageError::Io {
                    path: path.into(),
                    source,
                }
            })?)?;
        validate_manifest(&manifest)?;
        Ok(manifest)
    }
}

pub fn lock_manifest(path: impl AsRef<Path>) -> Result<LockFile, PackageError> {
    let path = path.as_ref();
    let manifest = PackageManifest::load(path)?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let mut dependencies = BTreeMap::new();
    for (name, dependency) in &manifest.dependencies {
        let locked = match dependency {
            Dependency::Local { path, integrity } => {
                let target = root.join(path);
                let actual = hash_tree(&target)?;
                if let Some(expected) = integrity
                    && expected != &actual
                {
                    return Err(PackageError::Integrity {
                        name: name.clone(),
                        expected: expected.clone(),
                        actual,
                    });
                }
                LockedDependency {
                    source: format!("path:{}", target.display()),
                    revision: "local".into(),
                    integrity: actual,
                }
            }
            Dependency::Git {
                git,
                rev,
                integrity,
            } => {
                if rev.len() < 7
                    || rev.eq_ignore_ascii_case("main")
                    || rev.eq_ignore_ascii_case("master")
                {
                    return Err(PackageError::Manifest(format!(
                        "git dependency '{name}' must pin an immutable revision"
                    )));
                }
                if integrity.is_empty() {
                    return Err(PackageError::Manifest(format!(
                        "git dependency '{name}' must declare integrity"
                    )));
                }
                LockedDependency {
                    source: format!("git:{git}"),
                    revision: rev.clone(),
                    integrity: integrity.clone(),
                }
            }
        };
        dependencies.insert(name.clone(), locked);
    }
    Ok(LockFile {
        format: 1,
        package: manifest.name,
        version: manifest.version,
        dependencies,
    })
}

pub fn write_lock(
    manifest_path: impl AsRef<Path>,
    lock_path: impl AsRef<Path>,
) -> Result<LockFile, PackageError> {
    let lock = lock_manifest(&manifest_path)?;
    let target = lock_path.as_ref();
    fs::write(target, serde_json::to_vec_pretty(&lock)?).map_err(|source| PackageError::Io {
        path: target.into(),
        source,
    })?;
    Ok(lock)
}

pub fn verify_lock(lock: &LockFile) -> Result<(), PackageError> {
    for (name, dependency) in &lock.dependencies {
        if let Some(path) = dependency.source.strip_prefix("path:") {
            let actual = hash_tree(Path::new(path))?;
            if actual != dependency.integrity {
                return Err(PackageError::Integrity {
                    name: name.clone(),
                    expected: dependency.integrity.clone(),
                    actual,
                });
            }
        } else if dependency.source.starts_with("git:") && dependency.integrity.is_empty() {
            return Err(PackageError::Manifest(format!(
                "locked Git dependency '{name}' has no integrity"
            )));
        }
    }
    Ok(())
}

fn validate_manifest(manifest: &PackageManifest) -> Result<(), PackageError> {
    if manifest.name.is_empty() {
        return Err(PackageError::Manifest("name cannot be empty".into()));
    }
    let parts: Vec<_> = manifest.version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.parse::<u64>().is_err()) {
        return Err(PackageError::Manifest(
            "version must be semantic x.y.z".into(),
        ));
    }
    if !crate::ast::SUPPORTED_LANGUAGE_VERSIONS.contains(&manifest.language.as_str()) {
        return Err(PackageError::Manifest(format!(
            "unsupported language {}",
            manifest.language
        )));
    }
    Ok(())
}

fn hash_tree(path: &Path) -> Result<String, PackageError> {
    let mut files = Vec::new();
    collect(path, &mut files)?;
    files.sort();
    let mut bytes = Vec::new();
    for file in files {
        bytes.extend_from_slice(
            file.strip_prefix(path)
                .unwrap_or(&file)
                .to_string_lossy()
                .as_bytes(),
        );
        bytes.push(0);
        bytes.extend_from_slice(&fs::read(&file).map_err(|source| PackageError::Io {
            path: file.clone(),
            source,
        })?);
        bytes.push(0);
    }
    Ok(content_fingerprint(&bytes))
}
fn collect(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), PackageError> {
    if path.is_file() {
        files.push(path.into());
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|source| PackageError::Io {
        path: path.into(),
        source,
    })? {
        let entry = entry.map_err(|source| PackageError::Io {
            path: path.into(),
            source,
        })?;
        let child = entry.path();
        if child.file_name().and_then(|name| name.to_str()) == Some(".agl-cache") {
            continue;
        }
        if child.is_dir() {
            collect(&child, files)?;
        } else {
            files.push(child);
        }
    }
    Ok(())
}
