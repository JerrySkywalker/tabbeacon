fn main() {
    println!(
        "{} {} (bootstrap; runtime integration not implemented)",
        tabbeacon::PRODUCT_NAME,
        env!("CARGO_PKG_VERSION")
    );
}
