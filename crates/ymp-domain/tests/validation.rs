use std::collections::BTreeSet;
use ymp_domain::{
    Digest, Id, PolicyRef, Proposal, Ref,
    journal::{PolicySelection, decode, encode},
};

#[test]
fn ids_and_digests_reject_invalid_wire_values() {
    for value in ["", "with spaces", "../escape", "a/b", "é", "-prefix", "\n"] {
        assert!(Id::<()>::new(value).is_err(), "{value:?}");
        assert!(decode::<Id>(&serde_json::to_vec(value).unwrap()).is_err());
    }
    assert!(Id::<()>::new("x".repeat(129)).is_err());
    assert_eq!(
        decode::<Id>(br#""a-Z_1.2:3""#).unwrap().as_str(),
        "a-Z_1.2:3"
    );
    let abc = Digest::of(b"abc");
    assert_eq!(
        abc.as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(decode::<Digest>(&encode(&abc).unwrap()).unwrap(), abc);
    for value in ["0".repeat(63), "F".repeat(64), "z".repeat(64)] {
        assert!(decode::<Digest>(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    struct Marker;
    let id: Id<Marker> = Id::new("typed").unwrap();
    assert_eq!(id.clone(), id);
    assert_eq!(BTreeSet::from([id]).len(), 1);
}

#[test]
fn canonical_encoding_is_recursive_and_strict_decoding_does_not_erase_duplicates() {
    let first: serde_json::Value = decode(br#"{ "z": [{"b":2,"a":1}], "a":true }"#).unwrap();
    let second: serde_json::Value = decode(br#"{"a":true,"z":[{"a":1,"b":2}]}"#).unwrap();
    assert_eq!(
        encode(&first).unwrap(),
        br#"{"a":true,"z":[{"a":1,"b":2}]}"#
    );
    assert_eq!(
        Digest::of_value(&first).unwrap(),
        Digest::of_value(&second).unwrap()
    );
    for bytes in [
        br#"{"a":1,"a":2}"#.as_slice(),
        br#"{"x":[{"a":1,"a":2}]}"#,
        br#"{"a":1} trailing"#,
        br#"[1e999]"#,
    ] {
        assert!(decode::<serde_json::Value>(bytes).is_err());
    }
}

#[test]
fn parameters_are_recoverable_and_hash_bound() {
    let mut selection = PolicySelection::new(
        "MethodRouter",
        "FixedMethod",
        "1",
        serde_json::json!({"kind":"SoloWithVerifier","ladder":["Retry"]}),
    )
    .unwrap();
    assert_eq!(
        decode::<PolicySelection>(&encode(&selection).unwrap()).unwrap(),
        selection
    );
    selection.parameters["kind"] = "Solo".into();
    assert_eq!(selection.validate().unwrap_err().code, "policy_parameters");
    assert!(
        PolicySelection::new("MethodRouter", "FixedMethod", "1", serde_json::Value::Null).is_err()
    );
    let policy = PolicyRef {
        port: String::new(),
        implementation: "FixedMethod".into(),
        version: "1".into(),
        params: Digest::of(b"{}"),
    };
    assert!(policy.validate().is_err());
    let proposal = Proposal {
        value: (),
        rationale: "reason".into(),
        basis: Vec::<Ref>::new(),
        policy,
    };
    assert!(proposal.validate().is_err());
}

#[test]
fn finite_reals_preserve_their_exact_bits_and_digest_through_json() {
    use ymp_domain::task::Real;
    let mut bits = 0x0123_4567_89ab_cdef_u64;
    for _ in 0..2048 {
        bits = bits.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let float = f64::from_bits(bits);
        if !float.is_finite() {
            continue;
        }
        let real = Real::new(float).unwrap();
        let bytes = encode(&real).unwrap();
        let restored: Real = decode(&bytes).unwrap();
        assert_eq!(
            restored.get().to_bits(),
            bits,
            "{}",
            String::from_utf8_lossy(&bytes)
        );
        assert_eq!(
            Digest::of_value(&restored).unwrap(),
            Digest::of_value(&real).unwrap()
        );
    }
}
