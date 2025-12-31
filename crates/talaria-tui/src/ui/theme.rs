use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

pub struct Theme {
    pub bg: Color,
    pub panel: Color,
    pub text: Color,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub subtle: Color,
    pub border: Color,
}

impl Theme {
    pub fn default() -> Self {
        Self {
            bg: hex("#c1040b"),
            panel: hex("#000000"),
            text: hex("#E6EBF2"),
            accent: hex("#58C6FF"),
            ok: hex("#2BD576"),
            warn: hex("#F2B705"),
            err: hex("#FF5D5D"),
            subtle: hex("#E6EBF2"),
            border: hex("#000000"),
        }
    }

    pub fn base(&self) -> Style {
        Style::default().fg(self.text).bg(self.bg)
    }

    pub fn panel(&self) -> Style {
        Style::default().fg(self.text).bg(self.panel)
    }

    pub fn border(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn panel_block(&self) -> Block {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Thick)
            .style(self.panel())
            .border_style(self.border())
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

fn hex(input: &str) -> Color {
    let hex = input.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::Reset;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::Rgb(r, g, b)
}
