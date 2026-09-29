use std::path::Path;

use graphile_worker_postgres_tls::connector;

#[test]
fn default_root_settings_construct_a_connector() {
    for root in [None, Some(Path::new("")), Some(Path::new("system"))] {
        assert!(connector(root).is_ok(), "default roots: {root:?}");
    }
}

#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
#[test]
fn private_ca_can_be_added_to_default_roots() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ca.pem");
    assert!(connector(Some(&path)).is_ok());
}

#[cfg(any(feature = "tls-native-tls", feature = "tls-rustls"))]
#[test]
fn invalid_root_files_fail_instead_of_disabling_verification() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for name in [
        "missing.pem",
        "no-certificates.pem",
        "malformed.pem",
        "invalid-der.pem",
    ] {
        assert!(connector(Some(&fixtures.join(name))).is_err(), "{name}");
    }
}

#[cfg(not(any(feature = "tls-native-tls", feature = "tls-rustls")))]
#[test]
fn plaintext_does_not_load_root_certificates() {
    assert!(connector(Some(Path::new("missing.pem"))).is_ok());
}
