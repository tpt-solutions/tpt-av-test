//! In-memory virtual filesystem: a `BTreeMap`-backed VFS with normalising
//! POSIX-style paths.
//!
//! Lets fixtures and tests create "files" without touching the real disk —
//! no temp directories, no cleanup, no platform path quirks — while
//! mirroring the operations the TPT AV Stack actually performs (read a
//! media file, list an asset directory, check existence and sizes).
//!
//! ```rust
//! use tpt_av_test_mock::filesystem::VirtualFs;
//!
//! let mut fs = VirtualFs::new();
//! fs.write_file("/assets/sine-440.wav", b"RIFF....")?;
//! assert!(fs.exists("/assets/sine-440.wav"));
//! assert_eq!(fs.file_size("/assets/sine-440.wav")?, 8);
//!
//! // Paths are normalised: `.` and `..` resolve against the virtual root.
//! assert_eq!(fs.read_file("/assets/../assets/./sine-440.wav")?, b"RIFF....");
//!
//! fs.create_dir("/cache")?;
//! assert!(fs.is_dir("/cache"));
//! # Ok::<(), tpt_av_test_mock::filesystem::FsError>(())
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// One entry in the virtual filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File(Vec<u8>),
    Dir,
}

/// Errors reported by [`VirtualFs`] operations, mirroring the errno shapes
/// real code expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    /// A parent component of the path is a file, not a directory.
    NotADirectory { path: String },
    /// A directory was expected but a file (or nothing) is there.
    IsADirectory { path: String },
    /// Nothing exists at the path.
    NotFound { path: String },
    /// Creating something that already exists.
    AlreadyExists { path: String },
    /// Removing a directory that still contains entries.
    DirectoryNotEmpty { path: String },
    /// The path could not be normalised (e.g. `..` past the root).
    InvalidPath { path: String },
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FsError::NotADirectory { path } => {
                write!(f, "not a directory: {path}")
            }
            FsError::IsADirectory { path } => write!(f, "is a directory: {path}"),
            FsError::NotFound { path } => write!(f, "no such file or directory: {path}"),
            FsError::AlreadyExists { path } => write!(f, "already exists: {path}"),
            FsError::DirectoryNotEmpty { path } => write!(f, "directory not empty: {path}"),
            FsError::InvalidPath { path } => write!(f, "invalid path: {path}"),
        }
    }
}

impl std::error::Error for FsError {}

/// An in-memory filesystem. Not `Sync` on purpose: tests own their VFS and
/// mutate it freely. Clone to snapshot a fixture.
#[derive(Debug, Clone, Default)]
pub struct VirtualFs {
    entries: BTreeMap<PathBuf, Entry>,
}

impl VirtualFs {
    /// An empty filesystem containing only the root directory `/`.
    pub fn new() -> Self {
        let mut fs = VirtualFs::default();
        fs.entries.insert(PathBuf::from("/"), Entry::Dir);
        fs
    }

    /// Writes (or overwrites) a file, creating parent directories as
    /// needed — like `mkdir -p` plus `open(.., "wb")`.
    pub fn write_file(
        &mut self,
        path: impl AsRef<Path>,
        contents: impl Into<Vec<u8>>,
    ) -> Result<(), FsError> {
        let path = self.normalize(path)?;
        // Ensure every parent directory exists.
        let mut ancestors: Vec<PathBuf> = path.ancestors().skip(1).map(Path::to_path_buf).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            match self.entries.get(&ancestor) {
                Some(Entry::Dir) => {}
                Some(Entry::File(_)) => {
                    return Err(FsError::NotADirectory {
                        path: ancestor.display().to_string(),
                    });
                }
                None => {
                    self.entries.insert(ancestor, Entry::Dir);
                }
            }
        }
        self.entries.insert(path, Entry::File(contents.into()));
        Ok(())
    }

    /// Reads a file's contents.
    pub fn read_file(&self, path: impl AsRef<Path>) -> Result<Vec<u8>, FsError> {
        let path = self.normalize(path)?;
        match self.entries.get(&path) {
            Some(Entry::File(contents)) => Ok(contents.clone()),
            Some(Entry::Dir) => Err(FsError::IsADirectory {
                path: path.display().to_string(),
            }),
            None => {
                // Distinguish "nothing there at all" from "a file sits in
                // the middle of the path" (ENOTDIR on a real system).
                if path
                    .ancestors()
                    .skip(1)
                    .any(|ancestor| matches!(self.entries.get(ancestor), Some(Entry::File(_))))
                {
                    Err(FsError::NotADirectory {
                        path: path.display().to_string(),
                    })
                } else {
                    Err(FsError::NotFound {
                        path: path.display().to_string(),
                    })
                }
            }
        }
    }

    /// Creates a directory (parents included). Fails if the path already
    /// exists.
    pub fn create_dir(&mut self, path: impl AsRef<Path>) -> Result<(), FsError> {
        let path = self.normalize(path)?;
        if self.entries.contains_key(&path) {
            return Err(FsError::AlreadyExists {
                path: path.display().to_string(),
            });
        }
        let mut ancestors: Vec<PathBuf> = path.ancestors().skip(1).map(Path::to_path_buf).collect();
        ancestors.reverse();
        for ancestor in ancestors {
            self.entries.entry(ancestor).or_insert(Entry::Dir);
        }
        self.entries.insert(path, Entry::Dir);
        Ok(())
    }

    /// Whether anything (file or directory) exists at the path.
    pub fn exists(&self, path: impl AsRef<Path>) -> bool {
        match self.normalize(path) {
            Ok(path) => self.entries.contains_key(&path),
            Err(_) => false,
        }
    }

    /// Whether a directory exists at the path.
    pub fn is_dir(&self, path: impl AsRef<Path>) -> bool {
        matches!(self.lookup(path), Some(Entry::Dir))
    }

    /// Whether a file exists at the path.
    pub fn is_file(&self, path: impl AsRef<Path>) -> bool {
        matches!(self.lookup(path), Some(Entry::File(_)))
    }

    /// Size in bytes of the file at the path.
    pub fn file_size(&self, path: impl AsRef<Path>) -> Result<u64, FsError> {
        let contents = self.read_file(path)?;
        Ok(contents.len() as u64)
    }

    /// Removes a file. Fails for directories (use
    /// [`VirtualFs::remove_dir`] / [`VirtualFs::remove_dir_all`]).
    pub fn remove_file(&mut self, path: impl AsRef<Path>) -> Result<(), FsError> {
        let path = self.normalize(path)?;
        match self.entries.remove(&path) {
            Some(Entry::File(_)) => Ok(()),
            Some(Entry::Dir) => {
                self.entries.insert(path.clone(), Entry::Dir);
                Err(FsError::IsADirectory {
                    path: path.display().to_string(),
                })
            }
            None => Err(FsError::NotFound {
                path: path.display().to_string(),
            }),
        }
    }

    /// Removes an empty directory.
    pub fn remove_dir(&mut self, path: impl AsRef<Path>) -> Result<(), FsError> {
        let path = self.normalize(path)?;
        match self.entries.get(&path) {
            None => Err(FsError::NotFound {
                path: path.display().to_string(),
            }),
            Some(Entry::File(_)) => Err(FsError::NotADirectory {
                path: path.display().to_string(),
            }),
            Some(Entry::Dir) => {
                if self.child_count(&path) > 0 {
                    Err(FsError::DirectoryNotEmpty {
                        path: path.display().to_string(),
                    })
                } else {
                    self.entries.remove(&path);
                    Ok(())
                }
            }
        }
    }

    /// Removes a directory and everything below it. The root `/` cannot be
    /// removed, only emptied.
    pub fn remove_dir_all(&mut self, path: impl AsRef<Path>) -> Result<(), FsError> {
        let path = self.normalize(path)?;
        if !self.is_dir(&path) {
            return Err(match self.entries.get(&path) {
                Some(_) => FsError::NotADirectory {
                    path: path.display().to_string(),
                },
                None => FsError::NotFound {
                    path: path.display().to_string(),
                },
            });
        }
        // `Path::starts_with` is component-wise, so "/audio2" survives
        // removing "/audio". The root is kept, only emptied.
        if path == Path::new("/") {
            self.entries
                .retain(|existing, _| existing == Path::new("/"));
        } else {
            self.entries
                .retain(|existing, _| !existing.starts_with(&path));
        }
        Ok(())
    }

    /// Renames (moves) a file or directory subtree.
    pub fn rename(&mut self, from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), FsError> {
        let from = self.normalize(from)?;
        let to = self.normalize(to)?;
        if !self.entries.contains_key(&from) {
            return Err(FsError::NotFound {
                path: from.display().to_string(),
            });
        }
        if self.entries.contains_key(&to) {
            return Err(FsError::AlreadyExists {
                path: to.display().to_string(),
            });
        }
        let moved: Vec<(PathBuf, Entry)> = self
            .entries
            .iter()
            .filter(|(existing, _)| existing.starts_with(&from))
            .map(|(existing, entry)| (existing.clone(), entry.clone()))
            .collect();
        for (existing, entry) in moved {
            self.entries.remove(&existing);
            let relative = existing.strip_prefix(&from).unwrap_or(Path::new(""));
            let new_path = if relative.as_os_str().is_empty() {
                to.clone()
            } else {
                to.join(relative)
            };
            self.entries.insert(new_path, entry);
        }
        Ok(())
    }

    /// Names of the entries in a directory, sorted.
    pub fn list_dir(&self, path: impl AsRef<Path>) -> Result<Vec<String>, FsError> {
        let path = self.normalize(path)?;
        match self.entries.get(&path) {
            Some(Entry::Dir) => {
                let names = self
                    .entries
                    .keys()
                    .filter_map(|existing| {
                        let rest = existing.strip_prefix(&path).ok()?;
                        let rest = rest.strip_prefix("/").unwrap_or(rest);
                        if rest.as_os_str().is_empty() || rest.parent() != Some(Path::new("")) {
                            return None;
                        }
                        Some(rest.display().to_string())
                    })
                    .collect();
                Ok(names)
            }
            Some(Entry::File(_)) => Err(FsError::NotADirectory {
                path: path.display().to_string(),
            }),
            None => Err(FsError::NotFound {
                path: path.display().to_string(),
            }),
        }
    }

    /// Every normalised path in the filesystem, sorted (root included).
    pub fn paths(&self) -> Vec<PathBuf> {
        self.entries.keys().cloned().collect()
    }

    fn lookup(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = self.normalize(path).ok()?;
        self.entries.get(&path)
    }

    fn child_count(&self, dir: &Path) -> usize {
        self.entries
            .keys()
            .filter(|existing| {
                existing != &dir
                    && existing
                        .strip_prefix(dir)
                        .map(|rest| !rest.as_os_str().is_empty())
                        .unwrap_or(false)
            })
            .count()
    }

    /// Normalises a path to an absolute, cleaned form: rooted at `/`, no
    /// empty or `.` components, `..` resolved (and refused past the root).
    fn normalize(&self, path: impl AsRef<Path>) -> Result<PathBuf, FsError> {
        let raw = path.as_ref();
        let text = raw.to_string_lossy();
        if text.trim().is_empty() {
            return Err(FsError::InvalidPath {
                path: text.into_owned(),
            });
        }
        let mut cleaned = PathBuf::from("/");
        // On Windows, `\` and drive letters would derail `components`; the
        // VFS contract is POSIX-style, so split on both separators.
        for component in text.replace('\\', "/").split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if !cleaned.pop() {
                        return Err(FsError::InvalidPath {
                            path: text.into_owned(),
                        });
                    }
                    if cleaned.as_os_str().is_empty() {
                        cleaned.push("/");
                    }
                }
                name => cleaned.push(name),
            }
        }
        Ok(cleaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> VirtualFs {
        let mut fs = VirtualFs::new();
        fs.write_file("/audio/wav/sine.wav", b"RIFFxxxxWAVE")
            .unwrap();
        fs.write_file("/audio/flac/sine.flac", b"fLaC....").unwrap();
        fs.create_dir("/cache").unwrap();
        fs
    }

    #[test]
    fn new_fs_contains_only_the_root() {
        let fs = VirtualFs::new();
        assert!(fs.is_dir("/"));
        assert_eq!(fs.paths(), vec![PathBuf::from("/")]);
    }

    #[test]
    fn write_read_round_trip() {
        let mut fs = VirtualFs::new();
        fs.write_file("/a/b/c.bin", [1u8, 2, 3]).unwrap();
        assert_eq!(fs.read_file("/a/b/c.bin").unwrap(), vec![1, 2, 3]);
        assert!(fs.is_dir("/a"));
        assert!(fs.is_dir("/a/b"));
        assert!(fs.is_file("/a/b/c.bin"));
        assert_eq!(fs.file_size("/a/b/c.bin").unwrap(), 3);
    }

    #[test]
    fn overwrite_replaces_contents() {
        let mut fs = VirtualFs::new();
        fs.write_file("/x.bin", b"old").unwrap();
        fs.write_file("/x.bin", b"new contents").unwrap();
        assert_eq!(fs.read_file("/x.bin").unwrap(), b"new contents");
    }

    #[test]
    fn paths_are_normalised() {
        let fs = fixture();
        assert_eq!(
            fs.read_file("/audio/../audio/./wav/sine.wav").unwrap(),
            b"RIFFxxxxWAVE"
        );
        assert!(
            fs.exists("audio/wav/sine.wav"),
            "relative resolves against /"
        );
        assert!(
            fs.exists("/audio//wav///sine.wav"),
            "empty components collapse"
        );
        assert!(
            fs.exists("/audio\\wav\\sine.wav"),
            "windows separators accepted"
        );
        assert!(matches!(
            fs.read_file("/../sine.wav"),
            Err(FsError::InvalidPath { .. })
        ));
        assert!(matches!(fs.read_file(""), Err(FsError::InvalidPath { .. })));
    }

    #[test]
    fn read_errors_distinguish_kinds() {
        let fs = fixture();
        assert!(matches!(
            fs.read_file("/missing.bin"),
            Err(FsError::NotFound { .. })
        ));
        assert!(matches!(
            fs.read_file("/cache"),
            Err(FsError::IsADirectory { .. })
        ));
        assert!(matches!(
            fs.read_file("/audio/wav/sine.wav/inner"),
            Err(FsError::NotADirectory { .. })
        ));
    }

    #[test]
    fn create_dir_fails_when_it_already_exists() {
        let mut fs = fixture();
        assert!(matches!(
            fs.create_dir("/cache"),
            Err(FsError::AlreadyExists { .. })
        ));
        assert!(fs.create_dir("/cache/deep/nested").is_ok());
        assert!(fs.is_dir("/cache/deep/nested"));
    }

    #[test]
    fn remove_semantics() {
        let mut fs = fixture();
        // An empty directory goes away silently.
        assert!(fs.remove_dir("/cache").is_ok());
        assert!(!fs.exists("/cache"));

        // A directory holding a file refuses.
        fs.create_dir("/full").unwrap();
        fs.write_file("/full/thing.bin", b"x").unwrap();
        assert!(matches!(
            fs.remove_dir("/full"),
            Err(FsError::DirectoryNotEmpty { .. })
        ));
        fs.remove_file("/full/thing.bin").unwrap();
        assert!(fs.remove_dir("/full").is_ok());

        fs.write_file("/tmp.bin", b"x").unwrap();
        assert!(matches!(
            fs.remove_dir("/tmp.bin"),
            Err(FsError::NotADirectory { .. })
        ));
        assert!(fs.remove_file("/tmp.bin").is_ok());
        assert!(matches!(
            fs.remove_file("/tmp.bin"),
            Err(FsError::NotFound { .. })
        ));
        assert!(matches!(
            fs.remove_file("/audio"),
            Err(FsError::IsADirectory { .. })
        ));
    }

    #[test]
    fn remove_dir_all_clears_subtrees() {
        let mut fs = fixture();
        fs.remove_dir_all("/audio").unwrap();
        assert!(!fs.exists("/audio"));
        assert!(fs.exists("/cache"));
        assert_eq!(
            fs.paths(),
            vec![PathBuf::from("/"), PathBuf::from("/cache")]
        );
    }

    #[test]
    fn rename_moves_files_and_subtrees() {
        let mut fs = fixture();
        fs.rename("/audio/flac/sine.flac", "/cache/renamed.flac")
            .unwrap();
        assert!(!fs.exists("/audio/flac/sine.flac"));
        assert_eq!(fs.read_file("/cache/renamed.flac").unwrap(), b"fLaC....");

        fs.rename("/audio", "/media").unwrap();
        assert_eq!(
            fs.read_file("/media/wav/sine.wav").unwrap(),
            b"RIFFxxxxWAVE"
        );
        assert!(!fs.exists("/audio"));

        assert!(matches!(
            fs.rename("/media", "/cache"),
            Err(FsError::AlreadyExists { .. })
        ));
        assert!(matches!(
            fs.rename("/missing", "/elsewhere"),
            Err(FsError::NotFound { .. })
        ));
    }

    #[test]
    fn list_dir_returns_sorted_names() {
        let fs = fixture();
        assert_eq!(
            fs.list_dir("/audio").unwrap(),
            vec!["flac".to_string(), "wav".to_string()]
        );
        assert_eq!(fs.list_dir("/").unwrap().len(), 2, "audio + cache");
        assert!(matches!(
            fs.list_dir("/audio/wav/sine.wav"),
            Err(FsError::NotADirectory { .. })
        ));
        assert!(matches!(
            fs.list_dir("/nowhere"),
            Err(FsError::NotFound { .. })
        ));
    }

    #[test]
    fn clone_snapshots_a_fixture() {
        let fs = fixture();
        let mut copy = fs.clone();
        copy.write_file("/extra.bin", b"later").unwrap();
        assert!(!fs.exists("/extra.bin"));
        assert!(copy.exists("/extra.bin"));
    }

    #[test]
    fn error_display_mentions_the_path() {
        let error = FsError::NotFound {
            path: "/gone.bin".into(),
        };
        assert_eq!(error.to_string(), "no such file or directory: /gone.bin");
    }
}
