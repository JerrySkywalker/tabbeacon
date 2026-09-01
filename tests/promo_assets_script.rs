#[test]
fn promo_asset_overwrite_guard_groups_each_test_path_call() {
    let script = include_str!("../scripts/generate-promo-assets.ps1");

    assert!(script.contains(
        "if ((Test-Path -LiteralPath $gif -PathType Leaf) -or (Test-Path -LiteralPath $poster -PathType Leaf))"
    ));
    assert!(!script.contains(
        "if (Test-Path -LiteralPath $gif -PathType Leaf -or Test-Path -LiteralPath $poster -PathType Leaf)"
    ));
}
