use shared::domain::jig::module::body::_groups::design::{
    PathCommand, Trace as RawTrace, TraceShape as RawTraceShape,
};
use utils::{math::BoundsF64, prelude::*, resize::ResizeInfo};

pub trait TraceExt {
    fn to_raw(&self) -> RawTrace;

    fn calc_bounds(&self, add_offset: bool) -> Option<BoundsF64>;

    fn calc_size(&self, resize_info: &ResizeInfo) -> Option<(f64, f64)> {
        self.calc_bounds(false)
            .map(|bounds| resize_info.get_size_full(bounds.width, bounds.height))
    }
}

impl TraceExt for RawTrace {
    fn to_raw(&self) -> RawTrace {
        self.clone()
    }

    fn calc_bounds(&self, add_offset: bool) -> Option<BoundsF64> {
        let offset = if add_offset {
            Some(self.transform.get_translation_2d())
        } else {
            None
        };

        match &self.shape {
            RawTraceShape::PathCommands(commands) => {
                calc_bounds(ShapeRef::PathCommands(commands), offset)
            }
            RawTraceShape::Path(path) => {
                calc_bounds(ShapeRef::Path(path), offset)
            },
            RawTraceShape::Ellipse(radius_x, radius_y) => {
                calc_bounds(ShapeRef::Ellipse(*radius_x, *radius_y), offset)
            }
            RawTraceShape::Rect(width, height) => {
                calc_bounds(ShapeRef::Rect(*width, *height), offset)
            }
        }
    }
}
pub enum ShapeRef<'a> {
    Path(&'a [(f64, f64)]),
    PathCommands(&'a [(PathCommand, bool)]),
    Ellipse(f64, f64),
    Rect(f64, f64),
}

//Gets the bounds of the shape itself, prior to any scaling or rotation
//if offset is supplied, then it is added
//TODO - document the use-cases for where offset is used
pub fn calc_bounds<'a>(shape: ShapeRef<'a>, offset: Option<(f64, f64)>) -> Option<BoundsF64> {
    let mut bounds = match shape {
        ShapeRef::PathCommands(_commands) => {
            //this will be tricky due to bezier curves etc. 
            //might need to actually draw the entire path in memory calc the extants
            unimplemented!("TODO - support calc_bounds for PathCommands!");
        }

        ShapeRef::Path(path) => {
            //Set to inverse of max values
            let mut left: f64 = 1.0;
            let mut right: f64 = 0.0;
            let mut top: f64 = 1.0;
            let mut bottom: f64 = 0.0;
            for (x, y) in path.iter() {
                let x = *x;
                let y = *y;
                if x < left {
                    left = x;
                }

                if x > right {
                    right = x;
                }

                if y < top {
                    top = y;
                }

                if y > bottom {
                    bottom = y;
                }
            }

            let width = right - left;
            let height = bottom - top;

            if width > 0.0 && height > 0.0 {
                Some(BoundsF64 {
                    x: left,
                    y: top,
                    width,
                    height,
                    invert_y: true,
                })
            } else {
                None
            }
        }

        ShapeRef::Ellipse(radius_x, radius_y) => Some(BoundsF64 {
            x: 0.0,
            y: 0.0,
            width: radius_x * 2.0,
            height: radius_y * 2.0,
            invert_y: true,
        }),
        ShapeRef::Rect(width, height) => Some(BoundsF64 {
            x: 0.0,
            y: 0.0,
            width,
            height,
            invert_y: true,
        }),
    };

    match (offset, bounds.as_mut()) {
        (Some((tx, ty)), Some(bounds)) => {
            bounds.x += tx;
            bounds.y += ty;
        }
        _ => {}
    };

    bounds
}

pub fn denormalize_command(command: &PathCommand, resize_info: &ResizeInfo) -> PathCommand {
    match command.clone() {
        PathCommand::MoveTo(x, y) => {
            let (x, y) = resize_info.get_pos_denormalized(x, y);
            PathCommand::MoveTo(x, y)
        }
        PathCommand::ClosePath => PathCommand::ClosePath,
        PathCommand::LineTo(x, y) => {
            let (x, y) = resize_info.get_pos_denormalized(x, y);
            PathCommand::LineTo(x, y)
        }
        PathCommand::HorizontalLineTo(x) => {
            let (x, _y) = resize_info.get_pos_denormalized(x, 0.0);
            PathCommand::HorizontalLineTo(x)
        }
        PathCommand::VerticalLineTo(y) => {
            let (_x, y) = resize_info.get_pos_denormalized(0.0, y);
            PathCommand::VerticalLineTo(y)
        }
        PathCommand::CurveTo(cp1x, cp1y, cp2x, cp2y, x, y) => {
            let (cp1x, cp1y) = resize_info.get_pos_denormalized(cp1x, cp1y);
            let (cp2x, cp2y) = resize_info.get_pos_denormalized(cp2x, cp2y);
            let (x, y) = resize_info.get_pos_denormalized(x, y);
            PathCommand::CurveTo(cp1x, cp1y, cp2x, cp2y, x, y)
        }
        PathCommand::SmoothCurveTo(cp1x, cp1y, x, y) => {
            let (cp1x, cp1y) = resize_info.get_pos_denormalized(cp1x, cp1y);
            let (x, y) = resize_info.get_pos_denormalized(x, y);
            PathCommand::SmoothCurveTo(cp1x, cp1y, x, y)
        }
        PathCommand::QuadCurveTo(cp1x, cp1y, x, y) => {
            let (cp1x, cp1y) = resize_info.get_pos_denormalized(cp1x, cp1y);
            let (x, y) = resize_info.get_pos_denormalized(x, y);
            PathCommand::QuadCurveTo(cp1x, cp1y, x, y)
        }
        PathCommand::SmoothQuadCurveTo(x, y) => {
            let (x, y) = resize_info.get_pos_denormalized(x, y);
            PathCommand::SmoothQuadCurveTo(x, y)
        }
        PathCommand::ArcTo(_a, _b, _c, _d, _e, _f, _g) => {
            unimplemented!("TODO: implement denormalize for ArcTo path command!")
        }
    }
}

// pub enum PathCommand {
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataMovetoCommands
//     MoveTo(f64, f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataLinetoCommands
//     ClosePath,
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataLinetoCommands
//     LineTo(f64, f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataLinetoCommands
//     HorizontalLineTo(f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataLinetoCommands
//     VerticalLineTo(f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataCubicBezierCommands
//     CurveTo(f64, f64, f64, f64, f64, f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataCubicBezierCommands
//     SmoothTo(f64, f64, f64, f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataQuadraticBezierCommands
//     QuadCurveTo(f64, f64, f64, f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataQuadraticBezierCommands
//     SmoothQuadCurveTo(f64, f64),
//     /// https://svgwg.org/svg2-draft/paths.html#PathDataEllipticalArcCommands
//     ArcTo(f64, f64, f64, f64, f64, f64)
// }
