#[test]
fn independent_enum_retirement_name_number_and_inclusive_end_refuse() {
    let mut d=FileDescriptorSet::decode(df_protocol::FILE_DESCRIPTOR_SET).unwrap();
    let e=d.file.iter_mut().flat_map(|f|f.enum_type.iter_mut()).find(|e|e.name.as_deref()==Some("CapabilityKind")).unwrap();
    let retired=e.value.pop().unwrap();let n=retired.number.unwrap();let name=retired.name.clone().unwrap();
    e.reserved_range.push(prost_types::enum_descriptor_proto::EnumReservedRange{start:Some(n),end:Some(n)}); e.reserved_name.push(name.clone());
    let baseline=descriptor_records(&d.encode_to_vec()).unwrap();
    for (number,label) in [(n,"FUTURE".to_owned()),(n+1,name)] {let mut bad=d.clone();let e=bad.file.iter_mut().flat_map(|f|f.enum_type.iter_mut()).find(|e|e.name.as_deref()==Some("CapabilityKind")).unwrap();e.value.push(prost_types::EnumValueDescriptorProto{name:Some(label),number:Some(number),..Default::default()});assert_eq!(check_ledger(&bad.encode_to_vec(),&baseline),Err(LedgerError::ReservedAllocation));}
}
#[test]
fn independent_actual_descriptor_new_service_and_oneof_relocation_detected() {
    let mut d=FileDescriptorSet::decode(df_protocol::FILE_DESCRIPTOR_SET).unwrap();
    let f=d.file.iter_mut().find(|f|f.package.as_deref()==Some("dungeonflux.public.v1")).unwrap();assert!(f.service.is_empty());f.service.push(prost_types::ServiceDescriptorProto{name:Some("Unapproved".into()),..Default::default()});assert_eq!(check_ledger(&d.encode_to_vec(),&ledger()),Err(LedgerError::ContractChanged));
    let mut d=FileDescriptorSet::decode(df_protocol::FILE_DESCRIPTOR_SET).unwrap();common_revision(&mut d).field[1].oneof_index=None;assert_eq!(check_ledger(&d.encode_to_vec(),&ledger()),Err(LedgerError::ContractChanged));
}
