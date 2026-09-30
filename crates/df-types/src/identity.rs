/// Rejection of a supplied canonical identity. Identities are not credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    InvalidLength { actual: usize },
    Zero,
}

// The five distinct contracts share canonical byte validation, not interchangeability.
macro_rules! identity {
    ($name:ident) => {
        /// An opaque, caller-supplied nonzero 128-bit identity in canonical byte order.
        /// This type neither generates values nor establishes access or uniqueness.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 16]);

        impl $name {
            /// Accepts exactly 16 bytes with at least one nonzero byte.
            pub fn from_bytes(bytes: &[u8]) -> Result<Self, IdentityError> {
                let value: [u8; 16] =
                    bytes.try_into().map_err(|_| IdentityError::InvalidLength {
                        actual: bytes.len(),
                    })?;
                if value == [0; 16] {
                    return Err(IdentityError::Zero);
                }
                Ok(Self(value))
            }

            /// Returns the supplied canonical bytes without reordering.
            pub fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }
        }
    };
}
identity!(SessionId);
identity!(MemberId);
identity!(ClientBindingId);
identity!(RunId);
identity!(OperationId);
