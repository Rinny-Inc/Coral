#[derive(Debug, Clone)]
pub enum NbtTag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(u8, Vec<NbtTag>),
    Compound(Vec<(String, NbtTag)>),
    IntArray(Vec<i32>),
}
impl NbtTag {
    pub fn type_id(&self) -> u8 {
        match self {
            NbtTag::Byte(_) => 1,
            NbtTag::Short(_) => 2,
            NbtTag::Int(_) => 3,
            NbtTag::Long(_) => 4,
            NbtTag::Float(_) => 5,
            NbtTag::Double(_) => 6,
            NbtTag::ByteArray(_) => 7,
            NbtTag::String(_) => 8,
            NbtTag::List(_, _) => 9,
            NbtTag::Compound(_) => 10,
            NbtTag::IntArray(_) => 11,
        }
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        match self {
            NbtTag::Byte(v) => out.push(*v as u8),
            NbtTag::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
            NbtTag::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
            NbtTag::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
            NbtTag::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
            NbtTag::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
            NbtTag::ByteArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                out.extend_from_slice(v);
            }
            NbtTag::String(v) => {
                let bytes = v.as_bytes();
                out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
                out.extend_from_slice(bytes);
            }
            NbtTag::List(type_id, entries) => {
                out.push(*type_id);
                out.extend_from_slice(&(entries.len() as i32).to_be_bytes());
                for entry in entries {
                    entry.write(out);
                }
            }
            NbtTag::Compound(entries) => {
                for (name, tag) in entries {
                    out.push(tag.type_id());
                    let name_bytes = name.as_bytes();
                    out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
                    out.extend_from_slice(name_bytes);
                    tag.write(out);
                }
                out.push(0);
            }
            NbtTag::IntArray(v) => {
                out.extend_from_slice(&(v.len() as i32).to_be_bytes());
                for i in v {
                    out.extend_from_slice(&i.to_be_bytes());
                }
            }
        }
    }

    pub fn write_named_root(name: &str, tag: &NbtTag, out: &mut Vec<u8>) {
        out.reserve(tag.estimated_size() + name.len() + 3);
        out.push(10);
        let name_bytes = name.as_bytes();
        out.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());
        out.extend_from_slice(name_bytes);
        tag.write(out);
    }
    fn estimated_size(&self) -> usize {
        match self {
            Self::Byte(_) => 1,
            Self::Short(_) => 2,
            Self::Int(_) => 4,
            Self::Long(_) | Self::Double(_) => 8,
            Self::Float(_) => 4,
            Self::ByteArray(v) => 4 + v.len(),
            Self::String(s) => 2 + s.len(),
            Self::List(_, v) => 5 + v.iter().map(|t| t.estimated_size()).sum::<usize>(),
            Self::Compound(v) => {
                v.iter()
                    .map(|(k, t)| 4 + k.len() + t.estimated_size())
                    .sum::<usize>()
                    + 1
            }
            Self::IntArray(v) => 4 + v.len() * 4,
        }
    }

    pub fn set(&mut self, key: &str, value: NbtTag) {
        if let NbtTag::Compound(entries) = self {
            if let Some(entry) = entries.iter_mut().find(|(k, _)| k == key) {
                entry.1 = value;
            } else {
                entries.push((key.to_string(), value));
            }
        }
    }
    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        if let NbtTag::Compound(entries) = self {
            entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
    pub fn get_mut(&mut self, key: &str) -> Option<&mut NbtTag> {
        if let NbtTag::Compound(entries) = self {
            entries.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            NbtTag::Byte(v) => Some(*v as i64),
            NbtTag::Short(v) => Some(*v as i64),
            NbtTag::Int(v) => Some(*v as i64),
            NbtTag::Long(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        self.as_i64().map(|v| v as i32)
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            NbtTag::Float(v) => Some(*v as f64),
            NbtTag::Double(v) => Some(*v),
            _ => self.as_i64().map(|v| v as f64),
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        self.as_f64().map(|v| v as f32)
    }

    pub fn as_i8(&self) -> Option<i8> {
        self.as_i64().map(|v| v as i8)
    }
    pub fn as_i16_val(&self) -> Option<i16> {
        self.as_i64().map(|v| v as i16)
    }
    pub fn as_byte_array(&self) -> Option<&Vec<u8>> {
        if let NbtTag::ByteArray(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_list(&self) -> Option<&Vec<NbtTag>> {
        if let NbtTag::List(_, v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn as_string(&self) -> Option<&str> {
        if let NbtTag::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

pub struct NbtReader<'a> {
    data: &'a [u8],
    pos: usize,
    failed: bool,
    depth: u32,
}

const MAX_DEPTH: u32 = 512;

impl<'a> NbtReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            failed: false,
            depth: 0,
        }
    }

    pub fn is_ok(&self) -> bool {
        !self.failed
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn fail(&mut self) {
        self.failed = true;
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.failed {
            return None;
        }
        let end = self.pos.checked_add(n)?;
        let slice = self.data.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn read_u8(&mut self) -> u8 {
        match self.take(1) {
            Some(b) => b[0],
            None => {
                self.fail();
                0
            }
        }
    }
    fn read_i8(&mut self) -> i8 {
        self.read_u8() as i8
    }
    fn read_i16(&mut self) -> i16 {
        match self.take(2) {
            Some(b) => i16::from_be_bytes([b[0], b[1]]),
            None => {
                self.fail();
                0
            }
        }
    }
    fn read_i32(&mut self) -> i32 {
        match self.take(4) {
            Some(b) => i32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            None => {
                self.fail();
                0
            }
        }
    }
    fn read_i64(&mut self) -> i64 {
        match self.take(8) {
            Some(b) => i64::from_be_bytes(b.try_into().expect("8 bytes")),
            None => {
                self.fail();
                0
            }
        }
    }
    fn read_f32(&mut self) -> f32 {
        f32::from_bits(self.read_i32() as u32)
    }
    fn read_f64(&mut self) -> f64 {
        f64::from_bits(self.read_i64() as u64)
    }

    fn read_len(&mut self, element_size: usize) -> Option<usize> {
        let raw = self.read_i32();
        if self.failed || raw < 0 {
            self.fail();
            return None;
        }
        let len = raw as usize;
        match len.checked_mul(element_size) {
            Some(bytes) if bytes <= self.remaining() => Some(len),
            _ => {
                self.fail();
                None
            }
        }
    }

    fn read_string(&mut self) -> String {
        let raw = self.read_i16();
        if self.failed || raw < 0 {
            self.fail();
            return String::new();
        }
        match self.take(raw as usize) {
            Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            None => {
                self.fail();
                String::new()
            }
        }
    }

    pub fn read_tag(&mut self, type_id: u8) -> NbtTag {
        if self.failed {
            return NbtTag::Byte(0);
        }
        match type_id {
            1 => NbtTag::Byte(self.read_i8()),
            2 => NbtTag::Short(self.read_i16()),
            3 => NbtTag::Int(self.read_i32()),
            4 => NbtTag::Long(self.read_i64()),
            5 => NbtTag::Float(self.read_f32()),
            6 => NbtTag::Double(self.read_f64()),
            7 => {
                let Some(len) = self.read_len(1) else {
                    return NbtTag::ByteArray(Vec::new());
                };
                let bytes = self.take(len).map(<[u8]>::to_vec).unwrap_or_default();
                NbtTag::ByteArray(bytes)
            }
            8 => NbtTag::String(self.read_string()),
            9 => {
                let elem_type = self.read_u8();
                let Some(len) = self.read_len(1) else {
                    return NbtTag::List(elem_type, Vec::new());
                };
                if elem_type == 0 {
                    return NbtTag::List(0, Vec::new());
                }
                if self.enter() {
                    return NbtTag::List(elem_type, Vec::new());
                }
                let mut entries = Vec::new();
                for _ in 0..len {
                    if self.failed {
                        break;
                    }
                    self.leave();
                    entries.push(self.read_tag(elem_type));
                }
                NbtTag::List(elem_type, entries)
            }
            10 => {
                if self.enter() {
                    return NbtTag::Compound(Vec::new());
                }
                let mut entries = vec![];
                loop {
                    let tag_type = self.read_u8();
                    if tag_type == 0 || self.failed {
                        break;
                    }
                    let name = self.read_string();
                    let tag = self.read_tag(tag_type);
                    entries.push((name, tag));
                }
                self.leave();
                NbtTag::Compound(entries)
            }
            11 => {
                let Some(len) = self.read_len(4) else {
                    return NbtTag::IntArray(Vec::new());
                };
                let mut ints = Vec::with_capacity(len);
                for _ in 0..len {
                    ints.push(self.read_i32());
                }
                NbtTag::IntArray(ints)
            }
            _ => {
                self.fail();
                NbtTag::Byte(0)
            }
        }
    }

    fn enter(&mut self) -> bool {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.fail();
            return true;
        }
        false
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub fn read_named_root(&mut self) -> (String, NbtTag) {
        let type_id = self.read_u8();
        let name = self.read_string();
        let tag = self.read_tag(type_id);
        (name, tag)
    }
    // todo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_accessors_widen_across_tag_types() {
        for tag in [
            NbtTag::Byte(1),
            NbtTag::Short(1),
            NbtTag::Int(1),
            NbtTag::Long(1),
        ] {
            assert_eq!(tag.as_i32(), Some(1), "{tag:?} did not read as i32");
            assert_eq!(tag.as_i64(), Some(1));
            assert_eq!(tag.as_i16_val(), Some(1));
            assert_eq!(tag.as_i8(), Some(1));
        }
    }

    #[test]
    fn float_accessors_widen_across_tag_types() {
        assert_eq!(NbtTag::Float(2.5).as_f64(), Some(2.5));
        assert_eq!(NbtTag::Double(2.5).as_f32(), Some(2.5));
        assert_eq!(NbtTag::Int(64).as_f32(), Some(64.0));
    }

    #[test]
    fn non_numeric_tags_are_still_rejected() {
        let s = NbtTag::String("1".into());
        assert_eq!(s.as_i32(), None);
        assert_eq!(s.as_f32(), None);
        assert_eq!(NbtTag::List(1, vec![]).as_i32(), None);
        assert_eq!(NbtTag::Compound(vec![]).as_f64(), None);
    }
}

#[cfg(test)]
mod reader_safety_tests {
    use super::*;

    fn encoded(tag: &NbtTag) -> Vec<u8> {
        let mut out = Vec::new();
        NbtTag::write_named_root("root", tag, &mut out);
        out
    }

    #[test]
    fn every_truncation_of_a_valid_document_is_survivable() {
        let tag = NbtTag::Compound(vec![
            (
                "Pos".into(),
                NbtTag::List(6, vec![NbtTag::Double(0.5), NbtTag::Double(64.0)]),
            ),
            ("Blocks".into(), NbtTag::ByteArray(vec![7u8; 512])),
            ("Name".into(), NbtTag::String("Noksio".into())),
            ("Heights".into(), NbtTag::IntArray(vec![4; 64])),
            ("Health".into(), NbtTag::Float(20.0)),
        ]);
        let full = encoded(&tag);

        for n in 0..full.len() {
            let mut r = NbtReader::new(&full[..n]);
            r.read_named_root();
        }

        let mut r = NbtReader::new(&full);
        let (name, _) = r.read_named_root();
        assert_eq!(name, "root");
        assert!(r.is_ok(), "the complete document should read cleanly");
    }

    #[test]
    fn negative_lengths_are_rejected() {
        let mut data = vec![7u8]; // type
        data.extend_from_slice(&0i16.to_be_bytes()); // empty name
        data.extend_from_slice(&(-1i32).to_be_bytes());
        let mut r = NbtReader::new(&data);
        let (_, tag) = r.read_named_root();
        assert!(!r.is_ok());
        assert!(matches!(tag, NbtTag::ByteArray(v) if v.is_empty()));

        let mut data = vec![8u8];
        data.extend_from_slice(&0i16.to_be_bytes());
        data.extend_from_slice(&(-1i16).to_be_bytes());
        let mut r = NbtReader::new(&data);
        r.read_named_root();
        assert!(!r.is_ok());
    }

    #[test]
    fn an_oversized_length_is_rejected_without_allocating() {
        let mut data = vec![11u8]; // IntArray
        data.extend_from_slice(&0i16.to_be_bytes());
        data.extend_from_slice(&1_000_000_000i32.to_be_bytes());
        let mut r = NbtReader::new(&data);
        let (_, tag) = r.read_named_root();
        assert!(!r.is_ok());
        assert!(matches!(tag, NbtTag::IntArray(v) if v.is_empty()));
    }

    #[test]
    fn deep_nesting_stops_at_the_depth_limit() {
        let mut data = vec![10u8, 0, 0];
        for _ in 0..(MAX_DEPTH + 50) {
            data.extend_from_slice(&[10u8, 0, 0]);
        }
        let mut r = NbtReader::new(&data);
        r.read_named_root();
        assert!(!r.is_ok(), "the depth limit should have tripped");
    }

    #[test]
    fn a_list_of_end_tags_terminates() {
        let mut data = vec![9u8]; // List
        data.extend_from_slice(&0i16.to_be_bytes());
        data.push(0); // element type End
        data.extend_from_slice(&100i32.to_be_bytes());
        let mut r = NbtReader::new(&data);
        let (_, tag) = r.read_named_root();
        assert!(matches!(tag, NbtTag::List(0, v) if v.is_empty()));
    }

    #[test]
    fn random_bytes_never_panic() {
        let mut seed = 0x12345678u32;
        for _ in 0..2000 {
            let len = (seed % 64) as usize;
            let data: Vec<u8> = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    (seed >> 16) as u8
                })
                .collect();
            let mut r = NbtReader::new(&data);
            r.read_named_root();
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        }
    }

    #[test]
    fn valid_documents_are_unaffected() {
        let tag = NbtTag::Compound(vec![
            ("a".into(), NbtTag::Int(-5)),
            ("b".into(), NbtTag::String("hello".into())),
            ("c".into(), NbtTag::ByteArray(vec![1, 2, 3])),
            ("d".into(), NbtTag::IntArray(vec![-1, 0, 1])),
            ("e".into(), NbtTag::Double(-0.5)),
            ("f".into(), NbtTag::Long(i64::MIN)),
        ]);
        let data = encoded(&tag);
        let mut r = NbtReader::new(&data);
        let (name, back) = r.read_named_root();

        assert!(r.is_ok());
        assert_eq!(name, "root");
        assert_eq!(back.get("a").and_then(|t| t.as_i32()), Some(-5));
        assert_eq!(back.get("b").and_then(|t| t.as_string()), Some("hello"));
        assert_eq!(
            back.get("c").and_then(|t| t.as_byte_array()),
            Some(&vec![1u8, 2, 3])
        );
        assert_eq!(back.get("e").and_then(|t| t.as_f64()), Some(-0.5));
        assert_eq!(back.get("f").and_then(|t| t.as_i64()), Some(i64::MIN));
    }
}
