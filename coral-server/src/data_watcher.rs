use std::collections::{HashMap, HashSet};

use coral_protocol::packets::play::entity::{EntityMetadata, MetadataValue};

pub struct DataWatcher {
    values: HashMap<u8, MetadataValue>,
    dirty: HashSet<u8>,
}
impl DataWatcher {
    pub fn new() -> Self {
        Self {
            values: HashMap::with_capacity(31),
            dirty: HashSet::with_capacity(31),
        }
    }

    /// Set a value; mark dirty only if it actually changed
    pub fn set(&mut self, index: u8, value: MetadataValue) {
        let changed = match (self.values.get(&index), &value) {
            (Some(MetadataValue::Byte(a)), MetadataValue::Byte(b)) => a != b,
            (Some(MetadataValue::Short(a)), MetadataValue::Short(b)) => a != b,
            (Some(MetadataValue::Int(a)), MetadataValue::Int(b)) => a != b,
            (Some(MetadataValue::Float(a)), MetadataValue::Float(b)) => a != b,
            (None, _) => true,
            _ => true, // type mismatch shouldn't happen, treat as changed
        };
        if !changed {
            return;
        }
        self.values.insert(index, value);
        self.dirty.insert(index);
    }

    /// Drain and return only the dirty entries as an EntityMetadata payload, clearing dirty flags.
    /// Returns None if nothing changed.
    pub fn take_dirty(&mut self, entity_id: i32) -> Option<EntityMetadata> {
        if self.dirty.is_empty() {
            return None;
        }
        let entries = self
            .dirty
            .drain()
            .map(|i| (i, self.values[&i].clone()))
            .collect();
        Some(EntityMetadata { entity_id, entries })
    }

    /// Full current state, for a newly-tracking viewer.
    pub fn full_snapshot(&self, entity_id: i32) -> EntityMetadata {
        let entries = self.values.iter().map(|(i, v)| (*i, v.clone())).collect();
        EntityMetadata { entity_id, entries }
    }
}
