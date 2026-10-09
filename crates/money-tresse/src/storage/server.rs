//! Remote server-backed content-addressed chunk store.
//!
//! Uses end-to-end encryption (nonce || ciphertext) via `tresse_lib::crypto`
//! (re-exported from `rdx_sync::crypto`). Chunks are addressed by their content
//! hash; the server is a generic `chunkId -> encryptedBlob` key-value store.

use std::io;

use reqwest::blocking::Client;

use rdx_sync::{
    ChunkClass, ChunkId, ObjectStore, RemoteKv, SyncError, chunk_id_bytes, verify_object_bytes,
};
use tresse_lib::crypto::decode_key_b64;

pub struct ServerTresseClient {
    client: Client,
    base_url: String,
    token: String,
    repo_id: String,
}

impl ServerTresseClient {
    pub fn new(
        base_url: String,
        token: String,
        repo_id: String,
        encryption_key_b64: &str,
    ) -> Result<Self, SyncError> {
        decode_key_b64(encryption_key_b64).map_err(|error| SyncError::Other(error.to_string()))?;
        Ok(Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            repo_id,
        })
    }

    fn objects_url(&self, class: ChunkClass) -> String {
        format!(
            "{}/objects/{}/{}",
            self.base_url,
            self.repo_id,
            class.prefix()
        )
    }

    fn object_url(&self, class: ChunkClass, id: &ChunkId) -> String {
        format!("{}/{}", self.objects_url(class), id)
    }

    fn remote_key_parts<'a>(&self, key: &'a str) -> Result<(ChunkClass, &'a str), SyncError> {
        let Some((class, id)) = key.split_once('/') else {
            return Err(SyncError::Other("remote key must be class/id".to_string()));
        };
        let class = match class {
            "meta" => ChunkClass::Meta,
            "content" => ChunkClass::Content,
            _ => return Err(SyncError::Other("invalid remote object class".to_string())),
        };
        if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(SyncError::Other("invalid remote object id".to_string()));
        }
        Ok((class, id))
    }

    fn remote_url(&self, key: &str) -> Result<String, SyncError> {
        let (class, id) = self.remote_key_parts(key)?;
        Ok(self.object_url(class, &id.to_string()))
    }

    fn auth(&self, req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        req.bearer_auth(&self.token)
    }
}

impl ObjectStore for ServerTresseClient {
    fn list_objects(&self, class: ChunkClass) -> Result<Vec<ChunkId>, SyncError> {
        let response = self
            .auth(self.client.get(self.objects_url(class)))
            .send()
            .map_err(http_error)?;
        if !response.status().is_success() {
            return Err(SyncError::Other(format!(
                "remote object list failed: {}",
                response.status()
            )));
        }
        let mut ids: Vec<ChunkId> = response.json().map_err(http_error)?;
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    fn get_bytes(&self, class: ChunkClass, id: &ChunkId) -> Result<Vec<u8>, SyncError> {
        let response = self
            .auth(self.client.get(self.object_url(class, id)))
            .send()
            .map_err(http_error)?;
        if response.status().as_u16() == 404 {
            return Err(SyncError::NotFound(id.clone()));
        }
        if !response.status().is_success() {
            return Err(SyncError::Other(format!(
                "remote object get failed: {}",
                response.status()
            )));
        }
        let bytes = response.bytes().map_err(http_error)?.to_vec();
        verify_object_bytes(id, &bytes)?;
        Ok(bytes)
    }

    fn put_bytes(&mut self, class: ChunkClass, bytes: &[u8]) -> Result<ChunkId, SyncError> {
        let id = chunk_id_bytes(bytes);
        let response = self
            .auth(self.client.post(self.object_url(class, &id)))
            .body(bytes.to_vec())
            .send()
            .map_err(http_error)?;
        if response.status().is_success() {
            Ok(id)
        } else {
            Err(SyncError::Other(format!(
                "remote object put failed: {}",
                response.status()
            )))
        }
    }
}

impl RemoteKv for ServerTresseClient {
    fn list(&self, prefix: &str) -> Result<Vec<String>, SyncError> {
        let class = match prefix {
            "meta/" => ChunkClass::Meta,
            "content/" => ChunkClass::Content,
            _ => return Err(SyncError::Other("invalid remote list prefix".to_string())),
        };
        let response = self
            .auth(self.client.get(self.objects_url(class)))
            .send()
            .map_err(http_error)?;
        if !response.status().is_success() {
            return Err(SyncError::Other(format!(
                "remote kv list failed: {}",
                response.status()
            )));
        }
        let mut ids: Vec<String> = response.json().map_err(http_error)?;
        ids.sort_unstable();
        ids.dedup();
        Ok(ids.into_iter().map(|id| format!("{prefix}{id}")).collect())
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, SyncError> {
        let response = self
            .auth(self.client.get(self.remote_url(key)?))
            .send()
            .map_err(http_error)?;
        if response.status().as_u16() == 404 {
            return Err(SyncError::NotFound(key.to_string()));
        }
        if response.status().is_success() {
            Ok(response.bytes().map_err(http_error)?.to_vec())
        } else {
            Err(SyncError::Other(format!(
                "remote kv get failed: {}",
                response.status()
            )))
        }
    }

    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        let response = self
            .auth(self.client.post(self.remote_url(key)?))
            .body(value.to_vec())
            .send()
            .map_err(http_error)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(SyncError::Other(format!(
                "remote kv put failed: {}",
                response.status()
            )))
        }
    }

    fn delete(&mut self, key: &str) -> Result<(), SyncError> {
        let response = self
            .auth(self.client.delete(self.remote_url(key)?))
            .send()
            .map_err(http_error)?;
        if response.status().is_success() || response.status().as_u16() == 404 {
            Ok(())
        } else {
            Err(SyncError::Other(format!(
                "remote kv delete failed: {}",
                response.status()
            )))
        }
    }
}

fn http_error(e: reqwest::Error) -> SyncError {
    SyncError::Io(io::Error::other(format!("http error: {e}")))
}

#[cfg(test)]
mod object_store_tests {
    use super::*;

    fn client() -> ServerTresseClient {
        ServerTresseClient::new(
            "https://sync.example.com/".to_string(),
            "token".to_string(),
            "repo".to_string(),
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        )
        .unwrap()
    }

    #[test]
    fn object_urls_include_repository_and_class() {
        let client = client();
        let id = "a".repeat(64);

        assert_eq!(
            client.object_url(ChunkClass::Meta, &id),
            format!("https://sync.example.com/objects/repo/meta/{id}")
        );
        assert_eq!(
            client.object_url(ChunkClass::Content, &id),
            format!("https://sync.example.com/objects/repo/content/{id}")
        );
    }

    #[test]
    fn explicit_remote_keys_map_to_categorized_urls() {
        let client = client();
        let id = "b".repeat(64);

        assert_eq!(
            client.remote_url(&format!("meta/{id}")).unwrap(),
            format!("https://sync.example.com/objects/repo/meta/{id}")
        );
        assert!(client.remote_url(&format!("other/{id}")).is_err());
    }
}
