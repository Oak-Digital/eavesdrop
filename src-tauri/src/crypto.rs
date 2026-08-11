use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use base64::{Engine, engine::general_purpose::STANDARD};

use crate::error::{AppError, AppResult};

const ASSET_MAGIC: &[u8; 8] = b"EAVAS001";
const JOURNAL_MAGIC: &[u8; 8] = b"EAVJR001";
const BLOCK_SIZE: usize = 1024 * 1024;

#[derive(Clone)]
pub struct Vault {
    root: PathBuf,
    master_key: [u8; 32],
}

#[derive(Clone)]
pub struct RecordingKey {
    pub plain: [u8; 32],
    pub wrapped: Vec<u8>,
}

impl Vault {
    pub fn open(root: PathBuf) -> AppResult<Self> {
        fs::create_dir_all(&root)?;
        let entry = keyring::Entry::new("com.eavesdrop.recorder", "library-master-key")
            .map_err(|error| AppError::Crypto(error.to_string()))?;
        let master_key = match entry.get_password() {
            Ok(value) => {
                let decoded = STANDARD
                    .decode(value)
                    .map_err(|error| AppError::Crypto(error.to_string()))?;
                decoded
                    .try_into()
                    .map_err(|_| AppError::Crypto("invalid master key length".into()))?
            }
            Err(keyring::Error::NoEntry) => {
                let key = random_array::<32>();
                entry
                    .set_password(&STANDARD.encode(key))
                    .map_err(|error| AppError::Crypto(error.to_string()))?;
                key
            }
            Err(error) => return Err(AppError::Crypto(error.to_string())),
        };
        Ok(Self { root, master_key })
    }

    #[cfg(test)]
    pub fn with_master(root: PathBuf, master_key: [u8; 32]) -> AppResult<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root, master_key })
    }

    pub fn create_recording_key(&self) -> AppResult<RecordingKey> {
        let plain = random_array::<32>();
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|error| AppError::Crypto(error.to_string()))?;
        let nonce = random_array::<12>();
        let ciphertext = cipher
            .encrypt((&nonce).into(), plain.as_ref())
            .map_err(|error| AppError::Crypto(error.to_string()))?;
        let mut wrapped = nonce.to_vec();
        wrapped.extend_from_slice(&ciphertext);
        Ok(RecordingKey { plain, wrapped })
    }

    pub fn unwrap_key(&self, wrapped: &[u8]) -> AppResult<[u8; 32]> {
        if wrapped.len() < 13 {
            return Err(AppError::Crypto(
                "wrapped recording key is truncated".into(),
            ));
        }
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|error| AppError::Crypto(error.to_string()))?;
        let plain = cipher
            .decrypt((&wrapped[..12]).into(), &wrapped[12..])
            .map_err(|_| AppError::Crypto("recording key authentication failed".into()))?;
        plain
            .try_into()
            .map_err(|_| AppError::Crypto("invalid recording key length".into()))
    }

    pub fn asset_path(&self, recording_id: &str) -> PathBuf {
        self.root.join(format!("{recording_id}.eav"))
    }

    pub fn journal_path(&self, recording_id: &str) -> PathBuf {
        self.root.join(format!("{recording_id}.journal"))
    }

    pub fn seal_asset(
        &self,
        recording_id: &str,
        key: &[u8; 32],
        data: &[u8],
    ) -> AppResult<PathBuf> {
        let final_path = self.asset_path(recording_id);
        let pending_path = final_path.with_extension("pending");
        let mut file = File::create(&pending_path)?;
        file.write_all(ASSET_MAGIC)?;
        file.write_all(&(BLOCK_SIZE as u32).to_le_bytes())?;
        file.write_all(&(data.len() as u64).to_le_bytes())?;
        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|error| AppError::Crypto(error.to_string()))?;
        for block in data.chunks(BLOCK_SIZE) {
            let nonce = random_array::<12>();
            let encrypted = cipher
                .encrypt((&nonce).into(), block)
                .map_err(|error| AppError::Crypto(error.to_string()))?;
            file.write_all(&nonce)?;
            file.write_all(&(encrypted.len() as u32).to_le_bytes())?;
            file.write_all(&encrypted)?;
        }
        file.sync_all()?;
        fs::rename(&pending_path, &final_path)?;
        Ok(final_path)
    }

    pub fn open_asset(&self, path: &Path, key: &[u8; 32]) -> AppResult<Vec<u8>> {
        let mut file = File::open(path)?;
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != ASSET_MAGIC {
            return Err(AppError::Crypto("invalid encrypted asset header".into()));
        }
        let mut u32_buf = [0u8; 4];
        let mut u64_buf = [0u8; 8];
        file.read_exact(&mut u32_buf)?;
        let block_size = u32::from_le_bytes(u32_buf) as usize;
        if block_size == 0 || block_size > BLOCK_SIZE * 2 {
            return Err(AppError::Crypto(
                "invalid encrypted asset block size".into(),
            ));
        }
        file.read_exact(&mut u64_buf)?;
        let expected_len = u64::from_le_bytes(u64_buf) as usize;
        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|error| AppError::Crypto(error.to_string()))?;
        let mut output = Vec::with_capacity(expected_len);
        while output.len() < expected_len {
            let mut nonce = [0u8; 12];
            file.read_exact(&mut nonce)?;
            file.read_exact(&mut u32_buf)?;
            let encrypted_len = u32::from_le_bytes(u32_buf) as usize;
            if encrypted_len < 16 || encrypted_len > block_size + 16 {
                return Err(AppError::Crypto("invalid encrypted block length".into()));
            }
            let mut encrypted = vec![0u8; encrypted_len];
            file.read_exact(&mut encrypted)?;
            let plain = cipher
                .decrypt((&nonce).into(), encrypted.as_ref())
                .map_err(|_| AppError::Crypto("asset block authentication failed".into()))?;
            output.extend_from_slice(&plain);
        }
        output.truncate(expected_len);
        Ok(output)
    }

    pub fn append_journal_packet(path: &Path, key: &[u8; 32], packet: &[u8]) -> AppResult<()> {
        let exists = path.exists();
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        if !exists {
            file.write_all(JOURNAL_MAGIC)?;
        }
        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|error| AppError::Crypto(error.to_string()))?;
        let nonce = random_array::<12>();
        let encrypted = cipher
            .encrypt((&nonce).into(), packet)
            .map_err(|error| AppError::Crypto(error.to_string()))?;
        file.write_all(&nonce)?;
        file.write_all(&(encrypted.len() as u32).to_le_bytes())?;
        file.write_all(&encrypted)?;
        file.flush()?;
        Ok(())
    }

    pub fn read_journal(path: &Path, key: &[u8; 32]) -> AppResult<Vec<Vec<u8>>> {
        let mut file = File::open(path)?;
        let mut magic = [0u8; 8];
        file.read_exact(&mut magic)?;
        if &magic != JOURNAL_MAGIC {
            return Err(AppError::Crypto("invalid recovery journal header".into()));
        }
        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|error| AppError::Crypto(error.to_string()))?;
        let mut packets = Vec::new();
        loop {
            let mut nonce = [0u8; 12];
            match file.read_exact(&mut nonce) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(error) => return Err(error.into()),
            }
            let mut len_buf = [0u8; 4];
            if file.read_exact(&mut len_buf).is_err() {
                break;
            }
            let encrypted_len = u32::from_le_bytes(len_buf) as usize;
            if !(16..=1024 * 1024).contains(&encrypted_len) {
                return Err(AppError::Crypto("invalid recovery packet length".into()));
            }
            let mut encrypted = vec![0u8; encrypted_len];
            if file.read_exact(&mut encrypted).is_err() {
                break;
            }
            let plain = cipher
                .decrypt((&nonce).into(), encrypted.as_ref())
                .map_err(|_| AppError::Crypto("recovery packet authentication failed".into()))?;
            packets.push(plain);
        }
        Ok(packets)
    }
}

fn random_array<const N: usize>() -> [u8; N] {
    let mut value = [0u8; N];
    OsRng.fill_bytes(&mut value);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_assets_round_trip_multiple_blocks() {
        let temp = tempfile::tempdir().unwrap();
        let vault = Vault::with_master(temp.path().to_path_buf(), [7; 32]).unwrap();
        let recording_key = vault.create_recording_key().unwrap();
        let source = vec![42u8; BLOCK_SIZE + 91];
        let path = vault
            .seal_asset("test", &recording_key.plain, &source)
            .unwrap();
        let opened = vault.open_asset(&path, &recording_key.plain).unwrap();
        assert_eq!(source, opened);
        assert_eq!(
            recording_key.plain,
            vault.unwrap_key(&recording_key.wrapped).unwrap()
        );
    }

    #[test]
    fn journal_ignores_an_incomplete_last_packet() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("capture.journal");
        let key = [3; 32];
        Vault::append_journal_packet(&path, &key, b"first").unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&[1, 2, 3])
            .unwrap();
        assert_eq!(
            Vault::read_journal(&path, &key).unwrap(),
            vec![b"first".to_vec()]
        );
    }
}
