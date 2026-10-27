//! Inline value entry: a text field that takes a length expression with units.
//!
//! `12.5`, `12.5 mm`, `3/4 in`, `1 1/2 in` and `(40 - 2*3) mm` all work: the text goes
//! through `kubit_math::evaluate`, a bare number means millimetres, and the result is
//! validated as a length. Nothing is parsed here.

use egui::{Key, RichText, Ui};
use kubit_math::Length;
use kubit_math::expr::{Dim, EmptyContext, evaluate};

/// Why a typed value was rejected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueError {
    /// The expression does not parse or evaluate.
    Invalid(String),
    /// It is an angle or another non-length quantity.
    NotALength,
}

/// Millimetres from a length expression; a bare number is millimetres.
pub fn parse_length_mm(text: &str) -> Result<f64, ValueError> {
    let q = evaluate(text.trim(), &EmptyContext).map_err(|e| ValueError::Invalid(e.to_string()))?;
    if q.dim != Dim::SCALAR && q.dim != Dim::LENGTH {
        return Err(ValueError::NotALength);
    }
    Length::from_mm(q.value)
        .map(Length::mm)
        .map_err(|e| ValueError::Invalid(e.to_string()))
}

/// Formats a millimetre value for the field (trailing zeros trimmed).
pub fn format_mm(mm: f64) -> String {
    let text = format!("{mm:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} mm")
}

/// The state of one inline field.
#[derive(Clone, Debug, PartialEq)]
pub struct ValueField {
    /// What the user typed.
    pub text: String,
    /// The last rejection, shown under the field.
    pub error: Option<ValueError>,
    focused: bool,
}

/// What the field asked for this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldAction {
    /// Still editing.
    None,
    /// Enter with a valid value, mm.
    Apply(f64),
    /// Escape.
    Cancel,
}

impl ValueField {
    /// A field showing `mm`, selected so typing replaces it.
    pub fn new(mm: f64) -> Self {
        Self {
            text: format_mm(mm),
            error: None,
            focused: false,
        }
    }

    /// Draws the field; Enter validates and applies, Escape cancels.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        error_color: egui::Color32,
        messages: [&str; 2],
    ) -> FieldAction {
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.text)
                .desired_width(110.0)
                .font(egui::TextStyle::Monospace),
        );
        if !self.focused {
            response.request_focus();
            self.focused = true;
        }
        let mut action = FieldAction::None;
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            action = FieldAction::Cancel;
        } else if ui.input(|i| i.key_pressed(Key::Enter)) {
            match parse_length_mm(&self.text) {
                Ok(mm) => action = FieldAction::Apply(mm),
                Err(e) => {
                    self.error = Some(e);
                    response.request_focus();
                }
            }
        }
        if let Some(e) = &self.error {
            let [invalid, not_length] = messages;
            let text = match e {
                ValueError::Invalid(_) => invalid,
                ValueError::NotALength => not_length,
            };
            ui.label(RichText::new(text).color(error_color).small());
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_and_units_become_millimetres() {
        assert_eq!(parse_length_mm("12.5"), Ok(12.5));
        assert_eq!(parse_length_mm(" 12.5 mm "), Ok(12.5));
        assert_eq!(parse_length_mm("2 cm"), Ok(20.0));
        let three_quarters = parse_length_mm("3/4 in").unwrap_or(f64::NAN);
        assert!((three_quarters - 19.05).abs() < 1e-9, "{three_quarters}");
        let mixed = parse_length_mm("1 1/2 in").unwrap_or(f64::NAN);
        assert!((mixed - 38.1).abs() < 1e-9, "{mixed}");
        assert_eq!(parse_length_mm("-5"), Ok(-5.0));
    }

    #[test]
    fn bad_input_is_rejected_not_panicking() {
        assert_eq!(parse_length_mm("45 deg"), Err(ValueError::NotALength));
        for bad in ["", "abc", "1/0", "1e400", "((", "5 mm * 2 mm", "NaN"] {
            assert!(parse_length_mm(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn format_trims_zeros() {
        assert_eq!(format_mm(5.0), "5 mm");
        assert_eq!(format_mm(2.25), "2.25 mm");
        assert_eq!(format_mm(-0.1), "-0.1 mm");
    }
}
