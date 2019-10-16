/// Implements
/// https://eprint.iacr.org/2018/483 and
/// https://crypto.stanford.edu/~dabo/pubs/papers/BLSmultisig.html
use amcl_wrapper::{
    constants::{GroupG1_SIZE, MODBYTES},
    extension_field_gt::GT,
    field_elem::FieldElement,
    group_elem::GroupElement,
    group_elem_g1::G1,
    group_elem_g2::G2,
    types_g2::GroupG2_SIZE,
};
use CryptoError;

pub const PRIVATE_KEY_SIZE: usize = MODBYTES;
/// This is a simple alias so the consumer can just use PrivateKey::random() to generate a new one
/// instead of wrapping it as a private field
pub type PrivateKey = FieldElement;

macro_rules! public_key_impl {
    () => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct PublicKey(Generator);

        impl PublicKey {
            pub fn new(sk: &PrivateKey, g: &Generator) -> Self {
                PublicKey(g * sk)
            }

            // Create an combined public key without rogue key mitigation
            pub fn combine(&mut self, pks: &[PublicKey]) {
                for pk in pks {
                    self.0 += &pk.0;
                }
            }

            pub fn to_bytes(&self) -> Vec<u8> {
                self.0.to_bytes()
            }

            pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
                Ok(PublicKey(Generator::from_bytes(bytes).map_err(|e| {
                    CryptoError::ParseError(format!("{:?}", e))
                })?))
            }
        }
    };
}

macro_rules! aggregate_public_key_impl {
    () => {
        /// Represents an aggregated BLS public key that mitigates the rogue key attack
        /// for verifying aggregated signatures.
        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct AggregatedPublicKey(Generator);

        impl From<&[PublicKey]> for AggregatedPublicKey {
            fn from(keys: &[PublicKey]) -> Self {
                // To combat the rogue key attack,
                // compute (t_1,…,t_n)←H1(pk_1,…,pk_n) ∈ R_n
                // output the aggregated public key
                // as described in section 3.1 from https://eprint.iacr.org/2018/483
                let mut bytes = Vec::new();
                for k in keys {
                    bytes.extend_from_slice(k.to_bytes().as_slice());
                }
                AggregatedPublicKey(keys.iter().fold(Generator::identity(), |apk, k| {
                    // The position of the ith public key in the byte array
                    // of the hash doesn't matter as much as its included twice.
                    // For convenience, its appended to the end
                    let mut h = bytes.clone();
                    h.extend_from_slice(k.0.to_bytes().as_slice());
                    apk + &k.0 * &FieldElement::from_msg_hash(h.as_slice())
                }))
            }
        }

        impl AggregatedPublicKey {
            pub fn new(keys: &[PublicKey]) -> Self {
                keys.into()
            }

            pub fn to_bytes(&self) -> Vec<u8> {
                self.0.to_bytes()
            }

            pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
                Ok(AggregatedPublicKey(Generator::from_bytes(bytes).map_err(
                    |e| CryptoError::ParseError(format!("{:?}", e)),
                )?))
            }
        }
    };
}

macro_rules! generate_impl {
    () => {
        /// Creates a new BLS key pair
        pub fn generate(g: &Generator) -> (PublicKey, PrivateKey) {
            let sk = PrivateKey::random();
            let pk = PublicKey::new(&sk, g);
            (pk, sk)
        }
    };
}

macro_rules! bls_tests_impl {
    () => {
        #[cfg(test)]
        mod tests {
            use super::*;

            const MESSAGE_1: &[u8; 22] = b"This is a test message";
            const MESSAGE_2: &[u8; 20] = b"Another test message";

            #[test]
            fn signature_verification() {
                let g = Generator::generator();
                let (pk, sk) = generate(&g);

                let signature_1 = Signature::new(&MESSAGE_1[..], &sk);
                assert!(signature_1.verify(&MESSAGE_1[..], &pk, &g));

                let signature_2 = Signature::new(&MESSAGE_2[..], &sk);
                assert!(signature_2.verify(&MESSAGE_2[..], &pk, &g));

                // Should fail for different messages
                assert!(!signature_1.verify(&MESSAGE_2[..], &pk, &g));
                assert!(!signature_2.verify(&MESSAGE_1[..], &pk, &g));
            }

            #[test]
            fn aggregate_signature_verification_rk() {
                const KEY_COUNT: usize = 10;

                let g = Generator::generator();
                let mut pks = Vec::new();
                let mut sks = Vec::new();
                let mut asigs = Vec::new();
                for _ in 0..KEY_COUNT {
                    let (pk, sk) = generate(&g);

                    pks.push(pk);
                    sks.push(sk);
                }

                for i in 0..KEY_COUNT {
                    let sig = Signature::new_with_rk_mitigation(
                        &MESSAGE_1[..],
                        &sks[i],
                        i,
                        pks.as_slice(),
                    );
                    asigs.push(sig);
                }

                let apk = AggregatedPublicKey::new(pks.as_slice());
                let asg = AggregatedSignature::new(asigs.as_slice());
                assert!(asg.verify(&MESSAGE_1[..], &apk, &g));

                // Can't verify individually because of rogue key mitigation
                for i in 0..KEY_COUNT {
                    assert!(!asigs[i].verify(&MESSAGE_1[..], &pks[i], &g));
                }
            }

            #[test]
            fn aggregate_signature_verification_no_rk() {
                const KEY_COUNT: usize = 10;

                let g = Generator::generator();
                let mut pks = Vec::new();
                let mut sks = Vec::new();
                let mut sigs = Vec::new();
                for _ in 0..KEY_COUNT {
                    let (pk, sk) = generate(&g);

                    pks.push(pk);
                    sks.push(sk);
                }

                for i in 0..KEY_COUNT {
                    let sig = Signature::new(&MESSAGE_1[..], &sks[i]);
                    sigs.push(sig);
                }

                let asg = AggregatedSignature::new(sigs.as_slice());
                assert!(asg.verify_no_rk(&MESSAGE_1[..], pks.as_slice(), &g));

                // Check that simple aggregation without rogue key mitigation fails
                let apk = AggregatedPublicKey::new(pks.as_slice());
                assert!(!asg.verify(&MESSAGE_1[..], &apk, &g));

                // Can verify individually because of no rogue key mitigation
                for i in 0..KEY_COUNT {
                    assert!(sigs[i].verify(&MESSAGE_1[..], &pks[i], &g));
                }
            }

            #[test]
            fn batch_signature_verification() {
                const KEY_COUNT: usize = 10;
                const SIG_COUNT: usize = 5;

                // First batch verification with rogue key mitigation
                let g = Generator::generator();
                let mut groups_1 = Vec::new();
                for _ in 0..SIG_COUNT {
                    let mut sks = Vec::new();
                    let mut pks = Vec::new();
                    let mut sigs = Vec::new();
                    let msg = FieldElement::random();
                    for _ in 0..KEY_COUNT {
                        let (pk, sk) = generate(&g);
                        pks.push(pk);
                        sks.push(sk);
                    }

                    for i in 0..KEY_COUNT {
                        let sig = Signature::new_with_rk_mitigation(
                            msg.to_bytes().as_slice(),
                            &sks[i],
                            i,
                            pks.as_slice(),
                        );
                        sigs.push(sig);
                    }

                    let asg = AggregatedSignature::new(sigs.as_slice());
                    let apk = AggregatedPublicKey::new(pks.as_slice());
                    //sanity check
                    assert!(asg.verify(msg.to_bytes().as_slice(), &apk, &g));
                    groups_1.push((msg.to_bytes(), asg, apk));
                }

                let refs = groups_1
                    .iter()
                    .map(|(m, s, p)| (m.as_slice(), s, p))
                    .collect::<Vec<(&[u8], &AggregatedSignature, &AggregatedPublicKey)>>();
                assert!(AggregatedSignature::batch_verify(refs.as_slice(), &g));

                // Second batch verification without rogue key mitigation
                let mut groups_2 = Vec::new();
                for _ in 0..SIG_COUNT {
                    let mut sks = Vec::new();
                    let mut pks = Vec::new();
                    let mut sigs = Vec::new();
                    let msg = FieldElement::random();
                    for _ in 0..KEY_COUNT {
                        let (pk, sk) = generate(&g);
                        pks.push(pk);
                        sks.push(sk);
                    }

                    for i in 0..KEY_COUNT {
                        let sig = Signature::new(msg.to_bytes().as_slice(), &sks[i]);
                        sigs.push(sig);
                    }

                    let mut asg = sigs[0].clone();
                    asg.combine(&sigs[1..]);

                    let mut apk = pks[0].clone();
                    apk.combine(&pks[1..]);

                    //sanity check
                    assert!(asg.verify(msg.to_bytes().as_slice(), &apk, &g));
                    groups_2.push((msg.to_bytes(), asg, apk));
                }

                let refs = groups_2
                    .iter()
                    .map(|(m, s, p)| (m.as_slice(), s, p))
                    .collect::<Vec<(&[u8], &Signature, &PublicKey)>>();
                assert!(Signature::batch_verify(refs.as_slice(), &g));
            }

            #[test]
            fn multi_signature_verification() {
                const KEY_COUNT: usize = 10;

                let g = Generator::generator();
                let mut pks = Vec::new();
                let mut sks = Vec::new();
                let mut sigs = Vec::new();
                let mut msgs = Vec::new();
                for _ in 0..KEY_COUNT {
                    let (pk, sk) = generate(&g);

                    let msg = FieldElement::random();
                    let sig = Signature::new(msg.to_bytes().as_slice(), &sk);

                    pks.push(pk);
                    sks.push(sk);
                    sigs.push(sig);
                    msgs.push(msg.to_bytes());
                }
                let mut sig = sigs[0].clone();
                sig.combine(&sigs[1..]);
                let inputs = msgs
                    .iter()
                    .zip(pks.iter())
                    .map(|(msg, pk)| (msg.as_slice(), pk))
                    .collect::<Vec<(&[u8], &PublicKey)>>();

                assert!(sig.verify_multi(inputs.as_slice(), &g));
                msgs[0] = msgs[1].clone();
                let inputs = msgs
                    .iter()
                    .zip(pks.iter())
                    .map(|(msg, pk)| (msg.as_slice(), pk))
                    .collect::<Vec<(&[u8], &PublicKey)>>();
                assert!(!sig.verify_multi(inputs.as_slice(), &g));
            }
        }
    };
}

pub mod prelude {
    pub use super::{small::*, PrivateKey};
}

/// This version is the small BLS signature scheme
/// with the public key group in G1 and signature group in G2.
/// 192 byte signatures and 97 byte public keys
pub mod normal {
    use super::*;

    pub const PUBLIC_KEY_SIZE: usize = GroupG1_SIZE;
    pub const SIGNATURE_SIZE: usize = GroupG2_SIZE;

    pub type Generator = G1;
    pub type SignatureGroup = G2;

    public_key_impl!();

    aggregate_public_key_impl!();

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Signature(SignatureGroup);

    impl Signature {
        pub fn new(message: &[u8], sk: &PrivateKey) -> Self {
            Signature(&SignatureGroup::from_msg_hash(message) * sk)
        }

        pub fn new_with_rk_mitigation(
            message: &[u8],
            sk: &PrivateKey,
            pk_index: usize,
            pks: &[PublicKey],
        ) -> Self {
            // To combat the rogue key attack,
            // compute (t_1,…,t_n)←H1(pk_1,…,pk_n) ∈ R_n
            // output the aggregated public key
            // as described in section 3.1 from https://eprint.iacr.org/2018/483
            let mut bytes = Vec::new();
            for k in pks {
                bytes.extend_from_slice(k.to_bytes().as_slice());
            }
            bytes.extend_from_slice(pks[pk_index].to_bytes().as_slice());
            let a = FieldElement::from_msg_hash(bytes.as_slice());
            Signature(SignatureGroup::from_msg_hash(message) * sk * &a)
        }

        // Collects multiple signatures into a single signature
        // Verified by using `verify_multi`. This method does not
        // directly mitigate the rogue key attack. It is expected the caller
        // handles this using other techniques like proof of possession
        pub fn combine(&mut self, signatures: &[Signature]) {
            for sig in signatures {
                self.0 += &sig.0;
            }
        }

        // Verify a signature generated by `new`
        pub fn verify(&self, message: &[u8], pk: &PublicKey, g: &Generator) -> bool {
            let hash = SignatureGroup::from_msg_hash(message);
            GT::ate_2_pairing(&-g, &self.0, &pk.0, &hash).is_one()
        }

        // Caller should aggregate all signatures into `self` by using `combine`.
        // Messages must be distinct
        // `inputs` is a slice of message - public key tuples
        // Multisignature verification
        pub fn verify_multi(&self, inputs: &[(&[u8], &PublicKey)], g: &Generator) -> bool {
            let mut msg_check = ::std::collections::HashSet::new();
            let mut pairs = Vec::new();
            for (msg, pk) in inputs {
                let hash = SignatureGroup::from_msg_hash(&msg);
                if msg_check.contains(&hash) {
                    return false;
                }
                pairs.push((pk.0.clone(), hash.clone()));
                msg_check.insert(hash);
            }

            pairs.push((-g, self.0.clone()));
            let ate_pairs = pairs.iter().map(|(g1, g2)| (g1, g2)).collect();
            GT::ate_multi_pairing(ate_pairs).is_one()
        }

        pub fn batch_verify(inputs: &[(&[u8], &Signature, &PublicKey)], g: &Generator) -> bool {
            // To avoid rogue key attacks, you must use proof of possession or `AggregateSignature::batch_verify`
            // This function just avoids checking for distinct messages and
            // uses batch verification as described in the end of section 3.1 from https://eprint.iacr.org/2018/483
            let mut pairs = Vec::new();
            let mut sig = SignatureGroup::identity();
            for (msg, asg, apk) in inputs {
                let random_exponent = FieldElement::random();
                let hash = SignatureGroup::from_msg_hash(msg);
                sig += &asg.0 * &random_exponent;
                pairs.push((&apk.0 * &random_exponent, hash));
            }

            pairs.push((-g, sig));

            let ate_pairs = pairs.iter().map(|(g1, g2)| (g1, g2)).collect();
            GT::ate_multi_pairing(ate_pairs).is_one()
        }

        pub fn to_bytes(&self) -> Vec<u8> {
            self.0.to_bytes()
        }

        pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
            Ok(Signature(SignatureGroup::from_bytes(bytes).map_err(
                |e| CryptoError::ParseError(format!("{:?}", e)),
            )?))
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AggregatedSignature(SignatureGroup);

    impl AggregatedSignature {
        // `Signature` should be generated by calling `Signature::new_with_rk_mitigation`
        // to avoid rogue key attacks. If using proof of possession mitigation
        // then `Signature` can be generated by calling `Signature::new`
        pub fn new(signatures: &[Signature]) -> Self {
            AggregatedSignature(
                signatures
                    .iter()
                    .fold(SignatureGroup::identity(), |sig, s| sig + &s.0),
            )
        }

        // Verify with rogue key attack mitigation.
        pub fn verify(&self, message: &[u8], pk: &AggregatedPublicKey, g: &Generator) -> bool {
            let hash = SignatureGroup::from_msg_hash(message);
            GT::ate_2_pairing(&-g, &self.0, &pk.0, &hash).is_one()
        }

        // Verify without rogue key mitigation. Assumes caller has handled
        // rogue key mitigation some other way like proof of possession.
        // This practice is discouraged in favor of the other method
        // but there are use cases where proof of possession is better suited
        pub fn verify_no_rk(&self, message: &[u8], pks: &[PublicKey], g: &Generator) -> bool {
            let apk = pks.iter().fold(Generator::identity(), |a, p| a + &p.0);
            let hash = SignatureGroup::from_msg_hash(message);
            GT::ate_2_pairing(&-g, &self.0, &apk, &hash).is_one()
        }

        /// This should be used to verify quickly multiple BLS aggregated signatures by batching
        /// versus verifying them one by one as it reduces the number of computed pairings
        pub fn batch_verify(
            inputs: &[(
                &[u8], /* message */
                &AggregatedSignature,
                &AggregatedPublicKey,
            )],
            g: &Generator,
        ) -> bool {
            // To combat the rogue key attack and avoid checking for distinct messages
            // use batch verification as described in the end of section 3.1 from https://eprint.iacr.org/2018/483
            let mut pairs = Vec::new();
            let mut sig = SignatureGroup::identity();
            for (msg, asg, apk) in inputs {
                let random_exponent = FieldElement::random();
                let hash = SignatureGroup::from_msg_hash(msg);
                sig += &asg.0 * &random_exponent;
                pairs.push((&apk.0 * &random_exponent, hash));
            }

            pairs.push((-g, sig));

            let ate_pairs = pairs.iter().map(|(g1, g2)| (g1, g2)).collect();
            GT::ate_multi_pairing(ate_pairs).is_one()
        }
    }

    generate_impl!();

    bls_tests_impl!();
}

/// This version is the small BLS signature scheme
/// with the public key group in G2 and signature group in G1.
/// 97 bytes signatures and 192 byte public keys
///
/// This results in smaller signatures but slower operations and bigger public key.
/// This is good for situations where space is a consideration and verification is infrequent
pub mod small {
    use super::*;

    pub const PUBLIC_KEY_SIZE: usize = GroupG2_SIZE;
    pub const SIGNATURE_SIZE: usize = GroupG1_SIZE;

    pub type Generator = G2;
    pub type SignatureGroup = G1;

    public_key_impl!();

    aggregate_public_key_impl!();

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Signature(SignatureGroup);

    impl Signature {
        pub fn new(message: &[u8], sk: &PrivateKey) -> Self {
            Signature(&SignatureGroup::from_msg_hash(message) * sk)
        }

        pub fn new_with_rk_mitigation(
            message: &[u8],
            sk: &PrivateKey,
            pk_index: usize,
            pks: &[PublicKey],
        ) -> Self {
            // To combat the rogue key attack,
            // compute (t_1,…,t_n)←H1(pk_1,…,pk_n) ∈ R_n
            // output the aggregated public key
            // as described in section 3.1 from https://eprint.iacr.org/2018/483
            let mut bytes = Vec::new();
            for k in pks {
                bytes.extend_from_slice(k.to_bytes().as_slice());
            }
            bytes.extend_from_slice(pks[pk_index].to_bytes().as_slice());
            let a = FieldElement::from_msg_hash(bytes.as_slice());
            Signature(SignatureGroup::from_msg_hash(message) * sk * &a)
        }

        // Collects multiple signatures into a single signature
        // Verified by using `verify_multi`. This method does not
        // directly mitigate the rogue key attack. It is expected the caller
        // handles this using other techniques like proof of possession
        pub fn combine(&mut self, signatures: &[Signature]) {
            for sig in signatures {
                self.0 += &sig.0;
            }
        }

        // Verify a signature generated by `new`
        pub fn verify(&self, message: &[u8], pk: &PublicKey, g: &Generator) -> bool {
            let hash = SignatureGroup::from_msg_hash(message);
            GT::ate_2_pairing(&self.0, &-g, &hash, &pk.0).is_one()
        }

        // Caller should aggregate all signatures into `self` by using `combine`.
        // Messages must be distinct
        // `inputs` is a slice of message - public key tuples
        // Multisignature verification
        pub fn verify_multi(&self, inputs: &[(&[u8], &PublicKey)], g: &Generator) -> bool {
            let mut msg_check = ::std::collections::HashSet::new();
            let mut pairs = Vec::new();
            for (msg, pk) in inputs {
                let hash = SignatureGroup::from_msg_hash(&msg);
                if msg_check.contains(&hash) {
                    return false;
                }
                pairs.push((hash.clone(), pk.0.clone()));
                msg_check.insert(hash);
            }

            pairs.push((self.0.clone(), -g));
            let ate_pairs = pairs.iter().map(|(g1, g2)| (g1, g2)).collect();
            GT::ate_multi_pairing(ate_pairs).is_one()
        }

        pub fn batch_verify(inputs: &[(&[u8], &Signature, &PublicKey)], g: &Generator) -> bool {
            // To avoid rogue key attacks, you must use proof of possession or `AggregateSignature::batch_verify`
            // This function just avoids checking for distinct messages and
            // uses batch verification as described in the end of section 3.1 from https://eprint.iacr.org/2018/483
            let mut pairs = Vec::new();
            let mut sig = SignatureGroup::identity();
            for (msg, asg, apk) in inputs {
                let random_exponent = FieldElement::random();
                let hash = SignatureGroup::from_msg_hash(msg);
                sig += &asg.0 * &random_exponent;
                pairs.push((hash, &apk.0 * &random_exponent));
            }

            pairs.push((sig, -g));

            let ate_pairs = pairs.iter().map(|(g1, g2)| (g1, g2)).collect();
            GT::ate_multi_pairing(ate_pairs).is_one()
        }

        pub fn to_bytes(&self) -> Vec<u8> {
            self.0.to_bytes()
        }

        pub fn from_bytes(bytes: &[u8]) -> Result<Self, CryptoError> {
            Ok(Signature(SignatureGroup::from_bytes(bytes).map_err(
                |e| CryptoError::ParseError(format!("{:?}", e)),
            )?))
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct AggregatedSignature(SignatureGroup);

    impl AggregatedSignature {
        // `Signature` should be generated by calling `Signature::new_with_rk_mitigation`
        // to avoid rogue key attacks. If using proof of possession mitigation
        // then `Signature` can be generated by calling `Signature::new`
        pub fn new(signatures: &[Signature]) -> Self {
            AggregatedSignature(
                signatures
                    .iter()
                    .fold(SignatureGroup::identity(), |sig, s| sig + &s.0),
            )
        }

        // Verify with rogue key attack mitigation.
        pub fn verify(&self, message: &[u8], pk: &AggregatedPublicKey, g: &Generator) -> bool {
            let hash = SignatureGroup::from_msg_hash(message);
            GT::ate_2_pairing(&self.0, &-g, &hash, &pk.0).is_one()
        }

        // Verify without rogue key mitigation. Assumes caller has handled
        // rogue key mitigation some other way like proof of possession.
        // This practice is discouraged in favor of the other method
        // but there are use cases where proof of possession is better suited
        pub fn verify_no_rk(&self, message: &[u8], pks: &[PublicKey], g: &Generator) -> bool {
            let apk = pks.iter().fold(Generator::identity(), |a, p| a + &p.0);
            let hash = SignatureGroup::from_msg_hash(message);
            GT::ate_2_pairing(&self.0, &-g, &hash, &apk).is_one()
        }

        /// This should be used to verify quickly multiple BLS aggregated signatures by batching
        /// versus verifying them one by one as it reduces the number of computed pairings
        pub fn batch_verify(
            inputs: &[(
                &[u8], /* message */
                &AggregatedSignature,
                &AggregatedPublicKey,
            )],
            g: &Generator,
        ) -> bool {
            // To combat the rogue key attack and avoid checking for distinct messages
            // use batch verification as described in the end of section 3.1 from https://eprint.iacr.org/2018/483
            let mut pairs = Vec::new();
            let mut sig = SignatureGroup::identity();
            for (msg, asg, apk) in inputs {
                let random_exponent = FieldElement::random();
                let hash = SignatureGroup::from_msg_hash(msg);
                sig += &asg.0 * &random_exponent;
                pairs.push((hash, &apk.0 * &random_exponent));
            }

            pairs.push((sig, -g));

            let ate_pairs = pairs.iter().map(|(g1, g2)| (g1, g2)).collect();
            GT::ate_multi_pairing(ate_pairs).is_one()
        }
    }

    generate_impl!();

    bls_tests_impl!();
}

#[cfg(test)]
mod tests {
    use super::normal::{
        generate as normal_generate, Generator as NormalGenerator, Signature as NormalSignature,
    };
    use super::small::{
        generate as small_generate, Generator as SmallGenerator, Signature as SmallSignature,
    };
    use amcl_wrapper::{
        constants::{GroupG1_SIZE, MODBYTES},
        field_elem::FieldElement,
        group_elem::GroupElement,
        types_g2::GroupG2_SIZE,
    };

    #[test]
    fn size_check() {
        let msg = FieldElement::random();
        let g = NormalGenerator::generator();
        let (pk, sk) = normal_generate(&g);
        assert_eq!(sk.to_bytes().len(), MODBYTES);
        assert_eq!(pk.to_bytes().len(), GroupG1_SIZE);
        let sig = NormalSignature::new(msg.to_bytes().as_slice(), &sk);
        assert_eq!(sig.to_bytes().len(), GroupG2_SIZE);

        let g = SmallGenerator::generator();
        let (pk, sk) = small_generate(&g);
        assert_eq!(sk.to_bytes().len(), MODBYTES);
        assert_eq!(pk.to_bytes().len(), GroupG2_SIZE);
        let sig = SmallSignature::new(msg.to_bytes().as_slice(), &sk);
        assert_eq!(sig.to_bytes().len(), GroupG1_SIZE);
    }
}