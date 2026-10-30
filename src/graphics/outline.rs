//! A line on the floor around one half of the engine block, a set distance
//! out from it: along the block's front, around that cylinder's rounded
//! end, and back along the block's back. The burner follows it around the
//! hot half and the ice heap around the cold half.

use std::f32::consts::{
    FRAC_PI_2,
    PI, //
};

use macroquad::prelude::{
    Vec2,
    vec2, //
};

/// Which half of the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Half {
    Hot,
    Cold,
}

#[derive(Clone, Copy, Debug)]
pub struct HalfOutline {
    half: Half,
    /// Distance from the middle of the block to the cylinder's axis.
    offset: f32,
    /// Distance from the axis to the line around the rounded end, and from
    /// the block's middle plane to each straight run.
    radius: f32,
    /// Length of each straight run, from the axis toward the middle.
    straight: f32,
}

impl HalfOutline {
    /// The line `radius` from the axis of the cylinder `offset` from the
    /// middle, whose straight runs reach `reach` of the way to the middle.
    pub fn new(half: Half, offset: f32, radius: f32, reach: f32) -> Self {
        Self {
            half,
            offset,
            radius,
            straight: reach.clamp(0.0, 1.0) * offset,
        }
    }

    /// The cylinder's axis, seen from above (x, z).
    pub fn axis(self) -> Vec2 {
        vec2(self.side() * self.offset, 0.0)
    }

    pub fn radius(self) -> f32 {
        self.radius
    }

    pub fn length(self) -> f32 {
        2.0 * self.straight + PI * self.radius
    }

    /// The point `fraction` of the way along, from the front run's inner
    /// end, and the direction straight out from the block there.
    pub fn at(self, fraction: f32) -> (Vec2, Vec2) {
        let along = fraction.clamp(0.0, 1.0) * self.length();
        let curve = PI * self.radius;
        // Worked out for the hot half (on the -x side), then mirrored.
        let (point, out) = if along < self.straight {
            (
                vec2(-self.offset + self.straight - along, self.radius),
                Vec2::Y,
            )
        } else if along <= self.straight + curve {
            let out = Vec2::from_angle(FRAC_PI_2 + (along - self.straight) / self.radius);
            (vec2(-self.offset, 0.0) + self.radius * out, out)
        } else {
            (
                vec2(-self.offset + along - self.straight - curve, -self.radius),
                -Vec2::Y,
            )
        };
        let mirror = vec2(-self.side(), 1.0);
        (point * mirror, out * mirror)
    }

    /// -1 for the hot half, +1 for the cold.
    fn side(self) -> f32 {
        match self.half {
            Half::Hot => -1.0,
            Half::Cold => 1.0,
        }
    }
}

/// How far `point` (seen from above) is outside the block whose bores are
/// `offset` from its middle and whose rounded ends have radius `outer`;
/// negative inside. For the tests of everything placed around the block.
#[cfg(test)]
pub fn outside_block(offset: f32, outer: f32, point: Vec2) -> f32 {
    let nearest = vec2(point.x.clamp(-offset, offset), 0.0);
    point.distance(nearest) - outer
}

#[cfg(test)]
#[path = "../../tests/unit/graphics/outline.rs"]
mod tests;
