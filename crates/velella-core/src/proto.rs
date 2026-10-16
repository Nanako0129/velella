//! Minimal, dependency-free, bounded protobuf wire-format codec (menu plan slice PB, T-11).
//!
//! The only protobuf decoder in core: `Message::decode` yields a generic field tree (one level; nested
//! messages are decoded on demand via `Message::nested`, depth-limited to `MAX_DEPTH`) that encodes
//! back byte for byte for canonical input. Typed schemas (PreloadedUserSettings, FrecencyUserSettings)
//! sit on top. Groups are rejected. Nothing here panics on any input.
#![allow(dead_code)] // consumers (ID, F2) land in later slices

use std::fmt;

/// Deepest nesting `Message::nested` will descend to (the top-level message is depth 0).
pub const MAX_DEPTH: usize = 16;
/// Largest legal field number (2^29 - 1).
const MAX_FIELD: u64 = 0x1FFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtoError {
    /// Input ended inside a tag, varint, fixed-width value, or length-delimited payload.
    Truncated,
    /// Varint longer than 10 bytes, or a 10th byte above 1 (overflows u64).
    BadVarint,
    /// Wire type 6 or 7.
    BadWireType,
    /// Wire type 3 or 4 (deprecated groups are not supported).
    Group,
    /// Field number 0 or above 2^29 - 1.
    BadFieldNumber,
    /// A length-delimited length that exceeds the remaining bytes.
    LengthOutOfBounds,
    /// Nesting deeper than `MAX_DEPTH`.
    TooDeep,
    /// Input larger than the caller's size bound.
    TooLarge,
    /// A string field that is not valid UTF-8.
    Utf8,
    /// A field present with a different wire type than the accessor expects.
    WrongType,
    /// Invalid base64 character or dangling 6-bit group.
    Base64,
}

impl fmt::Display for ProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "protobuf error: {self:?}")
    }
}
impl std::error::Error for ProtoError {}

type Result<T> = std::result::Result<T, ProtoError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Varint(u64),
    Fixed64(u64),
    Len(Vec<u8>),
    Fixed32(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub number: u32,
    pub value: Value,
}

/// An ordered list of fields (unknown ones included, in wire order).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    pub fields: Vec<Field>,
    depth: usize,
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64> {
    let mut out = 0u64;
    for i in 0..10 {
        let b = *buf.get(*pos).ok_or(ProtoError::Truncated)?;
        *pos += 1;
        if i == 9 && b > 1 {
            return Err(ProtoError::BadVarint);
        }
        out |= u64::from(b & 0x7f) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(out);
        }
    }
    Err(ProtoError::BadVarint)
}

fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = pos.checked_add(n).ok_or(ProtoError::Truncated)?;
    let s = buf.get(*pos..end).ok_or(ProtoError::Truncated)?;
    *pos = end;
    Ok(s)
}

fn write_varint(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

impl Message {
    pub fn new() -> Self {
        Self::default()
    }

    /// Decodes one level of `bytes`; rejects input above `max_len` before reading anything.
    pub fn decode(bytes: &[u8], max_len: usize) -> Result<Self> {
        if bytes.len() > max_len {
            return Err(ProtoError::TooLarge);
        }
        Self::decode_at(bytes, 0)
    }

    fn decode_at(buf: &[u8], depth: usize) -> Result<Self> {
        let mut fields = Vec::new();
        let mut pos = 0;
        while pos < buf.len() {
            let tag = read_varint(buf, &mut pos)?;
            let number = tag >> 3;
            if number == 0 || number > MAX_FIELD {
                return Err(ProtoError::BadFieldNumber);
            }
            let value = match tag & 7 {
                0 => Value::Varint(read_varint(buf, &mut pos)?),
                1 => Value::Fixed64(u64::from_le_bytes(
                    take(buf, &mut pos, 8)?
                        .try_into()
                        .map_err(|_| ProtoError::Truncated)?,
                )),
                2 => {
                    let len = read_varint(buf, &mut pos)?;
                    let len = usize::try_from(len).map_err(|_| ProtoError::LengthOutOfBounds)?;
                    if len > buf.len() - pos {
                        return Err(ProtoError::LengthOutOfBounds);
                    }
                    Value::Len(take(buf, &mut pos, len)?.to_vec())
                }
                5 => Value::Fixed32(u32::from_le_bytes(
                    take(buf, &mut pos, 4)?
                        .try_into()
                        .map_err(|_| ProtoError::Truncated)?,
                )),
                3 | 4 => return Err(ProtoError::Group),
                _ => return Err(ProtoError::BadWireType),
            };
            fields.push(Field {
                number: number as u32,
                value,
            });
        }
        Ok(Self { fields, depth })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for f in &self.fields {
            let n = u64::from(f.number) << 3;
            match &f.value {
                Value::Varint(v) => {
                    write_varint(&mut out, n);
                    write_varint(&mut out, *v);
                }
                Value::Fixed64(v) => {
                    write_varint(&mut out, n | 1);
                    out.extend_from_slice(&v.to_le_bytes());
                }
                Value::Len(b) => {
                    write_varint(&mut out, n | 2);
                    write_varint(&mut out, b.len() as u64);
                    out.extend_from_slice(b);
                }
                Value::Fixed32(v) => {
                    write_varint(&mut out, n | 5);
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        out
    }

    /// Decodes a length-delimited field as a nested message (one level deeper than `self`).
    pub fn nested(&self, field: &Field) -> Result<Message> {
        match &field.value {
            Value::Len(b) if self.depth < MAX_DEPTH => Self::decode_at(b, self.depth + 1),
            Value::Len(_) => Err(ProtoError::TooDeep),
            _ => Err(ProtoError::WrongType),
        }
    }

    pub fn all(&self, number: u32) -> impl Iterator<Item = &Field> {
        self.fields.iter().filter(move |f| f.number == number)
    }

    /// Last occurrence wins, as in protobuf for singular fields.
    pub fn get(&self, number: u32) -> Option<&Field> {
        self.fields.iter().rev().find(|f| f.number == number)
    }

    pub fn varint(&self, number: u32) -> Result<Option<u64>> {
        match self.get(number) {
            None => Ok(None),
            Some(Field {
                value: Value::Varint(v),
                ..
            }) => Ok(Some(*v)),
            Some(_) => Err(ProtoError::WrongType),
        }
    }

    pub fn bool(&self, number: u32) -> Result<Option<bool>> {
        Ok(self.varint(number)?.map(|v| v != 0))
    }

    /// UTF-8 is validated here, on access.
    pub fn string(&self, number: u32) -> Result<Option<&str>> {
        match self.get(number) {
            None => Ok(None),
            Some(Field {
                value: Value::Len(b),
                ..
            }) => std::str::from_utf8(b)
                .map(Some)
                .map_err(|_| ProtoError::Utf8),
            Some(_) => Err(ProtoError::WrongType),
        }
    }

    pub fn message(&self, number: u32) -> Result<Option<Message>> {
        self.get(number).map(|f| self.nested(f)).transpose()
    }

    pub fn messages(&self, number: u32) -> Result<Vec<Message>> {
        self.all(number).map(|f| self.nested(f)).collect()
    }

    /// Repeated fixed64: unpacked (`Fixed64` fields) and packed (`Len`, multiple of 8), in wire order.
    pub fn fixed64_repeated(&self, number: u32) -> Result<Vec<u64>> {
        let mut out = Vec::new();
        for f in self.all(number) {
            match &f.value {
                Value::Fixed64(v) => out.push(*v),
                Value::Len(b) if b.len() % 8 == 0 => out.extend(b.chunks_exact(8).map(|c| {
                    let mut a = [0u8; 8];
                    a.copy_from_slice(c);
                    u64::from_le_bytes(a)
                })),
                Value::Len(_) => return Err(ProtoError::Truncated),
                _ => return Err(ProtoError::WrongType),
            }
        }
        Ok(out)
    }

    /// Wrapper message (BoolValue/Int64Value/UInt64Value): present wrapper with no field 1 means 0.
    pub fn wrapper_varint(&self, number: u32) -> Result<Option<u64>> {
        Ok(match self.message(number)? {
            None => None,
            Some(w) => Some(w.varint(1)?.unwrap_or(0)),
        })
    }

    pub fn wrapper_bool(&self, number: u32) -> Result<Option<bool>> {
        Ok(self.wrapper_varint(number)?.map(|v| v != 0))
    }

    /// StringValue.
    pub fn wrapper_string(&self, number: u32) -> Result<Option<String>> {
        Ok(match self.message(number)? {
            None => None,
            Some(w) => Some(w.string(1)?.unwrap_or("").to_owned()),
        })
    }

    fn push(&mut self, number: u32, value: Value) {
        self.fields.push(Field { number, value });
    }

    pub fn push_varint(&mut self, number: u32, v: u64) {
        self.push(number, Value::Varint(v));
    }

    pub fn push_bool(&mut self, number: u32, v: bool) {
        self.push_varint(number, u64::from(v));
    }

    pub fn push_string(&mut self, number: u32, s: &str) {
        self.push(number, Value::Len(s.as_bytes().to_vec()));
    }

    pub fn push_message(&mut self, number: u32, m: &Message) {
        self.push(number, Value::Len(m.encode()));
    }

    pub fn push_fixed64_unpacked(&mut self, number: u32, vs: &[u64]) {
        for v in vs {
            self.push(number, Value::Fixed64(*v));
        }
    }

    /// One length-delimited field (proto3 default for repeated scalars); omitted when empty.
    pub fn push_fixed64_packed(&mut self, number: u32, vs: &[u64]) {
        if !vs.is_empty() {
            self.push(
                number,
                Value::Len(vs.iter().flat_map(|v| v.to_le_bytes()).collect()),
            );
        }
    }

    pub fn push_wrapper_varint(&mut self, number: u32, v: u64) {
        let mut w = Message::new();
        w.push_varint(1, v);
        self.push_message(number, &w);
    }

    pub fn push_wrapper_bool(&mut self, number: u32, v: bool) {
        self.push_wrapper_varint(number, u64::from(v));
    }

    pub fn push_wrapper_string(&mut self, number: u32, s: &str) {
        let mut w = Message::new();
        w.push_string(1, s);
        self.push_message(number, &w);
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard alphabet, always `=`-padded.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(c.get(1).copied().unwrap_or(0)) << 8)
            | u32::from(c.get(2).copied().unwrap_or(0));
        for i in 0..4 {
            if i <= c.len() {
                out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Lenient: standard or url-safe alphabet, ASCII whitespace skipped, padding optional.
pub fn base64_decode(s: &str) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3 + 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut padded = false;
    for b in s.bytes() {
        let v = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => {
                padded = true;
                continue;
            }
            b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return Err(ProtoError::Base64),
        };
        if padded {
            return Err(ProtoError::Base64); // data after padding
        }
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    if bits == 6 {
        return Err(ProtoError::Base64); // dangling 6-bit group
    }
    Ok(out)
}
