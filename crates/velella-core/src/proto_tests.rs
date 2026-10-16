use crate::proto::*;

const MAX: usize = 1 << 20;

fn sample() -> Vec<u8> {
    let mut inner = Message::new();
    inner.push_string(1, "hi");
    let mut m = Message::new();
    m.push_varint(1, 300);
    m.push_string(2, "héllo");
    m.push_message(3, &inner);
    m.fields.push(Field {
        number: 4,
        value: Value::Fixed32(7),
    });
    m.push_fixed64_unpacked(5, &[1, u64::MAX]);
    m.encode()
}

#[test]
fn truncated_at_every_offset() {
    let bytes = sample();
    assert!(Message::decode(&bytes, MAX).is_ok());
    // Every strict prefix either errors or ends exactly on a field boundary; never panics.
    let mut errs = 0;
    for n in 0..bytes.len() {
        if Message::decode(&bytes[..n], MAX).is_err() {
            errs += 1;
        }
    }
    assert!(errs > bytes.len() / 2);
    // A cut inside the first varint field's value is an error.
    assert_eq!(
        Message::decode(&bytes[..2], MAX),
        Err(ProtoError::Truncated)
    );
}

#[test]
fn overlong_varint() {
    let mut b = vec![0x08];
    b.extend([0x80; 10]);
    b.push(0x00);
    assert_eq!(Message::decode(&b, MAX), Err(ProtoError::BadVarint));
    // 10th byte above 1 overflows u64.
    let mut o = vec![0x08];
    o.extend([0xff; 9]);
    o.push(0x02);
    assert_eq!(Message::decode(&o, MAX), Err(ProtoError::BadVarint));
    // u64::MAX is accepted.
    let mut ok = vec![0x08];
    ok.extend([0xff; 9]);
    ok.push(0x01);
    assert_eq!(
        Message::decode(&ok, MAX).unwrap().varint(1),
        Ok(Some(u64::MAX))
    );
}

#[test]
fn length_past_end() {
    assert_eq!(
        Message::decode(&[0x12, 0x05, 1, 2], MAX),
        Err(ProtoError::LengthOutOfBounds)
    );
    let huge = [0x12, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01];
    assert_eq!(
        Message::decode(&huge, MAX),
        Err(ProtoError::LengthOutOfBounds)
    );
}

#[test]
fn size_bound_checked_up_front() {
    assert_eq!(Message::decode(&sample(), 3), Err(ProtoError::TooLarge));
}

#[test]
fn bad_tags() {
    assert_eq!(
        Message::decode(&[0x00, 0x00], MAX),
        Err(ProtoError::BadFieldNumber)
    );
    assert_eq!(Message::decode(&[0x0b], MAX), Err(ProtoError::Group)); // start group
    assert_eq!(Message::decode(&[0x0c], MAX), Err(ProtoError::Group)); // end group
    assert_eq!(Message::decode(&[0x0e], MAX), Err(ProtoError::BadWireType));
}

fn nest(levels: usize) -> Vec<u8> {
    let mut cur = Vec::new(); // innermost: empty message
    for _ in 0..levels {
        let mut m = Message::new();
        m.fields.push(Field {
            number: 1,
            value: Value::Len(cur),
        });
        cur = m.encode();
    }
    cur
}

fn descend(bytes: &[u8], steps: usize) -> Result<Message, ProtoError> {
    let mut m = Message::decode(bytes, MAX)?;
    for _ in 0..steps {
        m = m.message(1)?.ok_or(ProtoError::Truncated)?;
    }
    Ok(m)
}

#[test]
fn depth_limit() {
    // 17 wrappers: descending 16 times (depth 16) works, the 17th descent is rejected.
    let b = nest(17);
    assert!(descend(&b, 16).is_ok());
    assert_eq!(descend(&b, 17).err(), Some(ProtoError::TooDeep));
}

#[test]
fn unknown_fields_round_trip() {
    let bytes = sample();
    let m = Message::decode(&bytes, MAX).unwrap();
    assert_eq!(m.encode(), bytes);
    // Unknown field numbers stay in order.
    let nums: Vec<u32> = m.fields.iter().map(|f| f.number).collect();
    assert_eq!(nums, [1, 2, 3, 4, 5, 5]);
    assert_eq!(m.string(2), Ok(Some("héllo")));
}

#[test]
fn packed_and_unpacked_fixed64() {
    let ids = [1u64, 1 << 60, u64::MAX];
    let mut p = Message::new();
    p.push_fixed64_packed(2, &ids);
    let mut u = Message::new();
    u.push_fixed64_unpacked(2, &ids);
    assert_ne!(p.encode(), u.encode());
    for m in [p, u] {
        let d = Message::decode(&m.encode(), MAX).unwrap();
        assert_eq!(d.fixed64_repeated(2).unwrap(), ids);
    }
    let mut bad = Message::new();
    bad.fields.push(Field {
        number: 2,
        value: Value::Len(vec![0; 9]),
    });
    assert!(bad.fixed64_repeated(2).is_err());
}

#[test]
fn string_utf8_validated_on_access() {
    let m = Message::decode(&[0x0a, 0x02, 0xff, 0xfe], MAX).unwrap();
    assert_eq!(m.string(1), Err(ProtoError::Utf8));
    assert_eq!(m.varint(1), Err(ProtoError::WrongType));
}

#[test]
fn wrapper_messages() {
    let mut m = Message::new();
    m.push_wrapper_bool(1, true);
    m.push_wrapper_varint(2, 42);
    m.push_wrapper_string(3, "name");
    m.fields.push(Field {
        number: 4,
        value: Value::Len(vec![]),
    }); // empty wrapper = default
    let d = Message::decode(&m.encode(), MAX).unwrap();
    assert_eq!(d.wrapper_bool(1), Ok(Some(true)));
    assert_eq!(d.wrapper_varint(2), Ok(Some(42)));
    assert_eq!(d.wrapper_string(3), Ok(Some("name".to_owned())));
    assert_eq!(d.wrapper_varint(4), Ok(Some(0)));
    assert_eq!(d.wrapper_varint(9), Ok(None));
}

#[test]
fn base64_vectors() {
    for (raw, enc) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(base64_encode(raw.as_bytes()), enc);
        assert_eq!(base64_decode(enc).unwrap(), raw.as_bytes());
    }
    assert_eq!(base64_decode("Zm9vYg").unwrap(), b"foob"); // unpadded
    assert_eq!(base64_decode("Zm 9v\nYg=\t=").unwrap(), b"foob"); // whitespace
    assert_eq!(base64_decode("-_-_").unwrap(), [0xfb, 0xff, 0xbf]); // url-safe
    assert_eq!(base64_encode(&[0xfb, 0xff, 0xbf]), "+/+/");
    assert_eq!(base64_decode("Z"), Err(ProtoError::Base64));
    assert_eq!(base64_decode("Zm9v!"), Err(ProtoError::Base64));
    assert_eq!(base64_decode("Zg==Zg"), Err(ProtoError::Base64));
    // §17.2 example from 17.1's sibling: JSON context header.
    assert_eq!(
        base64_encode(br#"{"location":"chat_input"}"#),
        "eyJsb2NhdGlvbiI6ImNoYXRfaW5wdXQifQ=="
    );
}

/// Shaped like PreloadedUserSettings field 14 (GuildFolders): repeated GuildFolder {1 repeated fixed64
/// guild_ids, 2 Int64Value id, 3 StringValue name, 4 UInt64Value color}, 2 repeated fixed64 guild_positions.
#[test]
fn guild_folders_shape() {
    let mut f1 = Message::new();
    f1.push_fixed64_packed(1, &[111_111_111_111_111_111, 222_222_222_222_222_222]);
    f1.push_wrapper_varint(2, 1_700_000_000_000);
    f1.push_wrapper_string(3, "Games");
    f1.push_wrapper_varint(4, 0x5865f2);
    let mut f2 = Message::new();
    f2.push_fixed64_packed(1, &[333_333_333_333_333_333]);
    let mut gf = Message::new();
    gf.push_message(1, &f1);
    gf.push_message(1, &f2);
    gf.push_fixed64_packed(2, &[333_333_333_333_333_333, 111_111_111_111_111_111]);
    let mut top = Message::new();
    top.push_bool(2, true); // an unrelated sibling field
    top.push_message(14, &gf);
    let bytes = top.encode();
    let b64 = base64_encode(&bytes);

    let d = Message::decode(&base64_decode(&b64).unwrap(), MAX).unwrap();
    let g = d.message(14).unwrap().unwrap();
    let folders = g.messages(1).unwrap();
    assert_eq!(folders.len(), 2);
    assert_eq!(
        folders[0].fixed64_repeated(1).unwrap(),
        [111_111_111_111_111_111, 222_222_222_222_222_222]
    );
    assert_eq!(folders[0].wrapper_varint(2), Ok(Some(1_700_000_000_000)));
    assert_eq!(folders[0].wrapper_string(3), Ok(Some("Games".to_owned())));
    assert_eq!(folders[0].wrapper_varint(4), Ok(Some(0x5865f2)));
    assert_eq!(folders[1].wrapper_varint(2), Ok(None));
    assert_eq!(g.fixed64_repeated(2).unwrap().len(), 2);
    assert_eq!(d.encode(), bytes);
}

#[test]
fn random_bytes_never_panic() {
    let mut s = 0x9e37_79b9_7f4a_7c15u64;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    let seed = sample();
    for i in 0..4000 {
        let len = (next() % 64) as usize;
        let mut b: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        if i % 2 == 0 && !seed.is_empty() {
            // Mutate a valid message so deeper paths are exercised.
            b = seed.clone();
            let at = (next() as usize) % b.len();
            b[at] = next() as u8;
        }
        if let Ok(m) = Message::decode(&b, MAX) {
            for f in &m.fields {
                let _ = m.nested(f);
                let _ = m.string(f.number);
                let _ = m.fixed64_repeated(f.number);
                let _ = m.wrapper_varint(f.number);
            }
            let _ = m.encode();
        }
        let s: String = b.iter().map(|&c| char::from(c)).collect();
        let _ = base64_decode(&s);
    }
}
