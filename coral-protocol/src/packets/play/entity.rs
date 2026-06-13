use crate::{
    auth::ProfileProperty,
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug)]
pub struct SpawnPlayer {
    pub entity_id: i32,
    pub uuid: uuid::Uuid,
    // pub username: String,
    pub properties: Vec<ProfileProperty>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub current_item: i16,
}

#[derive(Debug)]
pub struct EntityTeleport {
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: u8,
    pub pitch: u8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityRelativeMove {
    pub entity_id: i32,
    pub dx: i8,
    pub dy: i8,
    pub dz: i8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityLookAndMove {
    pub entity_id: i32,
    pub dx: i8,
    pub dy: i8,
    pub dz: i8,
    pub yaw: u8,
    pub pitch: u8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityLook {
    pub entity_id: i32,
    pub yaw: u8,
    pub pitch: u8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct DestroyEntities {
    pub entity_ids: Vec<i32>,
}

#[derive(Debug)]
pub struct EntityHeadLook {
    pub entity_id: i32,
    pub head_yaw: u8,
}

fn degrees_to_byte(degrees: f32) -> u8 {
    ((degrees / 360.0 * 256.0) as i32).rem_euclid(256) as u8
}

impl PacketOut for SpawnPlayer {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0C);
        writer.write_varint(self.entity_id);

        writer.write_uuid(&self.uuid); // 1.7 = String UUID

        // 1.7 only
        /*
        writer.write_string(&self.username);
        writer.write_varint(self.properties.len() as i32);
        for prop in &self.properties {
            writer.write_string(&prop.name);
            writer.write_string(&prop.value);
            match &prop.signature {
                Some(sig) => {
                    writer.write_bool(true);
                    writer.write_string(sig);
                }
                None => writer.write_bool(false),
            }
        }
        */

        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(degrees_to_byte(self.yaw));
        writer.write_byte(degrees_to_byte(self.pitch));
        writer.write_i16(self.current_item);
        // metadata
        writer.write_byte(0x66);
        writer.write_f32(20.0);
        writer.write_byte(0x7F);
        Ok(())
    }
}

impl PacketOut for EntityTeleport {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x18);
        writer.write_varint(self.entity_id);
        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(self.yaw);
        writer.write_byte(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for EntityRelativeMove {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x15);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.dx as u8);
        writer.write_byte(self.dy as u8);
        writer.write_byte(self.dz as u8);
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for EntityLookAndMove {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x17);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.dx as u8);
        writer.write_byte(self.dy as u8);
        writer.write_byte(self.dz as u8);
        writer.write_byte(self.yaw);
        writer.write_byte(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for EntityLook {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x16);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.yaw);
        writer.write_byte(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for DestroyEntities {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x13);
        writer.write_varint(self.entity_ids.len() as i32);
        for id in &self.entity_ids {
            writer.write_varint(*id);
        }
        Ok(())
    }
}

impl PacketOut for EntityHeadLook {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x19);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.head_yaw);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ArmAnimation;

#[derive(Debug)]
pub struct EntityAnimation {
    pub entity_id: i32,
    // 0 = swing arm
    // 1 = take damage
    // 2 = leave bed
    // 3 = eat food
    // 4 = critical effect
    // 5 = magic critical effect
    pub animation: u8,
}

impl PacketIn for ArmAnimation {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(ArmAnimation)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for EntityAnimation {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0B);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.animation);
        Ok(())
    }
}

#[derive(Debug)]
pub struct EntityAction {
    pub entity_id: i32,
    // 0 = start sneaking
    // 1 = stop sneaking
    // 2 = leave bed
    // 3 = start sprinting
    // 4 = stop sprinting
    // 5 = jump with horse
    // 6 = open ridden horse inventory
    pub action: i32,
    pub jump_boost: i32, // HORSE ONLY
}

#[derive(Debug)]
pub struct UseEntity {
    pub target_entity_id: i32,
    // 0 = interact
    // 1 = attack
    // 2 = interact at
    pub action: i32,
}

#[derive(Debug)]
pub struct EntityVelocity {
    pub entity_id: i32,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
}

#[derive(Debug)]
pub struct EntityMetadata {
    pub entity_id: i32,
    pub entity_flags: u8, // bit 1 = sneaking, bit 3 = sprinting
    pub skin_parts: u8,
}

impl PacketIn for EntityAction {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let entity_id = reader.read_varint();
        let action = reader.read_varint();
        let jump_boost = reader.read_varint();
        Ok(EntityAction {
            entity_id,
            action,
            jump_boost,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketIn for UseEntity {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let target_entity_id = reader.read_varint();
        let action = reader.read_varint();
        Ok(UseEntity {
            target_entity_id,
            action,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketOut for EntityVelocity {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        let factor = 8000.0f64;
        let max = 3.9f64;

        writer.write_varint(0x12);
        writer.write_varint(self.entity_id);
        writer.write_i16((self.vx.clamp(-max, max) * factor) as i16);
        writer.write_i16((self.vy.clamp(-max, max) * factor) as i16);
        writer.write_i16((self.vz.clamp(-max, max) * factor) as i16);
        Ok(())
    }
}
impl PacketOut for EntityMetadata {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1C);
        writer.write_varint(self.entity_id);

        // index 0
        writer.write_byte(0x00);
        writer.write_byte(self.entity_flags);

        // index 10 - skin layers
        writer.write_byte(0x0A);
        writer.write_byte(self.skin_parts);

        // end
        writer.write_byte(0x7F);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SpawnObject {
    pub entity_id: i32,
    pub object_type: u8,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: u8,
    pub pitch: u8,
    pub data: i32,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
}

impl PacketOut for SpawnObject {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0E);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.object_type);
        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(self.pitch);
        writer.write_byte(self.yaw);
        writer.write_i32(self.data);
        if self.data != 0 {
            writer.write_i16(self.vx);
            writer.write_i16(self.vy);
            writer.write_i16(self.vz);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CollectItem {
    pub collected_entity_id: i32,
    pub collector_entity_id: i32,
}
impl PacketOut for CollectItem {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0D);
        writer.write_varint(self.collected_entity_id);
        writer.write_varint(self.collector_entity_id);
        Ok(())
    }
}

#[derive(Debug)]
pub struct EntityEquipment {
    pub entity_id: i32,
    pub slot: i16,
    pub item_id: i16,
    pub count: u8,
    pub metadata: i16,
}
impl PacketOut for EntityEquipment {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x04);
        writer.write_varint(self.entity_id);
        writer.write_i16(self.slot);
        writer.write_i16(self.item_id);
        if self.item_id != -1 {
            writer.write_byte(self.count);
            writer.write_i16(self.metadata);
            writer.write_byte(0);
        }
        Ok(())
    }
}
