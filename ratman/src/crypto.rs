//! Ratman cryptography module
//!
//! This module uses the x25519-dalek crate to computer a shared
//! diffie-helman secret between two addresses on the network.  This
//! shared key can then be used to encode and encrypt ERIS blocks.
//!
//! An address corresponds to the public key of a key pair, where the
//! private key is not shared outside the router.

// Utility imports
use crate::storage::{
    addr_key::{AddressData, EncryptedKey},
    MetadataDb,
};
use libratman::{
    types::{AddrAuth, Address, Ident32},
    ClientError, RatmanError, Result,
};
use rand::{rngs::OsRng, thread_rng, RngCore};
use std::{convert::TryInto, ffi::CString, sync::Arc};

// Cryptography imports
use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::ChaCha20;
use curve25519_dalek::edwards::CompressedEdwardsY;
use ed25519_dalek::{ExpandedSecretKey, PublicKey, SecretKey, Signature, Verifier};
use x25519_dalek::{PublicKey as X25519Pubkey, SharedSecret, StaticSecret as X25519Secret};

/// An ed25519 keypair
///
/// The public key represents an address on the Irdest network
pub struct Keypair {
    pub(crate) inner: ed25519_dalek::Keypair,
}

impl Keypair {
    pub fn new(secret: SecretKey) -> Self {
        let public = PublicKey::from(&secret);
        Self {
            inner: ed25519_dalek::Keypair { secret, public },
        }
    }

    fn to_expanded(&self) -> ExpandedSecretKey {
        ExpandedSecretKey::from(&self.inner.secret)
    }
}

/// Encrypt a data chunk with a shared secret and random nonce
///
/// The provided chunk is encrypted in place and the selected nonce (which must
/// be provided to decrypt) is returned
pub fn encrypt_chunk<const L: usize>(shared_key: &SharedSecret, chunk: &mut [u8; L]) -> [u8; 12] {
    encrypt_raw(shared_key.as_bytes(), chunk)
}

pub fn encrypt_raw(secret: &[u8; 32], data: &mut [u8]) -> [u8; 12] {
    let mut nonce = [0; 12];
    thread_rng().fill_bytes(&mut nonce);

    let mut cipher = ChaCha20::new(&(*secret).into(), &nonce.into());
    cipher.apply_keystream(data);
    nonce
}

/// Decrypt a data chunk with a shared secret and nonce
pub fn decrypt_chunk(
    shared_key: &SharedSecret,
    nonce: [u8; 12],
    mut encrypted_chunk: Vec<u8>,
) -> Vec<u8> {
    let mut cipher = ChaCha20::new(&(*shared_key.as_bytes()).into(), &nonce.into());
    cipher.apply_keystream(&mut encrypted_chunk);
    encrypted_chunk
}

pub fn decrypt_raw(secret: &[u8; 32], nonce: [u8; 12], encrypted_data: &mut Vec<u8>) {
    let mut cipher = ChaCha20::new(&(*secret).into(), &nonce.into());
    cipher.apply_keystream(encrypted_data.as_mut_slice());
}

pub fn diffie_hellman(self_keypair: &Keypair, peer: Address) -> Option<SharedSecret> {
    // Here we're taking a private key on the edwards curve and
    // transform it to a private key on the montgomery curve.
    // This is done via the `ExpandedSecretKey` type, which does
    // this transformation internally, while also adding another
    // 32 bytes of "nonce", which we discard.
    let self_x25519_secret = {
        let self_expanded = self_keypair.to_expanded();
        let self_montgomery: [u8; 32] = self_expanded.to_bytes()[..32].try_into().ok()?;
        X25519Secret::from(self_montgomery)
    };

    // The public key represents a compressed point on the edwards
    // curve, which can be decompressed, and then transformed to a
    // point on the montgomery curve.
    let peer_x25519_public = {
        let peer_compressed = CompressedEdwardsY::from_slice(peer.as_bytes());
        let peer_edwards = peer_compressed.decompress().unwrap();
        let peer_montgomery = peer_edwards.to_montgomery();
        X25519Pubkey::from(peer_montgomery.to_bytes())
    };

    // Finally we can perform a diffie hellman exchange between
    // the private sender and public recipient address keys
    Some(self_x25519_secret.diffie_hellman(&peer_x25519_public))
}

pub fn list_addr_keys(meta_db: &Arc<MetadataDb>) -> Vec<Address> {
    meta_db
        .addrs
        .iter()
        .into_iter()
        .map(|(addr, _)| Address::from_string(&addr))
        .collect()
}

pub async fn create_addr_key(
    meta_db: &Arc<MetadataDb>,
    name: Option<CString>,
) -> Result<(Address, AddrAuth)> {
    let secret = SecretKey::generate(&mut OsRng {});
    let public = PublicKey::from(&secret);
    let addr = Address::from_bytes(public.as_bytes());

    // Generate a public-private keypair
    let client_auth = AddrAuth::new();

    let mut encrypted_secret = *secret.as_bytes();
    let nonce = encrypt_raw(
        client_auth.token.as_bytes().try_into().unwrap(),
        &mut encrypted_secret,
    );

    meta_db
        .addrs
        .insert(
            addr.to_string(),
            &AddressData::Local(
                EncryptedKey {
                    encrypted: encrypted_secret.to_vec(),
                    nonce,
                },
                name,
            ),
        )
        .await?;

    Ok((addr, client_auth))
}

pub async fn get_addr_key(
    meta_db: &Arc<MetadataDb>,
    addr: Address,
    auth: AddrAuth,
) -> Result<Keypair> {
    let key_data =
        match meta_db
            .addrs
            .get(&addr.to_string())
            .await?
            .ok_or(RatmanError::Encoding(libratman::EncodingError::Encryption(
                "Address key was deleted!".into(),
            )))? {
            AddressData::Local(e, _name) => e,
            AddressData::Space(e, _name) => e,
            AddressData::Remote => unreachable!("called open_addr_key with a remote key"),
        };

    let mut decrypted_key = key_data.encrypted.clone();
    decrypt_raw(
        auth.token.as_bytes().try_into().unwrap(),
        key_data.nonce,
        &mut decrypted_key,
    );

    let secret_key = SecretKey::from_bytes(&decrypted_key)
        .map_err::<RatmanError, _>(|_| ClientError::InvalidAuth.into())?;
    let public_key = PublicKey::from(&secret_key);

    let computed_addr = Address::from_bytes(public_key.as_bytes());
    assert_eq!(computed_addr, addr);

    Ok(Keypair::new(secret_key))
}

//////// Namespace key commands

pub async fn create_namespace(
    meta_db: &Arc<MetadataDb>,
    name: Option<CString>,
    pubkey: Address,
    privkey: Ident32,
) -> Result<()> {
    let secret = SecretKey::from_bytes(privkey.as_bytes()).unwrap();
    let public = PublicKey::from(&secret);
    let addr = Address::from_bytes(public.as_bytes());
    assert_eq!(addr, pubkey);

    let mut encrypted_secret = *secret.as_bytes();
    let nonce = encrypt_raw(
        meta_db.router_id().as_bytes().try_into().unwrap(),
        &mut encrypted_secret,
    );

    meta_db
        .addrs
        .insert(
            addr.to_string(),
            &AddressData::Space(
                EncryptedKey {
                    encrypted: encrypted_secret.to_vec(),
                    nonce,
                },
                name,
            ),
        )
        .await?;

    Ok(())
}

pub async fn get_namespace_key(meta_db: &Arc<MetadataDb>, namespace: Address) -> Result<Keypair> {
    let key_data =
        match meta_db
            .addrs
            .get(&namespace.to_string())
            .await?
            .ok_or(RatmanError::Encoding(libratman::EncodingError::Encryption(
                "Address key was deleted!".into(),
            )))? {
            AddressData::Local(e, _name) => e,
            AddressData::Space(e, _name) => e,
            AddressData::Remote => unreachable!("called open_addr_key with a remote key"),
        };

    let mut decrypted_key = key_data.encrypted.clone();
    decrypt_raw(
        meta_db.router_id().as_bytes().try_into().unwrap(),
        key_data.nonce,
        &mut decrypted_key,
    );

    let secret_key = SecretKey::from_bytes(&decrypted_key)
        .map_err::<RatmanError, _>(|_| ClientError::InvalidAuth.into())?;
    let public_key = PublicKey::from(&secret_key);

    let computed_addr = Address::from_bytes(public_key.as_bytes());
    assert_eq!(computed_addr, namespace);

    Ok(Keypair::new(secret_key))
}

///////////////

/// Destroy the local address key data
pub async fn destroy_addr_key(meta_db: &Arc<MetadataDb>, addr: Address) -> Result<()> {
    meta_db.addrs.remove(addr.to_string()).await?;
    Ok(())
}

/// Destroy the local address key data
pub async fn destroy_space_key(meta_db: &Arc<MetadataDb>, addr: Address) -> Result<()> {
    meta_db.addrs.remove(addr.to_string()).await?;
    Ok(())
}

/// Verify the signature of a payload with a peer's public key (address)
#[allow(unused)] // todo
pub fn verify_message(peer: Address, msg: &[u8], signature: Signature) -> Option<()> {
    let peer_pubkey = PublicKey::from_bytes(peer.as_bytes()).ok()?;
    peer_pubkey.verify(msg, &signature).ok()
}
