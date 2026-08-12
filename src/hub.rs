use crate::model::ModelId;
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;
use ureq::{Agent, AgentBuilder};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogModel {
    pub id: ModelId,
    pub repo: String,
    pub filename: String,
    pub sha256: String,
    pub size_mb: u32,
    pub license: &'static str,
}

impl CatalogModel {
    pub fn url(&self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/main/{}",
            self.repo, self.filename
        )
    }
}

impl Default for CatalogModel {
    fn default() -> Self {
        Self {
            id: ModelId::new("qwen3-4b-q4-k-m"),
            repo: "Qwen/Qwen3-4B-GGUF".to_string(),
            filename: "Qwen3-4B-Q4_K_M.gguf".to_string(),
            sha256: "7485fe6f11af29433bc51cab58009521f205840f5b4ae3a32fa7f92e8534fdf5".to_string(),
            size_mb: 2383,
            license: "apache-2.0",
        }
    }
}

pub fn default_catalog() -> CatalogModel {
    CatalogModel::default()
}

#[derive(Debug)]
pub enum HubError {
    Network(String),
    Io(String),
    HashMismatch { expected: String, actual: String },
    FileMissing(String),
}

impl fmt::Display for HubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HubError::Network(reason) => write!(f, "download failed: {reason}"),
            HubError::Io(reason) => write!(f, "io error: {reason}"),
            HubError::HashMismatch { expected, actual } => write!(
                f,
                "sha256 mismatch: expected {expected}, got {actual}"
            ),
            HubError::FileMissing(path) => write!(f, "file not found: {path}"),
        }
    }
}

impl std::error::Error for HubError {}

pub trait HttpClient {
    fn download(&self, url: &str, dest: &Path) -> Result<(), HubError>;
}

pub struct UreqClient {
    agent: Agent,
}

impl UreqClient {
    pub fn new() -> Self {
        Self {
            agent: AgentBuilder::new()
                .timeout(Duration::from_secs(600))
                .redirects(5)
                .build(),
        }
    }
}

impl Default for UreqClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for UreqClient {
    fn download(&self, url: &str, dest: &Path) -> Result<(), HubError> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| HubError::Network(e.to_string()))?;
        let mut reader = response.into_reader();
        let mut file = fs::File::create(dest).map_err(|e| HubError::Io(e.to_string()))?;
        io::copy(&mut reader, &mut file).map_err(|e| HubError::Io(e.to_string()))?;
        Ok(())
    }
}

pub struct ModelStore {
    models_dir: PathBuf,
    client: Box<dyn HttpClient>,
}

impl ModelStore {
    pub fn new(models_dir: impl Into<PathBuf>, client: Box<dyn HttpClient>) -> Self {
        Self {
            models_dir: models_dir.into(),
            client,
        }
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    pub fn verify(&self, model: &CatalogModel) -> Result<PathBuf, HubError> {
        let dest = self.models_dir.join(&model.filename);
        let actual = sha256_of(&dest)?
            .ok_or_else(|| HubError::FileMissing(dest.display().to_string()))?;
        if actual != model.sha256 {
            return Err(HubError::HashMismatch {
                expected: model.sha256.clone(),
                actual,
            });
        }
        Ok(dest)
    }

    pub fn resolve(&self, model: &CatalogModel) -> Result<PathBuf, HubError> {
        let dest = self.models_dir.join(&model.filename);
        match sha256_of(&dest)? {
            Some(actual) if actual == model.sha256 => return Ok(dest),
            _ => {}
        }

        fs::create_dir_all(&self.models_dir).map_err(|e| HubError::Io(e.to_string()))?;
        let tmp = self.models_dir.join(format!("{}.part", model.filename));
        self.client.download(&model.url(), &tmp)?;

        let actual = sha256_of(&tmp)?
            .ok_or_else(|| HubError::FileMissing(tmp.display().to_string()))?;
        if actual != model.sha256 {
            let _ = fs::remove_file(&tmp);
            return Err(HubError::HashMismatch {
                expected: model.sha256.clone(),
                actual,
            });
        }

        fs::rename(&tmp, &dest).map_err(|e| HubError::Io(e.to_string()))?;
        Ok(dest)
    }
}

pub fn sha256_of(path: &Path) -> Result<Option<String>, HubError> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(HubError::Io(e.to_string())),
    };
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| HubError::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(Some(hex(&hasher.finalize())))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    const HELLO_SHA256: &str =
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";

    fn write_hello(path: &Path) {
        fs::write(path, b"hello world").expect("write file");
    }

    #[test]
    fn sha256_of_missing_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("nope");
        assert_eq!(sha256_of(&path).expect("sha256"), None);
    }

    #[test]
    fn sha256_of_known_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("f.bin");
        write_hello(&path);
        assert_eq!(
            sha256_of(&path).expect("sha256").expect("digest"),
            HELLO_SHA256
        );
    }

    #[test]
    fn hex_formats_lowercase() {
        assert_eq!(hex(&[0xab, 0x01]), "ab01");
    }

    #[test]
    fn catalog_model_url_points_at_resolve_main() {
        let model = default_catalog();
        assert_eq!(
            model.url(),
            "https://huggingface.co/Qwen/Qwen3-4B-GGUF/resolve/main/Qwen3-4B-Q4_K_M.gguf"
        );
    }

    struct RecordingClient {
        called_with: Mutex<Option<String>>,
        bytes: &'static [u8],
    }

    impl HttpClient for RecordingClient {
        fn download(&self, url: &str, dest: &Path) -> Result<(), HubError> {
            *self.called_with.lock().expect("lock") = Some(url.to_string());
            fs::write(dest, self.bytes).map_err(|e| HubError::Io(e.to_string()))
        }
    }

    impl HttpClient for Arc<RecordingClient> {
        fn download(&self, url: &str, dest: &Path) -> Result<(), HubError> {
            (**self).download(url, dest)
        }
    }

    fn model_with_sha256(sha: &str) -> CatalogModel {
        let mut model = CatalogModel::default();
        model.sha256 = sha.to_string();
        model
    }

    #[test]
    fn verify_accepts_matching_on_disk_model_without_network() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256(HELLO_SHA256);
        let dest = dir.path().join("models").join(&model.filename);
        fs::create_dir_all(dir.path().join("models")).expect("mkdir");
        write_hello(&dest);

        let verified = store.verify(&model).expect("verify");
        assert_eq!(verified, dest);
        assert_eq!(*client.called_with.lock().expect("lock"), None);
    }

    #[test]
    fn verify_errors_when_model_is_missing() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256(HELLO_SHA256);

        let err = store.verify(&model).expect_err("missing");
        assert!(matches!(err, HubError::FileMissing(_)));
        assert_eq!(*client.called_with.lock().expect("lock"), None);
    }

    #[test]
    fn verify_rejects_mismatched_hash() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256("0000000000000000000000000000000000000000000000000000000000000000");
        let dest = dir.path().join("models").join(&model.filename);
        fs::create_dir_all(dir.path().join("models")).expect("mkdir");
        write_hello(&dest);

        let err = store.verify(&model).expect_err("mismatch");
        assert!(matches!(err, HubError::HashMismatch { .. }));
        assert_eq!(*client.called_with.lock().expect("lock"), None);
    }

    #[test]
    fn resolve_uses_cached_file_without_downloading() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256(HELLO_SHA256);
        let cached = dir.path().join("models").join(&model.filename);
        fs::create_dir_all(dir.path().join("models")).expect("mkdir");
        write_hello(&cached);

        let resolved = store.resolve(&model).expect("resolve");
        assert_eq!(resolved, cached);
        assert_eq!(*client.called_with.lock().expect("lock"), None);
    }

    #[test]
    fn resolve_downloads_and_verifies() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256(HELLO_SHA256);

        let resolved = store.resolve(&model).expect("resolve");
        assert_eq!(resolved, dir.path().join("models").join(&model.filename));
        assert_eq!(*client.called_with.lock().expect("lock"), Some(model.url()));
        assert!(resolved.exists());
    }

    #[test]
    fn resolve_rejects_mismatched_hash() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256("0000000000000000000000000000000000000000000000000000000000000000");

        let err = store.resolve(&model).expect_err("mismatch");
        assert!(matches!(err, HubError::HashMismatch { .. }));
    }

    #[test]
    fn resolve_redownloads_when_cached_file_is_stale() {
        let client = Arc::new(RecordingClient {
            called_with: Mutex::new(None),
            bytes: b"hello world",
        });
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ModelStore::new(dir.path().join("models"), Box::new(Arc::clone(&client)));
        let model = model_with_sha256(HELLO_SHA256);
        let cached = dir.path().join("models").join(&model.filename);
        fs::create_dir_all(dir.path().join("models")).expect("mkdir");
        fs::write(&cached, b"stale bytes").expect("write stale");

        let resolved = store.resolve(&model).expect("resolve");
        assert_eq!(*client.called_with.lock().expect("lock"), Some(model.url()));
        assert_eq!(fs::read(&resolved).expect("read"), b"hello world");
    }
}
