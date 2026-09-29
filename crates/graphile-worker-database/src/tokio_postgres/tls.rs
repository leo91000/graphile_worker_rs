use crate::DbError;

#[cfg(feature = "tls-native-tls")]
pub(super) type Connector = postgres_native_tls::MakeTlsConnector;
#[cfg(all(feature = "tls-rustls", not(feature = "tls-native-tls")))]
pub(super) type Connector = tokio_postgres_rustls::MakeRustlsConnect;
#[cfg(not(any(feature = "tls-native-tls", feature = "tls-rustls")))]
pub(super) type Connector = ::tokio_postgres::NoTls;

#[cfg(feature = "tls-native-tls")]
pub(super) fn connector() -> Result<Connector, DbError> {
    let mut builder = native_tls::TlsConnector::builder();
    for cert in extra_root_certs()? {
        let cert = native_tls::Certificate::from_der(&cert)
            .map_err(|error| DbError::new(error.to_string()))?;
        builder.add_root_certificate(cert);
    }
    let connector = builder
        .build()
        .map_err(|error| DbError::new(error.to_string()))?;
    Ok(Connector::new(connector))
}

#[cfg(all(feature = "tls-rustls", not(feature = "tls-native-tls")))]
pub(super) fn connector() -> Result<Connector, DbError> {
    use std::sync::Arc;

    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for cert in extra_root_certs()? {
        roots
            .add(cert.into())
            .map_err(|error| DbError::new(error.to_string()))?;
    }

    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| DbError::new(error.to_string()))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(Connector::new(config))
}

#[cfg(not(any(feature = "tls-native-tls", feature = "tls-rustls")))]
pub(super) fn connector() -> Result<Connector, DbError> {
    Ok(::tokio_postgres::NoTls)
}

#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
fn extra_root_certs() -> Result<Vec<Vec<u8>>, DbError> {
    use std::io::Cursor;

    let Some(path) = std::env::var_os("PGSSLROOTCERT") else {
        return Ok(Vec::new());
    };
    let pem = std::fs::read(path).map_err(|error| DbError::new(error.to_string()))?;
    let certs = rustls_pemfile::certs(&mut Cursor::new(pem))
        .map(|cert| cert.map(|cert| cert.as_ref().to_vec()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| DbError::new(error.to_string()))?;
    if certs.is_empty() {
        return Err(DbError::new("PGSSLROOTCERT contains no PEM certificates"));
    }
    Ok(certs)
}
