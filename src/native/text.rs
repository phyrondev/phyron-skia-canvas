use crate::native::color::RgbaLinear;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextBoxOptions {
    pub color: RgbaLinear,
    pub font_family: Option<String>,
    pub font_size: f32,
    pub font_weight: i32,
    pub horizontal_align: TextAlign,
    pub vertical_align: VerticalAlign,
    pub opacity: f32,
}

impl Default for TextBoxOptions {
    fn default() -> Self {
        Self {
            color: RgbaLinear::opaque(0.0, 0.0, 0.0),
            font_family: None,
            font_size: 16.0,
            font_weight: 400,
            horizontal_align: TextAlign::Left,
            vertical_align: VerticalAlign::Top,
            opacity: 1.0,
        }
    }
}
