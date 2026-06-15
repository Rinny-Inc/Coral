#[derive(Debug, Clone, PartialEq)]
pub enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolMaterial {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    Any,
}
