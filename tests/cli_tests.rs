#[ignore = "too slow to be enabled by default"]
#[test]
fn cli_tests() {
    trycmd::TestCases::new()
        .case("tests/cmd/*.toml");
}