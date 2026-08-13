#[test]
fn package_identity_is_stable() {
    assert_eq!(env!("CARGO_PKG_NAME"), "tabbeacon");
    assert_eq!(tabbeacon::PRODUCT_NAME, "TabBeacon");
    assert_eq!(tabbeacon::BOOTSTRAP_SCHEMA_VERSION, 1);
}
