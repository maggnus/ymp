//! Keep socket files in the metadata home, with a private short address when needed.
use anyhow::{Context, Result};
use std::{
    fs,
    os::unix::{fs::MetadataExt, fs::PermissionsExt, net::SocketAddr},
    path::{Path, PathBuf},
};
use tempfile::TempDir;
use tokio::net::UnixListener;

pub(super) struct Binding {
    pub(super) address: PathBuf,
    disk_path: PathBuf,
    owned_inode: Option<(u64, u64)>,
    _alias: Option<TempDir>,
}

pub(super) fn bind(directory: &Path) -> Result<(UnixListener, Binding)> {
    fs::create_dir_all(directory)?;
    // Native MCP children have their own working directories. Always return an
    // absolute address, including when --home was supplied as a relative path.
    let directory = directory.canonicalize()?;
    bind_path(directory.join(format!("{}.sock", &ymp_core::new_id()[..8])))
}

fn bind_path(disk_path: PathBuf) -> Result<(UnixListener, Binding)> {
    let (address, alias) = if SocketAddr::from_pathname(&disk_path).is_ok() {
        (disk_path.clone(), None)
    } else {
        // TMPDIR on macOS can itself exceed the Unix address limit. The private
        // directory contains only an alias; the socket inode stays in metadata.
        let alias = tempfile::Builder::new()
            .prefix("ymp-mcp-")
            .tempdir_in("/tmp")?;
        fs::set_permissions(alias.path(), fs::Permissions::from_mode(0o700))?;
        std::os::unix::fs::symlink(
            disk_path
                .parent()
                .context("Socket has no parent directory")?,
            alias.path().join("run"),
        )?;
        let address = alias
            .path()
            .join("run")
            .join(disk_path.file_name().context("Socket has no filename")?);
        (address, Some(alias))
    };
    let mut binding = Binding {
        address,
        disk_path,
        owned_inode: None,
        _alias: alias,
    };
    let listener =
        UnixListener::bind(&binding.address).context("Cannot bind the internal team socket")?;
    let metadata = fs::symlink_metadata(&binding.disk_path)?;
    binding.owned_inode = Some((metadata.dev(), metadata.ino()));
    fs::set_permissions(&binding.disk_path, fs::Permissions::from_mode(0o600))?;
    Ok((listener, binding))
}

impl Drop for Binding {
    fn drop(&mut self) {
        // Failed setup never owns an existing listener. Also preserve a new
        // listener that replaced our pathname after our own socket was unlinked.
        if let (Some(owned), Ok(current)) =
            (self.owned_inode, fs::symlink_metadata(&self.disk_path))
        {
            if owned == (current.dev(), current.ino()) {
                let _ = fs::remove_file(&self.disk_path);
            }
        }
        // TempDir removes only the private alias after the owned socket is gone.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::UnixStream;

    async fn round_trip(listener: &UnixListener, address: &Path, byte: u8) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let mut client = UnixStream::connect(address).await.unwrap();
            let (mut server, _) = listener.accept().await.unwrap();
            client.write_all(&[byte]).await.unwrap();
            assert_eq!(server.read_u8().await.unwrap(), byte);
            server.write_all(&[byte + 1]).await.unwrap();
            assert_eq!(client.read_u8().await.unwrap(), byte + 1);
        })
        .await
        .expect("socket round trip must finish");
    }

    #[tokio::test]
    async fn short_address_needs_no_alias_and_releases_its_socket() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (listener, binding) = bind(&home.path().join("run")).unwrap();
        assert!(binding.address.is_absolute());
        assert!(binding._alias.is_none());
        assert_eq!(binding.address, binding.disk_path);
        assert_eq!(
            fs::metadata(&binding.disk_path).unwrap().mode() & 0o777,
            0o600
        );
        round_trip(&listener, &binding.address, 3).await;
        let disk = binding.disk_path.clone();
        drop(binding);
        assert!(!disk.exists());
        assert!(home.path().join("run").is_dir());
    }

    #[tokio::test]
    async fn long_addresses_use_separate_private_aliases_and_keep_metadata() {
        let home = tempfile::tempdir().unwrap();
        let directory = home.path().join("metadata-directory-".repeat(8));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("keep"), "metadata").unwrap();
        let (first_listener, first) = bind(&directory).unwrap();
        let (second_listener, second) = bind(&directory).unwrap();
        assert!(SocketAddr::from_pathname(&first.disk_path).is_err());
        assert!(SocketAddr::from_pathname(&first.address).is_ok());
        assert_eq!(first.address.canonicalize().unwrap(), first.disk_path);
        let alias = first._alias.as_ref().unwrap().path().to_path_buf();
        assert_eq!(fs::metadata(&alias).unwrap().mode() & 0o777, 0o700);
        assert_eq!(
            fs::metadata(&first.disk_path).unwrap().mode() & 0o777,
            0o600
        );
        assert_ne!(first.address, second.address);
        assert_ne!(first.disk_path, second.disk_path);
        round_trip(&first_listener, &first.address, 4).await;
        let disk = first.disk_path.clone();
        drop(first);
        assert!(!disk.exists());
        assert!(!alias.exists());
        round_trip(&second_listener, &second.address, 5).await;
        let second_disk = second.disk_path.clone();
        let second_alias = second._alias.as_ref().unwrap().path().to_path_buf();
        drop(second);
        assert!(!second_disk.exists());
        assert!(!second_alias.exists());
        assert_eq!(
            fs::read_to_string(directory.join("keep")).unwrap(),
            "metadata"
        );
    }

    #[tokio::test]
    async fn failed_bind_does_not_unlink_the_existing_listener() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (listener, binding) = bind(home.path()).unwrap();
        assert!(bind_path(binding.disk_path.clone()).is_err());
        round_trip(&listener, &binding.address, 6).await;
    }

    #[tokio::test]
    async fn closing_an_old_binding_preserves_a_replacement_listener() {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let (_old_listener, old) = bind(home.path()).unwrap();
        let disk = old.disk_path.clone();
        fs::remove_file(&disk).unwrap();
        let (listener, replacement) = bind_path(disk.clone()).unwrap();
        drop(old);
        round_trip(&listener, &replacement.address, 7).await;
        drop(replacement);
        assert!(!disk.exists());
    }
}
