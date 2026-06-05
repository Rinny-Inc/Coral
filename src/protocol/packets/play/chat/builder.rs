use crate::protocol::packets::play::chat::ChatMessageOut;

#[derive(Debug, Clone)]
pub enum ChatColor {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
    Reset,
}
impl ChatColor {
    pub fn name(&self) -> &str {
        match self {
            ChatColor::Black => "black",
            ChatColor::DarkBlue => "dark_blue",
            ChatColor::DarkGreen => "dark_green",
            ChatColor::DarkAqua => "dark_aqua",
            ChatColor::DarkRed => "dark_red",
            ChatColor::DarkPurple => "dark_purple",
            ChatColor::Gold => "gold",
            ChatColor::Gray => "gray",
            ChatColor::DarkGray => "dark_gray",
            ChatColor::Blue => "blue",
            ChatColor::Green => "green",
            ChatColor::Aqua => "aqua",
            ChatColor::Red => "red",
            ChatColor::LightPurple => "light_purple",
            ChatColor::Yellow => "yellow",
            ChatColor::White => "white",
            ChatColor::Reset => "reset",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatBuilder {
    text: String,
    color: Option<ChatColor>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}
impl ChatBuilder {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }

    pub fn color(mut self, color: ChatColor) -> Self {
        self.color = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    pub fn build(self) -> String {
        let escaped = self
            .text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");

        let mut json = format!("{{\"text\":\"{}\"", escaped);
        if let Some(color) = self.color {
            json.push_str(&format!(",\"color\":\"{}\"", color.name()));
        }

        if self.bold {
            json.push_str(",\"bold\":true");
        }
        if self.italic {
            json.push_str(",\"italic\":true");
        }
        if self.underline {
            json.push_str(",\"underlined\":true");
        }
        if self.strikethrough {
            json.push_str(",\"strikethrough\":true");
        }
        json.push('}');
        json
    }

    pub fn into_packet(self) -> ChatMessageOut {
        ChatMessageOut::from_json(&self.build())
    }

    pub fn plain_json(text: &str) -> String {
        Self::new(text).build()
    }

    pub fn colored_json(text: &str, color: ChatColor) -> String {
        Self::new(text).color(color).build()
    }

    pub fn chat_message(format: &str, username: &str, message: &str) -> String {
        let formatted = format
            .replace("{username}", username)
            .replace("{message}", message);
        Self::new(formatted).build()
    }
}
