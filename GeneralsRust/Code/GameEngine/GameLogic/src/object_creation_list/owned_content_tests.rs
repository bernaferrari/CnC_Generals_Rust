//! Pure receiving content parsing: no global store, factory, frame or RNG access.
use super::*;
fn generic(list: &ObjectCreationList, index: usize) -> &GenericObjectCreationNugget {
    ObjectCreationNugget::as_any(list.nuggets()[index].as_ref())
        .downcast_ref()
        .unwrap()
}
#[test]
fn mixed_map_ini_retains_actual_order_defaults_and_nested_delivery_decal() {
    let text = "GameData\n MaxCameraHeight=600\n End\nObjectCreationList OCL_Owned\n CreateDebris\n ModelNames=First Second\n Count=6\n Mass=5\n Disposition=SEND_IT_FLYING INHERIT_VELOCITY\n DispositionIntensity=1.5\n End\n DeliverPayload\n Transport=TransportA\n DeliveryDecal\n Radius=12\n End\n End\n CreateObject\n ObjectNames=InfantryA\n End\n End\nWeapon Other\n Damage=99\n End\n";
    let parsed = parse_object_creation_lists_from_ini(text).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].0, "OCL_Owned");
    let list = &parsed[0].1;
    assert_eq!(list.get_nugget_count(), 3);
    let debris = generic(list, 0);
    assert_eq!(debris.names, vec!["First", "Second"]);
    assert!(!debris.name_are_objects);
    assert_eq!(debris.debris_to_generate, 6);
    assert_eq!(debris.mass, 5.);
    assert_eq!(debris.disposition_intensity, 1.5);
    assert!(ObjectCreationNugget::as_any(list.nuggets()[1].as_ref()).is::<DeliverPayloadNugget>());
    let object = generic(list, 2);
    assert_eq!(object.names, vec!["InfantryA"]);
    assert!(object.name_are_objects);
    let direct = parse_object_creation_lists_from_str(
        "ObjectCreationList Direct\n CreateObject\n ObjectNames=InfantryA\n End\n End\n",
    )
    .unwrap();
    assert_eq!(
        object.debris_to_generate,
        generic(&direct[0].1, 0).debris_to_generate
    );
}
#[test]
fn pure_parser_keeps_duplicate_named_blocks_in_admission_order_and_errors_on_truncation() {
    let parsed=parse_object_creation_lists_from_ini("ObjectCreationList Same\n CreateObject\n ObjectNames=DefaultA\n End\n End\nObjectCreationList Same\n CreateObject\n ObjectNames=RetailB\n End\n End\n").unwrap();
    assert_eq!(
        parsed
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        vec!["Same", "Same"]
    );
    assert_eq!(generic(&parsed[0].1, 0).names, vec!["DefaultA"]);
    assert_eq!(generic(&parsed[1].1, 0).names, vec!["RetailB"]);
    let error = parse_object_creation_lists_from_ini(
        "GameData\n End\nObjectCreationList Broken\n CreateDebris\n ModelNames=ActualModel\n",
    )
    .unwrap_err();
    assert!(error.contains("Unterminated OCL nugget"), "{error}");
}
