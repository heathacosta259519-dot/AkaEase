use crate::{
    error::{BackendError, Result},
    session::StoredSession,
};
use std::{future::Future, pin::Pin, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use zeroize::Zeroizing;

pub(crate) type StoreFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
pub(crate) trait CredentialStore: Send + Sync {
    fn load(&self) -> StoreFuture<'_, Option<StoredSession>>;
    fn save<'a>(&'a self, secret: &'a StoredSession) -> StoreFuture<'a, ()>;
    fn clear(&self) -> StoreFuture<'_, ()>;
}

/// libsecret's maintained CLI supplies Secret Service session encryption and prompts.
/// The secret is written to stdin, never to argv, a file or an environment variable.
pub(crate) struct SecretServiceStore;

pub(crate) struct EphemeralStore;
impl CredentialStore for EphemeralStore {
    fn load(&self) -> StoreFuture<'_, Option<StoredSession>> {
        Box::pin(async { Ok(None) })
    }
    fn save<'a>(&'a self, _: &'a StoredSession) -> StoreFuture<'a, ()> {
        Box::pin(async { Err(BackendError::CredentialStorage) })
    }
    fn clear(&self) -> StoreFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl SecretServiceStore {
    async fn execute(
        action: &str,
        secret: Option<&StoredSession>,
    ) -> Result<Option<StoredSession>> {
        Self::execute_program(
            std::path::Path::new("secret-tool"),
            action,
            secret,
            Duration::from_secs(10),
        )
        .await
    }

    async fn execute_program(
        executable: &std::path::Path,
        action: &str,
        secret: Option<&StoredSession>,
        deadline: Duration,
    ) -> Result<Option<StoredSession>> {
        let operation = async {
            let mut command = Command::new(executable);
            command.arg(action);
            if action == "store" {
                command.arg("--label=AkaNetease desktop session");
            }
            command
                .args([
                    "application",
                    "AkaNetease-desktop",
                    "credential",
                    "netease-session-v1",
                ])
                .stdin(if secret.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command
                .spawn()
                .map_err(|_| BackendError::CredentialStorage)?;
            if let Some(secret) = secret {
                let mut input = child.stdin.take().ok_or(BackendError::CredentialStorage)?;
                input
                    .write_all(&secret.0)
                    .await
                    .map_err(|_| BackendError::CredentialStorage)?;
                input
                    .shutdown()
                    .await
                    .map_err(|_| BackendError::CredentialStorage)?;
            }
            let stdout = child.stdout.take().ok_or(BackendError::CredentialStorage)?;
            let stderr = child.stderr.take().ok_or(BackendError::CredentialStorage)?;
            let read = |pipe| async move {
                let mut bytes = Zeroizing::new(Vec::new());
                tokio::io::AsyncReadExt::take(pipe, 65_537)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|_| BackendError::CredentialStorage)?;
                if bytes.len() > 65_536 {
                    return Err(BackendError::CredentialStorage);
                }
                Ok(bytes)
            };
            // Read both pipes concurrently; never let diagnostic output deadlock a child.
            let (stdout, stderr) = tokio::try_join!(
                read(Box::pin(stdout) as Pin<Box<dyn tokio::io::AsyncRead + Send>>),
                read(Box::pin(stderr) as Pin<Box<dyn tokio::io::AsyncRead + Send>>)
            )?;
            let status = child
                .wait()
                .await
                .map_err(|_| BackendError::CredentialStorage)?;
            if status.success() {
                return Ok((action == "lookup").then_some(StoredSession(stdout)));
            }
            // libsecret uses exit 1 with no diagnostic for a missing item.
            if action != "store" && status.code() == Some(1) && stderr.is_empty() {
                return Ok(None);
            }
            Err(BackendError::CredentialStorage)
        };
        tokio::time::timeout(deadline, operation)
            .await
            .map_err(|_| BackendError::CredentialStorage)?
    }
}

impl CredentialStore for SecretServiceStore {
    fn load(&self) -> StoreFuture<'_, Option<StoredSession>> {
        Box::pin(Self::execute("lookup", None))
    }
    fn save<'a>(&'a self, secret: &'a StoredSession) -> StoreFuture<'a, ()> {
        Box::pin(async move {
            Self::execute("store", Some(secret)).await?;
            Ok(())
        })
    }
    fn clear(&self) -> StoreFuture<'_, ()> {
        Box::pin(async {
            Self::execute("clear", None).await?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn secret_tool_adapter_uses_stdin_and_redacts_failures() {
        let path = std::env::temp_dir().join(format!(
            "aka-secret-tool-test-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        // Entirely synthetic child process: no D-Bus access and no credential files.
        std::fs::write(
            &path,
            r#"#!/bin/sh
case "$*" in *synthetic-secret*) exit 9;; esac
case "$1" in
  store) IFS= read -r secret; test "$secret" = 'synthetic-secret';;
  lookup) printf '%s\n' 'synthetic-secret';;
  clear) exit 1;;
  fail) printf '%s\n' 'synthetic-secret diagnostic' >&2; exit 2;;
  slow) exec sleep 5;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let secret = StoredSession(Zeroizing::new(b"synthetic-secret".to_vec()));
        let timeout = Duration::from_secs(2);
        assert!(
            SecretServiceStore::execute_program(&path, "store", Some(&secret), timeout)
                .await
                .is_ok()
        );
        let loaded = SecretServiceStore::execute_program(&path, "lookup", None, timeout)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&*loaded.0, b"synthetic-secret\n");
        assert!(
            SecretServiceStore::execute_program(&path, "clear", None, timeout)
                .await
                .unwrap()
                .is_none()
        );
        let failed = SecretServiceStore::execute_program(&path, "fail", None, timeout).await;
        assert!(matches!(failed, Err(BackendError::CredentialStorage)));
        assert!(matches!(
            SecretServiceStore::execute_program(&path, "slow", None, Duration::from_millis(50))
                .await,
            Err(BackendError::CredentialStorage)
        ));
        std::fs::remove_file(path).unwrap();
    }
}
