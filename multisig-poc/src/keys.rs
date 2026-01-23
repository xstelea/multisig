//! Test key generation and virtual badge derivation for multisig POC.
//!
//! Virtual signature badges are NonFungibleGlobalIds derived from public keys.
//! When a transaction is signed with the corresponding private key, the runtime
//! automatically creates a proof of that badge for access rule checking.

use radix_common::prelude::*;

/// A signer with their private key and derived virtual badge.
/// Note: Ed25519PrivateKey is intentionally not Clone for security.
pub struct Signer {
    pub name: String,
    pub private_key: Ed25519PrivateKey,
    pub public_key: Ed25519PublicKey,
    pub badge: NonFungibleGlobalId,
}

impl Signer {
    /// Create a new signer from a seed value.
    /// Uses Ed25519PrivateKey::from_u64 for deterministic test keys.
    pub fn from_seed(name: impl Into<String>, seed: u64) -> anyhow::Result<Self> {
        let name = name.into();
        let private_key = Ed25519PrivateKey::from_u64(seed)
            .map_err(|e| anyhow::anyhow!("Failed to create key from seed {}: {:?}", seed, e))?;
        let public_key = private_key.public_key();
        let badge = NonFungibleGlobalId::from_public_key(&public_key);

        Ok(Self {
            name,
            private_key,
            public_key,
            badge,
        })
    }

    /// Get the public key as a generic PublicKey enum.
    pub fn public_key_generic(&self) -> PublicKey {
        PublicKey::Ed25519(self.public_key.clone())
    }
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("name", &self.name)
            .field("public_key", &hex::encode(self.public_key.0))
            // Debug uses NO_NETWORK context for display
            .field("badge", &format!("{:?}", self.badge))
            .finish()
    }
}

/// Collection of signers for multisig operations.
pub struct MultisigSigners {
    pub signers: Vec<Signer>,
    pub notary: Signer,
}

impl MultisigSigners {
    /// Create 4 signers and a notary for testing.
    /// Seeds 1-4 for signers, seed 100 for notary.
    pub fn new_test_set() -> anyhow::Result<Self> {
        let signers = vec![
            Signer::from_seed("signer_1", 1)?,
            Signer::from_seed("signer_2", 2)?,
            Signer::from_seed("signer_3", 3)?,
            Signer::from_seed("signer_4", 4)?,
        ];
        let notary = Signer::from_seed("notary", 100)?;

        Ok(Self { signers, notary })
    }

    /// Get badges for all signers.
    pub fn all_badges(&self) -> Vec<NonFungibleGlobalId> {
        self.signers.iter().map(|s| s.badge.clone()).collect()
    }

    /// Get references to the first n signers (for partial signing scenarios).
    pub fn take_signers(&self, n: usize) -> Vec<&Signer> {
        self.signers.iter().take(n).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signer_creation() {
        let signer = Signer::from_seed("test", 42).unwrap();
        assert_eq!(signer.name, "test");
        // Badge should be a NonFungibleGlobalId on the Ed25519 signature resource
        let badge_str = format!("{:?}", signer.badge);
        // Debug format shows ResourceAddress
        assert!(badge_str.contains("ResourceAddress"), "badge debug: {}", badge_str);
    }

    #[test]
    fn test_multisig_signers() {
        let signers = MultisigSigners::new_test_set().unwrap();
        assert_eq!(signers.signers.len(), 4);
        assert_eq!(signers.all_badges().len(), 4);
    }

    #[test]
    fn test_deterministic_keys() {
        let signer1 = Signer::from_seed("a", 1).unwrap();
        let signer2 = Signer::from_seed("b", 1).unwrap();
        // Same seed should produce same keys
        assert_eq!(signer1.public_key.0, signer2.public_key.0);
    }
}
