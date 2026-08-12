use serde_bytes::ByteBuf;
use serde_json::from_str;

#[test]
fn protected_lone_surrogates_remain_available_to_byte_buffers() {
    assert_eq!(
        from_str::<ByteBuf>(r#""\ud83c""#).unwrap(),
        ByteBuf::from(vec![0xed, 0xa0, 0xbc])
    );
    assert_eq!(
        from_str::<ByteBuf>(r#""\udc01""#).unwrap(),
        ByteBuf::from(vec![0xed, 0xb0, 0x81])
    );
}

#[test]
fn protected_escape_after_lone_surrogate_is_not_skipped() {
    assert_eq!(
        from_str::<ByteBuf>(r#""\ud83c\n""#).unwrap(),
        ByteBuf::from(vec![0xed, 0xa0, 0xbc, b'\n'])
    );
    assert!(from_str::<ByteBuf>(r#""\ud83c\!""#).is_err());
    assert!(from_str::<ByteBuf>(r#""\ud83c\u""#).is_err());
    assert!(from_str::<ByteBuf>(r#""\ud83c\ud83c""#).is_err());
}
