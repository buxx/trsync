use std::fmt::Display;

use self::instance::GuiInstance;

pub mod add;
pub mod instance;
pub mod root;

#[derive(Debug, Clone)]
pub enum Panel {
    Root,
    Instance(GuiInstance),
    AddInstance(GuiInstance),
}

impl Display for Panel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Panel::Root => f.write_str("Configuration"),
            Panel::Instance(instance) => f.write_str(&instance.name.to_string()),
            Panel::AddInstance(_) => f.write_str("Ajouter"),
        }
    }
}

impl PartialEq for Panel {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Instance(l0), Self::Instance(r0)) => l0.name == r0.name,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl Eq for Panel {}
