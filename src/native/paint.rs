use crate::native::color::RgbaLinear;

#[derive(Debug, Clone, PartialEq)]
pub struct ShapePaint {
    pub fill: Option<RgbaLinear>,
    pub stroke: Option<RgbaLinear>,
    pub stroke_width: f32,
    pub anti_alias: bool,
}

impl ShapePaint {
    pub fn fill(color: RgbaLinear) -> Self {
        Self {
            fill: Some(color),
            stroke: None,
            stroke_width: 0.0,
            anti_alias: true,
        }
    }

    pub fn stroke(color: RgbaLinear, width: f32) -> Self {
        Self {
            fill: None,
            stroke: Some(color),
            stroke_width: width,
            anti_alias: true,
        }
    }

    pub fn fill_and_stroke(fill: RgbaLinear, stroke: RgbaLinear, width: f32) -> Self {
        Self {
            fill: Some(fill),
            stroke: Some(stroke),
            stroke_width: width,
            anti_alias: true,
        }
    }
}
