//! The execution runtime : how caraxes will touch the users os.
//!
//! tools will describe a intent , this mod will be boundary that turns intent into file system or process exec
//! every tool should go through this mod (unless explicit in container and set by user (this will be done later))

pub mod file_system;
pub mod process;

use crate::error::{
    Error::{AbsolutePath, Escape},
    Result,
};

use std::{
    ffi::OsString,
    io::Result as IOResult,
    path::{Path, PathBuf},
};

pub struct WorkSpace {
    root: PathBuf,
}

impl WorkSpace {
    // open a workspace
    fn open(root: impl AsRef<Path>) -> IOResult<Self> {
        Ok(WorkSpace {
            root: root.as_ref().canonicalize()?,
        })
    }
    // get workspace root
    pub fn root(&self) -> &Path {
        &self.root
    }
    // resolve: check model supplied path
    pub fn resolve(&self, req: &str) -> Result<PathBuf> {
        let path = Path::new(req);

        if path.is_absolute() {
            return Err(AbsolutePath(req.to_string()));
        }

        let joined = self.root.join(path);
        let canonical = canonicalize_path(&joined)?;
        if !canonical.starts_with(&self.root) {
            return Err(Escape(req.to_string()));
        }

        Ok(canonical)
    }
}

fn canonicalize_path(path: &Path) -> IOResult<PathBuf> {
    let mut current = path;
    let mut tail: Vec<OsString> = Vec::new();

    loop {
        match current.canonicalize() {
            Ok(mut canonical) => {
                for component in tail.iter().rev() {
                    canonical.push(component);
                }

                return Ok(canonical);
            }
            Err(err) => {
                let Some(parent) = current.parent() else {
                    return Err(err);
                };
                if let Some(name) = current.file_name() {
                    tail.push(name.to_os_string());
                }

                current = parent;
            }
        }
    }
}

pub struct ExecutionRuntime {
    workspace: WorkSpace,
}

impl ExecutionRuntime {
    pub fn open(root: impl AsRef<Path>) -> IOResult<Self> {
        Ok(ExecutionRuntime {
            workspace: WorkSpace::open(root)?,
        })
    }

    pub fn workspace(&self) -> &WorkSpace {
        &self.workspace
    }
}
