mod common;

use common::*;
use tsv_core::file_format::{LoadError, from_json, to_json};
use tsv_core::model::{DeviceKind, Document, PortKind, Rgb};
use tsv_core::validate::ValidationError;

fn sample() -> Document {
    let mut dmm = with_ports(
        device("DMM", 10, 2),
        vec![port("CH1", 0, 0), port("CH2", 1, 4)],
    );
    dmm.color = Rgb {
        r: 0x12,
        g: 0xab,
        b: 0xff,
    };
    let mut d = doc(vec![
        rack("Rack1", 42, vec![dmm, device("PSU", 1, 1)]),
        rack("Rack2", 24, vec![]),
    ]);
    d.name = tsv_core::name::DocumentName::parse("Lab 3 Station").unwrap();
    d
}

#[test]
fn round_trip_preserves_the_document() {
    let d = sample();
    assert_eq!(from_json(&to_json(&d), &limits()), Ok(d));
}

#[test]
fn output_is_pretty_and_versioned() {
    let json = to_json(&sample());
    assert!(json.starts_with("{\n  \"format_version\": 1,"), "{json}");
    assert!(json.contains("\"color\": \"#12abff\""), "{json}");
}

#[test]
fn missing_kinds_default() {
    let json = r##"{
      "format_version": 1,
      "name": "Station1",
      "racks": [{
        "id": "00000000-0000-0000-0000-000000000001",
        "name": "Rack1",
        "height_u": 42,
        "devices": [{
          "id": "00000000-0000-0000-0000-000000000002",
          "name": "DMM",
          "bottom_u": 1,
          "height_u": 1,
          "color": "#a0a0a0",
          "ports": [{
            "id": "00000000-0000-0000-0000-000000000003",
            "name": "CH1",
            "row": 0,
            "col": 0
          }]
        }]
      }]
    }"##;
    let d = from_json(json, &limits()).unwrap();
    let dev = &d.racks[0].devices[0];
    assert_eq!(dev.kind, DeviceKind::AdHoc);
    assert_eq!(dev.ports[0].kind, PortKind::Unspecified);
}

#[test]
fn newer_versions_are_rejected() {
    let json = to_json(&sample()).replacen("\"format_version\": 1", "\"format_version\": 2", 1);
    let err = from_json(&json, &limits()).unwrap_err();
    assert_eq!(err, LoadError::TooNew { found: 2 });
    assert_eq!(
        err.to_string(),
        "This file was made by a newer version of the app."
    );
}

#[test]
fn broken_files_are_rejected() {
    assert!(matches!(
        from_json("{", &limits()),
        Err(LoadError::NotJson(_))
    ));
    assert_eq!(from_json("{}", &limits()), Err(LoadError::MissingVersion));
    assert!(matches!(
        from_json(r#"{"format_version": 1, "name": "X"}"#, &limits()),
        Err(LoadError::Malformed(_))
    ));
    assert!(matches!(
        from_json(
            r#"{"format_version": 0, "name": "X", "racks": []}"#,
            &limits()
        ),
        Err(LoadError::Malformed(_))
    ));
}

#[test]
fn names_are_not_silently_corrected() {
    let json = to_json(&sample()).replace("\"name\": \"DMM\"", "\"name\": \"DM M\"");
    assert!(matches!(
        from_json(&json, &limits()),
        Err(LoadError::InvalidName { .. })
    ));
    let json = to_json(&sample()).replace(
        "\"name\": \"Lab 3 Station\"",
        "\"name\": \" Lab 3 Station\"",
    );
    assert!(matches!(
        from_json(&json, &limits()),
        Err(LoadError::InvalidName { .. })
    ));
}

#[test]
fn bad_colours_are_rejected() {
    let json = to_json(&sample()).replace("#12abff", "grey");
    assert_eq!(
        from_json(&json, &limits()),
        Err(LoadError::InvalidColor("grey".into()))
    );
}

#[test]
fn invariant_violations_are_reported() {
    let d = doc(vec![rack(
        "Rack1",
        42,
        vec![device("A", 1, 2), device("B", 2, 1)],
    )]);
    let err = from_json(&to_json(&d), &limits()).unwrap_err();
    assert!(matches!(
        err,
        LoadError::Invalid(ValidationError::DevicesOverlap { .. })
    ));
    assert_eq!(
        err.to_string(),
        "Rack 'Rack1': devices 'A' and 'B' overlap at U2"
    );
}
