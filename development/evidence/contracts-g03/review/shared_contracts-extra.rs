#[test]
fn independent_capability_signed_extremes_require_explicit_opt_out() {
    for kind in [i32::MIN,-1,2,i32::MAX] {
        for required in [false,true] {
            let mut value=revision_fixture(u64::MAX,0);
            value.capabilities=vec![fixture::Capability{kind:Some(kind),required:Some(required)}];
            let decoded=fixture::CompatibilityFixture::decode(value.encode_to_vec().as_slice()).unwrap();
            if required {assert_eq!(consume(decoded),Err(ContractError::UnknownRequiredCapability(kind)));}
            else {assert!(consume(decoded).is_ok());}
        }
    }
}
#[test]
fn independent_all_canonical_identity_bits_and_revision_boundaries() {
    for i in 0..128 {let mut b=[0u8;16]; b[i/8]=1 << (i%8); assert_eq!(SessionId::from_bytes(&b).unwrap().as_bytes(),&b); assert_eq!(MemberId::from_bytes(&b).unwrap().as_bytes(),&b); assert_eq!(ClientBindingId::from_bytes(&b).unwrap().as_bytes(),&b); assert_eq!(RunId::from_bytes(&b).unwrap().as_bytes(),&b); assert_eq!(OperationId::from_bytes(&b).unwrap().as_bytes(),&b);}
    for epoch in [1,2,u64::MAX-1,u64::MAX] {for seq in [0,1,u64::MAX-1,u64::MAX] {let r=SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(),seq); let d=read_revision(wire::SessionRevision::decode(write_revision(r).encode_to_vec().as_slice()).unwrap()).unwrap();assert_eq!(r,d); if seq<u64::MAX {assert_eq!(d.next_sequence().unwrap().sequence(),seq+1);} else {assert_eq!(d.next_sequence(),Err(RevisionError::SequenceOverflow));}}}
    assert!(SessionRevision::new(RecoveryEpoch::new(u64::MAX).unwrap(),0)>SessionRevision::new(RecoveryEpoch::new(u64::MAX-1).unwrap(),u64::MAX));
}
#[test]
fn independent_label_byte_policy_exhaustive_ascii_and_all_wire_field_bounds() {
    for b in 0u8..=127 {let s=String::from_utf8(vec![b]).unwrap();assert_eq!(RevisionLabel::new(Some(&s)).is_ok(),b.is_ascii_alphanumeric()||b"._-:/".contains(&b));}
    for i in 0..5 {for length in [128,129] {let label="Z".repeat(length);let mut labels=[Some("ok");5];labels[i]=Some(&label);let value=wire::BuildIdentity{source_revision:labels[0].map(str::to_owned),native_revision:labels[1].map(str::to_owned),wasm_revision:labels[2].map(str::to_owned),configuration_revision:labels[3].map(str::to_owned),content_revision:labels[4].map(str::to_owned)};assert_eq!(read_build(wire::BuildIdentity::decode(value.encode_to_vec().as_slice()).unwrap()).is_ok(),length==128);}}
}
