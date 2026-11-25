//! The link's certificate: made at every start, self-signed ECDSA P-256 for
//! the loopback names, valid under two weeks — the only kind a browser accepts
//! by its SHA-256 hash (`serverCertificateHashes`) instead of a CA.

use anyhow::{Context, Result};
use web_transport_quinn::quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Browsers refuse a hash-pinned certificate valid for two weeks or more.
const CERTIFICATE_DAYS: i64 = 13;

pub struct Certificate {
    pub chain: Vec<CertificateDer<'static>>,
    pub key: PrivateKeyDer<'static>,
    pub hash: Vec<u8>,
    pub not_after: time::OffsetDateTime,
}

impl Certificate {
    pub fn new(now: time::OffsetDateTime) -> Result<Certificate> {
        let names = vec!["localhost".to_owned(), "127.0.0.1".to_owned(), "::1".to_owned()];
        let mut params = rcgen::CertificateParams::new(names).context("the link's certificate")?;
        params.not_before = now - time::Duration::hours(1);
        let not_after = now + time::Duration::days(CERTIFICATE_DAYS);
        params.not_after = not_after;
        let key = rcgen::KeyPair::generate().context("the link's certificate key")?;
        let cert = params.self_signed(&key).context("the link's certificate")?;
        let der = cert.der().to_vec();
        let hash = ring::digest::digest(&ring::digest::SHA256, &der).as_ref().to_vec();
        Ok(Certificate {
            chain: vec![CertificateDer::from(der)],
            key: PrivateKeyDer::Pkcs8(key.serialize_der().into()),
            hash,
            not_after,
        })
    }
}
