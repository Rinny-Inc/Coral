use crate::packets::PacketOut;

#[derive(Debug, Clone)]
pub struct Explosion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub power: f32,
    pub records: Vec<(i8, i8, i8)>,
    pub motion: (f64, f64, f64),
}
impl Explosion {
    /// Turns absolute block positions into the packet's relative records
    pub fn records_from(
        x: f64,
        y: f64,
        z: f64,
        blocks: impl IntoIterator<Item = (i32, i32, i32)>,
    ) -> Vec<(i8, i8, i8)> {
        let (cx, cy, cz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        blocks
            .into_iter()
            .filter_map(|(bx, by, bz)| {
                Some((
                    i8::try_from(bx - cx).ok()?,
                    i8::try_from(by - cy).ok()?,
                    i8::try_from(bz - cz).ok()?,
                ))
            })
            .collect()
    }
}

impl PacketOut for Explosion {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x27);
        writer.write_f32(self.x as f32);
        writer.write_f32(self.y as f32);
        writer.write_f32(self.z as f32);
        writer.write_f32(self.power);
        writer.write_i32(self.records.len() as i32);
        for &(dx, dy, dz) in &self.records {
            writer.write_byte(dx as u8);
            writer.write_byte(dy as u8);
            writer.write_byte(dz as u8);
        }
        writer.write_f32(self.motion.0 as f32);
        writer.write_f32(self.motion.1 as f32);
        writer.write_f32(self.motion.2 as f32);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::writer::Writer;

    use super::*;

    fn encoded(e: &Explosion) -> Vec<u8> {
        let mut w = Writer::new();
        e.encode(&mut w).unwrap();
        w.into_data()
    }

    fn blast() -> Explosion {
        Explosion {
            x: 8.5,
            y: 64.5,
            z: -3.5,
            power: 4.0,
            records: vec![(0, 0, 0), (1, -2, 3)],
            motion: (0.25, 0.5, -0.25),
        }
    }

    #[test]
    fn the_packet_id_is_0x27() {
        assert_eq!(encoded(&blast())[0], 0x27);
    }

    #[test]
    fn the_layout_is_header_records_then_motion() {
        let bytes = encoded(&blast());
        assert_eq!(bytes.len(), 1 + 16 + 4 + 2 * 3 + 12);
        let count = i32::from_be_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]);
        assert_eq!(count, 2);
    }

    #[test]
    fn records_are_offsets_from_the_floored_centre() {
        let records = Explosion::records_from(8.5, 64.5, -3.5, [(8, 64, -4), (9, 62, -1)]);
        assert_eq!(records, vec![(0, 0, 0), (1, -2, 3)]);
    }

    #[test]
    fn a_negative_centre_still_floors_rather_than_truncates() {
        let records = Explosion::records_from(0.5, 64.5, -3.5, [(0, 64, -4)]);
        assert_eq!(records, vec![(0, 0, 0)]);
    }

    #[test]
    fn blocks_too_far_out_to_fit_a_byte_are_dropped() {
        let records =
            Explosion::records_from(0.5, 64.5, 0.5, [(0, 64, 0), (200, 64, 0), (0, 64, -200)]);
        assert_eq!(records, vec![(0, 0, 0)]);
    }

    #[test]
    fn an_empty_blast_still_encodes() {
        let quiet = Explosion {
            records: vec![],
            ..blast()
        };
        let bytes = encoded(&quiet);
        assert_eq!(bytes[0], 0x27);
        assert_eq!(bytes.len(), 1 + 16 + 4 + 12);
    }
}
