use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Property {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Default)]
pub struct PropertyMap {
    properties: HashMap<String, Vec<Property>>,
}

impl PropertyMap {
    pub fn new() -> Self {
        PropertyMap {
            properties: HashMap::new(),
        }
    }

    pub fn delegate(&self) -> &HashMap<String, Vec<Property>> {
        &self.properties
    }

    pub fn delegate_mut(&mut self) -> &mut HashMap<String, Vec<Property>> {
        &mut self.properties
    }

    pub fn insert(&mut self, key: String, property: Property) {
        self.properties.entry(key).or_insert_with(Vec::new).push(property);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<Property>> {
        self.properties.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<Vec<Property>> {
        self.properties.remove(key)
    }

    pub fn clear(&mut self) {
        self.properties.clear();
    }
}