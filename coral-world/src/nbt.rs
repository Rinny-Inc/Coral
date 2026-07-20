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

    pub fn as_i32(&self) -> Option<i32> {
        if let NbtTag::Int(v) = self {
            Some(*v)
        } else {
            None
        }
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
    pub fn as_i8(&self) -> Option<i8> {
        if let NbtTag::Byte(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub fn as_i16_val(&self) -> Option<i16> {
        if let NbtTag::Short(v) = self {
            Some(*v)
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
}
impl<'a> NbtReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_u8(&mut self) -> u8 {
        let v = self.data[self.pos];
        self.pos += 1;
        v
    }
    fn read_i8(&mut self) -> i8 {
        self.read_u8() as i8
    }
    fn read_i16(&mut self) -> i16 {
        let v = i16::from_be_bytes(self.data[self.pos..self.pos + 2].try_into().unwrap());
        self.pos += 2;
        v
    }
    fn read_i32(&mut self) -> i32 {
        let v = i32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        v
    }
    fn read_i64(&mut self) -> i64 {
        let v = i64::from_be_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        v
    }
    fn read_f32(&mut self) -> f32 {
        let v = f32::from_be_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;
        v
    }
    fn read_f64(&mut self) -> f64 {
        let v = f64::from_be_bytes(self.data[self.pos..self.pos + 8].try_into().unwrap());
        self.pos += 8;
        v
    }
    fn read_string(&mut self) -> String {
        let len = self.read_i16() as usize;
        let s = String::from_utf8_lossy(&self.data[self.pos..self.pos + len]).to_string();
        self.pos += len;
        s
    }

    pub fn read_tag(&mut self, type_id: u8) -> NbtTag {
        match type_id {
            1 => NbtTag::Byte(self.read_i8()),
            2 => NbtTag::Short(self.read_i16()),
            3 => NbtTag::Int(self.read_i32()),
            4 => NbtTag::Long(self.read_i64()),
            5 => NbtTag::Float(self.read_f32()),
            6 => NbtTag::Double(self.read_f64()),
            7 => {
                let len = self.read_i32() as usize;
                let bytes = self.data[self.pos..self.pos + len].to_vec();
                self.pos += len;
                NbtTag::ByteArray(bytes)
            }
            8 => NbtTag::String(self.read_string()),
            9 => {
                let elem_type = self.read_u8();
                let len = self.read_i32() as usize;
                let mut entries = Vec::with_capacity(len);
                for _ in 0..len {
                    entries.push(self.read_tag(elem_type));
                }
                NbtTag::List(elem_type, entries)
            }
            10 => {
                let mut entries = vec![];
                loop {
                    let tag_type = self.read_u8();
                    if tag_type == 0 {
                        break;
                    }
                    let name = self.read_string();
                    let tag = self.read_tag(tag_type);
                    entries.push((name, tag));
                }
                NbtTag::Compound(entries)
            }
            11 => {
                let len = self.read_i32() as usize;
                let mut ints = Vec::with_capacity(len);
                for _ in 0..len {
                    ints.push(self.read_i32());
                }
                NbtTag::IntArray(ints)
            }
            _ => NbtTag::Byte(0),
        }
    }

    pub fn read_named_root(&mut self) -> (String, NbtTag) {
        let type_id = self.read_u8();
        let name = self.read_string();
        let tag = self.read_tag(type_id);
        (name, tag)
    }
    // todo
}
