//! File storage abstraction: local disk in development and self-hosting, any S3-compatible service
//! (AWS S3, Cloudflare R2, MinIO, Backblaze B2) in production, memory in tests.

use std::sync::Arc;

use object_store::aws::AmazonS3Builder;
use object_store::local::LocalFileSystem;
use object_store::memory::InMemory;
use object_store::path::Path;
use object_store::{ObjectStore, ObjectStoreExt, PutPayload};

use crate::config::StorageConfig;

#[derive(Clone, Debug)]
pub struct Storage {
    store: Arc<dyn ObjectStore>,
}

impl Storage {
    pub fn from_config(config: &StorageConfig) -> anyhow::Result<Self> {
        let store: Arc<dyn ObjectStore> = match config {
            StorageConfig::Local { path } => {
                std::fs::create_dir_all(path)?;
                Arc::new(LocalFileSystem::new_with_prefix(path)?.with_automatic_cleanup(true))
            }
            StorageConfig::S3(s3) => {
                let mut builder = AmazonS3Builder::new()
                    .with_bucket_name(&s3.bucket)
                    .with_region(&s3.region)
                    .with_virtual_hosted_style_request(!s3.path_style);
                if let Some(endpoint) = &s3.endpoint {
                    builder = builder
                        .with_endpoint(endpoint)
                        .with_allow_http(endpoint.starts_with("http://"));
                }
                if let (Some(key), Some(secret)) = (&s3.access_key_id, &s3.secret_access_key) {
                    builder = builder
                        .with_access_key_id(key)
                        .with_secret_access_key(secret);
                }
                Arc::new(builder.build()?)
            }
            StorageConfig::Memory => Arc::new(InMemory::new()),
        };
        Ok(Self { store })
    }

    pub async fn put(&self, key: &str, bytes: Vec<u8>) -> anyhow::Result<()> {
        self.store
            .put(&Path::parse(key)?, PutPayload::from(bytes))
            .await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        match self.store.get(&Path::parse(key)?).await {
            Ok(result) => Ok(Some(result.bytes().await?.to_vec())),
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Deletes an object; deleting something already gone is not an error.
    pub async fn delete(&self, key: &str) -> anyhow::Result<()> {
        match self.store.delete(&Path::parse(key)?).await {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// Image and document types accepted for uploads, detected from the file's leading bytes rather than
/// the client-supplied name or content type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Png,
    Jpeg,
    Webp,
    Pdf,
}

impl FileKind {
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Self::Webp)
        } else if bytes.starts_with(b"%PDF-") {
            Some(Self::Pdf)
        } else {
            None
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Pdf => "application/pdf",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Pdf => "pdf",
        }
    }

    pub fn from_extension(key: &str) -> Option<Self> {
        match key.rsplit('.').next()? {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "pdf" => Some(Self::Pdf),
            _ => None,
        }
    }

    pub fn is_image(self) -> bool {
        !matches!(self, Self::Pdf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffing_uses_magic_bytes() {
        assert_eq!(
            FileKind::sniff(b"\x89PNG\r\n\x1a\nrest"),
            Some(FileKind::Png)
        );
        assert_eq!(
            FileKind::sniff(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(FileKind::Jpeg)
        );
        assert_eq!(
            FileKind::sniff(b"RIFF\0\0\0\0WEBPVP8 "),
            Some(FileKind::Webp)
        );
        assert_eq!(FileKind::sniff(b"%PDF-1.7"), Some(FileKind::Pdf));
        assert_eq!(FileKind::sniff(b"<svg onload=alert(1)>"), None);
        assert_eq!(FileKind::from_extension("a/b/c.webp"), Some(FileKind::Webp));
    }

    #[tokio::test]
    async fn memory_round_trip() {
        let storage = Storage::from_config(&StorageConfig::Memory).expect("memory store");
        storage.put("a/b.png", vec![1, 2, 3]).await.expect("put");
        assert_eq!(
            storage.get("a/b.png").await.expect("get"),
            Some(vec![1, 2, 3])
        );
        storage.delete("a/b.png").await.expect("delete");
        storage.delete("a/b.png").await.expect("idempotent delete");
        assert_eq!(storage.get("a/b.png").await.expect("get"), None);
    }
}
