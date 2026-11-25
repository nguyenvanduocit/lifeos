//! The link's certificate: self-signed ECDSA P-256, valid under two weeks —
//! the only kind a browser accepts by its SHA-256 hash
//! (`serverCertificateHashes`) instead of a CA. A browser does not check the
//! host name of a hash-pinned certificate, so one works for loopback and for
//! a Tailscale name alike.
//!
//! A server runs longer than a certificate lives, so `rotate` puts a fresh one
//! on the QUIC endpoint every `ROTATE_EVERY`: new sessions get it, sessions
//! already open keep theirs, and a page that redials reads the new hash from
//! `/link.json`.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use web_transport_quinn::quinn;
use web_transport_quinn::quinn::rustls;
use web_transport_quinn::quinn::rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Browsers refuse a hash-pinned certificate valid for two weeks or more.
const CERTIFICATE_DAYS: i64 = 13;

/// Well inside `CERTIFICATE_DAYS`, so a certificate handed out is never near its end.
pub const ROTATE_EVERY: Duration = Duration::from_secs(6 * 24 * 3600);

pub struct Certificate {
    chain: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
    pub hash: Vec<u8>,
    pub not_after: time::OffsetDateTime,
}

/// What `/link.json` says about the certificate the endpoint presents now.
#[derive(Clone)]
pub struct Published {
    /// SHA-256, hex.
    pub hash: String,
    /// RFC 3339.
    pub expires: String,
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

    pub fn published(&self) -> Result<Published> {
        Ok(Published {
            hash: self.hash.iter().map(|b| format!("{b:02x}")).collect(),
            expires: self
                .not_after
                .format(&time::format_description::well_known::Rfc3339)?,
        })
    }

    /// The QUIC server config `web_transport_quinn::ServerBuilder` would make:
    /// TLS 1.3, ALPN `h3`, default transport.
    pub fn quic_config(&self) -> Result<quinn::ServerConfig> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut tls = rustls::ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_no_client_auth()
            .with_single_cert(self.chain.clone(), self.key.clone_key())?;
        tls.alpn_protocols = vec![web_transport_quinn::ALPN.as_bytes().to_vec()];
        let crypto = quinn::crypto::rustls::QuicServerConfig::try_from(tls)?;
        Ok(quinn::ServerConfig::with_crypto(Arc::new(crypto)))
    }
}

/// Puts a fresh certificate on `endpoint` every `ROTATE_EVERY`, forever.
pub async fn rotate(endpoint: quinn::Endpoint, published: Arc<RwLock<Published>>) {
    loop {
        tokio::time::sleep(ROTATE_EVERY).await;
        match rotate_once(&endpoint, &published) {
            Ok(expires) => tracing::info!("link: new certificate, valid until {expires}"),
            // Keep serving the current certificate; try again next period.
            Err(err) => tracing::error!("link: cannot make a new certificate: {err:#}"),
        }
    }
}

/// Swaps in a fresh certificate now; returns when it expires.
pub fn rotate_once(endpoint: &quinn::Endpoint, published: &RwLock<Published>) -> Result<String> {
    let cert = Certificate::new(time::OffsetDateTime::now_utc())?;
    let info = cert.published()?;
    endpoint.set_server_config(Some(cert.quic_config()?));
    let expires = info.expires.clone();
    *published.write().expect("certificate lock") = info;
    Ok(expires)
}
