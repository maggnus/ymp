use semver::{Version, VersionReq};

fn matches(requirement: &str, version: &str) -> bool {
    VersionReq::parse(requirement)
        .unwrap()
        .matches(&Version::parse(version).unwrap())
}

#[test]
fn protected_partial_less_than_excludes_boundary_prereleases() {
    assert!(matches("<1.2", "1.1.999"));
    assert!(!matches("<1.2", "1.2.0"));
    assert!(!matches(">1.2.0-alpha, <1.2", "1.2.0-beta"));
}

#[test]
fn protected_partial_less_equal_has_the_same_boundary() {
    assert!(matches("<=1.2", "1.1.999"));
    assert!(matches("<=1.2", "1.2.0"));
    assert!(matches("<=1.2", "1.2.1"));
    assert!(!matches(">1.2.0-alpha, <=1.2", "1.2.0-beta"));
}
