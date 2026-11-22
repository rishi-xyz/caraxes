use std::path::PathBuf;

use tokio::fs::{create_dir_all, read_to_string, remove_file, write};
use tracing::{debug, instrument};

use super::ExecutionRuntime;
use crate::error::Result;

impl ExecutionRuntime {
    // read a file
    #[instrument(skip_all)]
    pub async fn read_file(&self, path: &str) -> Result<String> {
        let full = self.workspace().resolve(path)?;
        debug!(%path, resolved = %full.display(), "reading file");
        let contents = read_to_string(full).await?;
        debug!(bytes = contents.len(), "file read complete");
        Ok(contents)
    }
    // write a file
    pub async fn write_file(&self, path: &str, contents: impl AsRef<[u8]>) -> Result<()> {
        let full = self.workspace().resolve(path)?;
        write(full, contents).await?;
        Ok(())
    }
    // create a dir
    pub async fn create_dir_all(&self, path: &str) -> Result<()> {
        let full = self.workspace().resolve(path)?;
        create_dir_all(full).await?;
        Ok(())
    }
    // remove file
    pub async fn remove_file(&self, path: &str) -> Result<()> {
        let full = self.workspace().resolve(path)?;
        remove_file(full).await?;
        Ok(())
    }
    // rename
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let from = self.workspace().resolve(from)?;
        let to = self.workspace().resolve(to)?;
        tokio::fs::rename(from, to).await?;
        Ok(())
    }
    // metadata for a path inside the workspace
    pub async fn metadata(&self, path: &str) -> Result<std::fs::Metadata> {
        let full = self.workspace().resolve(path)?;
        Ok(tokio::fs::metadata(full).await?)
    }
    // resolve path
    pub fn resolve_path(&self, path: &str) -> Result<PathBuf> {
        self.workspace().resolve(path)
    }
}
