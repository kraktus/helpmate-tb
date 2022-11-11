#[ignore = "too slow to be enabled by default"]
#[test]
fn cli_tests() {
    #[cfg(feature = "trycmd-overwrite")]
    std::env::set_var("TRYCMD", "overwrite");
    trycmd::TestCases::new().case("tests/cmd/*.toml");
}
