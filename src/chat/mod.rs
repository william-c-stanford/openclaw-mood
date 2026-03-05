pub mod widget;

/// A single chat message
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Chat state: messages and scroll position
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    /// Streaming partial content for current assistant response
    pub streaming: Option<String>,
    /// Scroll offset from bottom (0 = pinned to bottom)
    pub scroll_offset: usize,
}

#[allow(dead_code)]
impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            streaming: None,
            scroll_offset: 0,
        }
    }

    pub fn push_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::User,
            content,
        });
        self.scroll_offset = 0;
    }

    pub fn push_assistant_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::Assistant,
            content,
        });
        self.scroll_offset = 0;
    }

    pub fn start_streaming(&mut self) {
        self.streaming = Some(String::new());
    }

    pub fn append_streaming(&mut self, delta: &str) {
        if let Some(ref mut s) = self.streaming {
            s.push_str(delta);
        }
    }

    /// Replace streaming content with a full snapshot (used when gateway sends accumulated text)
    pub fn set_streaming(&mut self, content: String) {
        self.streaming = Some(content);
    }

    pub fn finish_streaming(&mut self) {
        if let Some(content) = self.streaming.take() {
            self.messages.push(ChatMessage {
                role: Role::Assistant,
                content,
            });
        }
        self.scroll_offset = 0;
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_add(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }
}
