use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub text: Color,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub subtle: Color,
}

impl Theme {
    pub fn default() -> Self {
        Self {
            bg: Color::Black,
            text: Color::Gray,
            accent: Color::Cyan,
            ok: Color::Green,
            warn: Color::Yellow,
            err: Color::Red,
            subtle: Color::DarkGray,
        }
    }

    pub fn title(&self) -> Style {
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn subtle(&self) -> Style {
        Style::default().fg(self.subtle)
    }

    pub fn ok(&self) -> Style {
        Style::default().fg(self.ok)
    }

    pub fn warn(&self) -> Style {
        Style::default().fg(self.warn)
    }

    pub fn err(&self) -> Style {
        Style::default().fg(self.err)
    }
}
