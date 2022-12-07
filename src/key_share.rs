//! Key share

use generic_ec::{Curve, Point, SecretScalar};
use libpaillier::unknown_order::BigNumber;
use thiserror::Error;

use crate::security_level::SecurityLevel;

/// Core key share
///
/// Core key share is obtained as an output of [key generation protocol](crate::keygen()).
/// It can not be used in signing protocol as it lacks of required auxiliary information.
/// You need to carry out [key refresh protocol](crate::refresh) to obtain "completed"
/// [KeyShare].
#[derive(Clone)]
pub struct IncompleteKeyShare<E: Curve, L: SecurityLevel> {
    /// Index of local party in key generation protocol
    pub i: u16,
    /// Public key corresponding to shared secret key
    pub shared_public_key: Point<E>,
    /// Randomness derived at key generation
    pub rid: L::Rid,
    /// Public shares of all parties sharing the key
    ///
    /// `public_shares[i]` corresponds to public share of $\ith$ party
    pub public_shares: Vec<Point<E>>,
    /// Secret share $x_i$
    pub x: SecretScalar<E>,
}

/// Key share
///
/// Key share is obtained as output of [key refresh protocol](crate::refresh).
/// It contains a [core share](IncompleteKeyShare) and auxiliary data required to
/// carry out signing.
#[derive(Clone)]
pub struct KeyShare<E: Curve, L: SecurityLevel> {
    /// Core key share
    pub core: IncompleteKeyShare<E, L>,
    /// Secret prime $p$
    pub p: BigNumber,
    /// Secret prime $q$
    pub q: BigNumber,
    /// El-Gamal private key
    pub y: SecretScalar<E>,
    /// Public auxiliary data of all parties sharing the key
    ///
    /// `parties[i]` corresponds to public auxiliary data of $\ith$ party
    pub parties: Vec<PartyAux<E>>,
}

/// Party public auxiliary data
#[derive(Debug, Clone)]
pub struct PartyAux<E: Curve> {
    /// $N_i = p_i \cdot q_i$
    pub N: BigNumber,
    /// Ring-Perdesten parameter $s_i$
    pub s: BigNumber,
    /// Ring-Perdesten parameter $t_i$
    pub t: BigNumber,
    /// El-Gamal public key
    pub Y: Point<E>,
}

impl<E: Curve, L: SecurityLevel> IncompleteKeyShare<E, L> {
    /// Validates a share
    ///
    /// Performs consistency checks against a key share, returns `Ok(())` if share looks OK.
    pub fn validate(&self) -> Result<(), InvalidKeyShare> {
        let n: u16 = self
            .public_shares
            .len()
            .try_into()
            .or(Err(ErrorReason::PartiesNumberOverflowU16))?;
        if self.i >= n {
            return Err(ErrorReason::PartyIndexOutOfBounds.into());
        }

        let party_public_share = self.public_shares[usize::from(self.i)];
        if party_public_share != Point::generator() * &self.x {
            return Err(ErrorReason::PartySecretShareDoesntMatchPublicShare.into());
        }

        if self.shared_public_key != self.public_shares.iter().sum::<Point<E>>() {
            return Err(ErrorReason::SharesDontMatchPublicKey.into());
        }
        Ok(())
    }
}

impl<E: Curve, L: SecurityLevel> KeyShare<E, L> {
    /// Validates a share
    ///
    /// Performs consistency checks against a key share, returns `Ok(())` if share looks OK.
    pub fn validate(&self) -> Result<(), InvalidKeyShare> {
        self.core.validate()?;

        if self.core.public_shares.len() != self.parties.len() {
            return Err(ErrorReason::AuxWrongLength.into());
        }

        let el_gamal_public = self.parties[usize::from(self.core.i)].Y;
        if el_gamal_public != Point::generator() * &self.y {
            return Err(ErrorReason::ElGamalKey.into());
        }

        let N_i = &self.parties[usize::from(self.core.i)].N;
        if *N_i != &self.p * &self.q {
            return Err(ErrorReason::PrimesMul.into());
        }

        if self
            .parties
            .iter()
            .any(|p| p.s.gcd(&p.N) != BigNumber::one() || p.t.gcd(&p.N) != BigNumber::one())
        {
            return Err(ErrorReason::StGcdN.into());
        }

        Ok(())
    }
}

/// Error indicating that key share is not valid
#[derive(Debug, Error)]
#[error(transparent)]
pub struct InvalidKeyShare(#[from] ErrorReason);

#[derive(Debug, Error)]
enum ErrorReason {
    #[error("number of parties `n` overflow u16::MAX (implying `n = public_shares.len()`)")]
    PartiesNumberOverflowU16,
    #[error("party index `i` out of bounds: i >= n")]
    PartyIndexOutOfBounds,
    #[error("party secret share doesn't match its public share: public_shares[i] != G x")]
    PartySecretShareDoesntMatchPublicShare,
    #[error("list of public shares doesn't match shared public key: public_shares.sum() != shared_public_key")]
    SharesDontMatchPublicKey,
    #[error("size of parties auxiliary data list doesn't match `n`: n != parties.len()")]
    AuxWrongLength,
    #[error("party El-Gamal secret key doesn't match public key: y_i G != Y_i")]
    ElGamalKey,
    #[error("N_i != p q")]
    PrimesMul,
    #[error("gcd(s_j, N_j) != 1 or gcd(t_j, N_j) != 1")]
    StGcdN,
}