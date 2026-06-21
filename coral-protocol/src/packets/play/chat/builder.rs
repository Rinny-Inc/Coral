use crate::packets::play::chat::{ChatMessageOut, builder::HoverEvent::ShowText};
use serde_json::{Value, json};

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
pub enum ClickEvent {
    OpenUrl(String),
    RunCommand(String),
    SuggestCommand(String),
    ChangePage(i32),
    OpenFile(String),
}
impl ClickEvent {
    fn action(&self) -> &str {
        match self {
            ClickEvent::OpenUrl(_) => "open_url",
            ClickEvent::RunCommand(_) => "run_command",
            ClickEvent::SuggestCommand(_) => "suggest_command",
            ClickEvent::ChangePage(_) => "change_page",
            ClickEvent::OpenFile(_) => "open_file",
        }
    }
    fn value(&self) -> String {
        match self {
            ClickEvent::OpenUrl(v)
            | ClickEvent::RunCommand(v)
            | ClickEvent::SuggestCommand(v)
            | ClickEvent::OpenFile(v) => v.clone(),
            ClickEvent::ChangePage(p) => p.to_string(),
        }
    }

    fn to_json(&self) -> Value {
        json!({ "action": self.action(), "value": &self.value() })
    }
}

#[derive(Debug, Clone)]
pub enum HoverEvent {
    ShowText(String),
    ShowItem {
        id: String,
        count: i32,
        /// raw SNBT tag content, e.g. `{display:{Name:"Foo"}}` — passed through as-is
        tag: Option<String>,
    },
    ShowAchievement(String),
    ShowEntity {
        name: String,
        entity_type: Option<String>,
        id: String, // entity uuid
    },
}
impl HoverEvent {
    fn action(&self) -> &str {
        match self {
            HoverEvent::ShowText(_) => "show_text",
            HoverEvent::ShowItem { .. } => "show_item",
            HoverEvent::ShowAchievement(_) => "show_achievement",
            HoverEvent::ShowEntity { .. } => "show_entity",
        }
    }
    fn value(&self) -> String {
        match self {
            HoverEvent::ShowText(t) => t.clone(),
            HoverEvent::ShowItem { id, count, tag } => match tag {
                Some(t) => format!("{{id:\"{}\",Count:{},tag:{}}}", id, count, t),
                None => format!("{{id:\"{}\",Count:{}}}", id, count),
            },
            HoverEvent::ShowAchievement(a) => a.clone(),
            HoverEvent::ShowEntity {
                name,
                entity_type,
                id,
            } => match entity_type {
                Some(t) => format!("{{name:\"{}\",type:\"{}\",id:\"{}\"}}", name, t, id),
                None => format!("{{name:\"{}\",id:\"{}\"}}", name, id),
            },
        }
    }
    fn to_json(&self) -> Value {
        json!({ "action": self.action(), "value": &self.value() })
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
    click_event: Option<ClickEvent>,
    hover_event: Option<HoverEvent>,
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
            click_event: None,
            hover_event: None,
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

    pub fn click_event(mut self, event: ClickEvent) -> Self {
        self.click_event = Some(event);
        self
    }
    pub fn hover_event(mut self, event: HoverEvent) -> Self {
        self.hover_event = Some(event);
        self
    }
    pub fn click_url(self, url: impl Into<String>) -> Self {
        self.click_event(ClickEvent::OpenUrl(url.into()))
    }
    pub fn click_command(self, cmd: impl Into<String>) -> Self {
        self.click_event(ClickEvent::RunCommand(cmd.into()))
    }
    pub fn click_suggest(self, cmd: impl Into<String>) -> Self {
        self.click_event(ClickEvent::SuggestCommand(cmd.into()))
    }
    pub fn hover_text(self, text: impl Into<String>) -> Self {
        self.hover_event(ShowText(text.into()))
    }
    pub fn hover_item(self, id: impl Into<String>, count: i32) -> Self {
        self.hover_event(HoverEvent::ShowItem {
            id: id.into(),
            count,
            tag: None,
        })
    }

    pub fn build_value(&self) -> Value {
        let mut obj = json!({ "text": self.text });
        let map = obj.as_object_mut().unwrap();
        if let Some(color) = &self.color {
            map.insert("color".into(), json!(color.name()));
        }
        if self.bold {
            map.insert("bold".into(), json!(true));
        }
        if self.italic {
            map.insert("italic".into(), json!(true));
        }
        if self.underline {
            map.insert("underlined".into(), json!(true));
        }
        if self.strikethrough {
            map.insert("strikethrough".into(), json!(true));
        }
        if let Some(click) = &self.click_event {
            map.insert("clickEvent".into(), click.to_json());
        }
        if let Some(hover) = &self.hover_event {
            map.insert("hoverEvent".into(), hover.to_json());
        }
        obj
    }

    pub fn build(self) -> String {
        self.build_value().to_string()
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

#[derive(Debug, Clone, Default)]
pub struct ChatAppender {
    segments: Vec<ChatBuilder>,
}
impl ChatAppender {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    pub fn add(mut self, segment: ChatBuilder) -> Self {
        self.segments.push(segment);
        self
    }

    pub fn text(self, text: impl Into<String>) -> Self {
        self.add(ChatBuilder::new(text))
    }

    pub fn build_value(self) -> Value {
        if self.segments.is_empty() {
            return json!({ "text": "" });
        }
        let mut iter = self.segments.iter();
        let mut root = iter.next().unwrap().build_value();
        let rest: Vec<Value> = iter.map(|s| s.build_value()).collect();
        if !rest.is_empty() {
            root.as_object_mut()
                .unwrap()
                .insert("extra".into(), Value::Array(rest));
        }
        root
    }

    pub fn build(self) -> String {
        self.build_value().to_string()
    }

    pub fn into_packet(self) -> ChatMessageOut {
        ChatMessageOut::from_json(&self.build())
    }
}
