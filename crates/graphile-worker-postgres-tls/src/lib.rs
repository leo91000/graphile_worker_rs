//! PostgreSQL TLS selection shared by pooled queries and notification listeners.
//!
//! Kept in an optional crate so selecting a database driver and selecting a TLS
//! backend are independent: SQLx-only and plaintext builds need no connectors.

use std::path::Path;

/// Errors while constructing a connector or loading additional trust roots.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A root certificate file could not be read or parsed as PEM.
    #[error("could not read PGSSLROOTCERT: {0}")]
    ReadRoots(#[from] std::io::Error),
    /// The supplied file contained no certificates.
    #[error("PGSSLROOTCERT contains no PEM certificates")]
    EmptyRoots,
    /// Native TLS could not load a certificate or construct a connector.
    #[cfg(feature = "tls-native-tls")]
    #[error(transparent)]
    NativeTls(#[from] native_tls::Error),
    /// Rustls could not load a certificate or construct a connector.
    #[cfg(all(feature = "tls-rustls", not(feature = "tls-native-tls")))]
    #[error(transparent)]
    Rustls(#[from] rustls::Error),
}

/// Result of configuring PostgreSQL TLS.
pub type Result<T> = core::result::Result<T, Error>;

/// Selected TLS connector. Native TLS takes precedence when both are enabled.
#[cfg(feature = "tls-native-tls")]
pub type Connector = postgres_native_tls::MakeTlsConnector;
/// Selected TLS connector using Rustls and public certificate roots.
#[cfg(all(feature = "tls-rustls", not(feature = "tls-native-tls")))]
pub type Connector = tokio_postgres_rustls::MakeRustlsConnect;
/// Plaintext connector when neither TLS feature is enabled.
#[cfg(not(any(feature = "tls-native-tls", feature = "tls-rustls")))]
pub type Connector = tokio_postgres::NoTls;

/// Builds the selected connector, optionally adding roots from a PEM file.
/// An absent, empty, or `system` path retains the selected backend's defaults:
/// platform roots for native TLS, public WebPKI roots for Rustls.
#[cfg(feature = "tls-native-tls")]
pub fn connector(root_cert: Option<&Path>) -> Result<Connector> {
    let mut builder = native_tls::TlsConnector::builder();
    for cert in extra_root_certs(root_cert)? {
        builder.add_root_certificate(native_tls::Certificate::from_der(&cert)?);
    }
    Ok(Connector::new(builder.build()?))
}

/// Builds the selected connector, optionally adding roots from a PEM file.
/// An absent, empty, or `system` path retains the public WebPKI roots.
#[cfg(all(feature = "tls-rustls", not(feature = "tls-native-tls")))]
pub fn connector(root_cert: Option<&Path>) -> Result<Connector> {
    use std::sync::Arc;

    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for cert in extra_root_certs(root_cert)? {
        roots.add(cert.into())?;
    }

    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Connector::new(config))
}

/// Returns a plaintext connector. Root certificate settings are unused.
#[cfg(not(any(feature = "tls-native-tls", feature = "tls-rustls")))]
pub fn connector(_root_cert: Option<&Path>) -> Result<Connector> {
    Ok(tokio_postgres::NoTls)
}

#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
fn extra_root_certs(root_cert: Option<&Path>) -> Result<Vec<Vec<u8>>> {
    use std::io::Cursor;

    let Some(path) = root_cert else {
        return Ok(Vec::new());
    };
    if path.as_os_str().is_empty() || path == Path::new("system") {
        return Ok(Vec::new());
    }
    let pem = std::fs::read(path)?;
    let certs = rustls_pemfile::certs(&mut Cursor::new(pem))
        .map(|cert| cert.map(|cert| cert.as_ref().to_vec()))
        .collect::<std::io::Result<Vec<_>>>()?;
    if certs.is_empty() {
        return Err(Error::EmptyRoots);
    }
    Ok(certs)
}
