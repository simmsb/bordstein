use crate::bindings::{self, GAlign, GPoint, GRect, GSize};

impl GRect {
    pub const fn new(x: i16, y: i16, w: i16, h: i16) -> Self {
        Self {
            origin: GPoint { x, y },
            size: GSize { w, h },
        }
    }

    pub fn left(&self) -> i16 {
        self.origin.x
    }
    pub fn right(&self) -> i16 {
        self.origin.x + self.size.w
    }
    pub fn top(&self) -> i16 {
        self.origin.y
    }
    pub fn bottom(&self) -> i16 {
        self.origin.y + self.size.h
    }

    pub fn center_x(&self) -> i16 {
        self.origin.x + self.size.w / 2
    }
    pub fn center_y(&self) -> i16 {
        self.origin.y + self.size.h / 2
    }
}

impl GRect {
    /// Places this rectangle directly below another, matching the left edge.
    pub fn below(mut self, other: GRect, gap: i16) -> Self {
        self.origin.y = other.bottom() + gap;
        self.origin.x = other.left();
        self
    }

    /// Places this rectangle directly above another, matching the left edge.
    pub fn above(mut self, other: GRect, gap: i16) -> Self {
        self.origin.y = other.top() - self.size.h - gap;
        self.origin.x = other.left();
        self
    }

    /// Places this rectangle to the right of another, matching the top edge.
    pub fn right_of(mut self, other: GRect, gap: i16) -> Self {
        self.origin.x = other.right() + gap;
        self.origin.y = other.top();
        self
    }

    /// Places this rectangle to the left of another, matching the top edge.
    pub fn left_of(mut self, other: GRect, gap: i16) -> Self {
        self.origin.x = other.left() - self.size.w - gap;
        self.origin.y = other.top();
        self
    }
}

impl GRect {
    pub fn align_left(mut self, other: GRect) -> Self {
        self.origin.x = other.left();
        self
    }

    pub fn align_right(mut self, other: GRect) -> Self {
        self.origin.x = other.right() - self.size.w;
        self
    }

    pub fn align_top(mut self, other: GRect) -> Self {
        self.origin.y = other.top();
        self
    }

    pub fn align_bottom(mut self, other: GRect) -> Self {
        self.origin.y = other.bottom() - self.size.h;
        self
    }

    /// Centers this rectangle horizontally relative to another.
    pub fn center_horizontally(mut self, other: GRect) -> Self {
        self.origin.x = other.center_x() - (self.size.w / 2);
        self
    }

    /// Centers this rectangle vertically relative to another.
    pub fn center_vertically(mut self, other: GRect) -> Self {
        self.origin.y = other.center_y() - (self.size.h / 2);
        self
    }

    /// Centers this rectangle perfectly inside another.
    pub fn center_inside(self, other: GRect) -> Self {
        self.center_horizontally(other).center_vertically(other)
    }
}

impl GRect {
    pub fn inset(mut self, amount: i16) -> Self {
        self.origin.x += amount;
        self.origin.y += amount;
        self.size.w = (self.size.w - 2 * amount).max(0);
        self.size.h = (self.size.h - 2 * amount).max(0);
        self
    }

    /// Displaces the rectangle by a relative offset vector.
    pub fn translate(mut self, dx: i16, dy: i16) -> Self {
        self.origin.x += dx;
        self.origin.y += dy;
        self
    }

    /// Returns a new rectangle with a forced explicit size, keeping the origin.
    pub fn with_size(mut self, w: i16, h: i16) -> Self {
        self.size.w = w;
        self.size.h = h;
        self
    }

    pub fn with_height(mut self, h: i16) -> Self {
        self.size.h = h;
        self
    }

    pub fn with_width(mut self, w: i16) -> Self {
        self.size.w = w;
        self
    }

    pub fn align(mut self, inside: &GRect, alignment: GAlign, clip: bool) -> Self {
        unsafe {
            bindings::grect_align(&raw mut self, &raw const *inside, alignment, clip);
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl GRect {
    /// Shrinks this rectangle's boundaries to prevent it from overlapping with
    /// an encroaching rectangle situated at a specific `Edge`.
    pub fn shrink_to_avoid(mut self, obstacle: GRect, side: Edge, gap: i16) -> Self {
        match side {
            Edge::Right => {
                let target_right = obstacle.left() - gap;
                if target_right > self.left() {
                    self.size.w = target_right - self.left();
                } else {
                    self.size.w = 0;
                }
            }

            Edge::Left => {
                let target_left = obstacle.right() + gap;
                if target_left < self.right() {
                    let diff = target_left - self.origin.x;
                    self.origin.x = target_left;
                    self.size.w = (self.size.w - diff).max(0);
                } else {
                    self.origin.x = self.right();
                    self.size.w = 0;
                }
            }
            Edge::Bottom => {
                let target_bottom = obstacle.top() - gap;
                if target_bottom > self.top() {
                    self.size.h = target_bottom - self.top();
                } else {
                    self.size.h = 0;
                }
            }
            Edge::Top => {
                let target_top = obstacle.bottom() + gap;
                if target_top < self.bottom() {
                    let diff = target_top - self.origin.y;
                    self.origin.y = target_top;
                    self.size.h = (self.size.h - diff).max(0);
                } else {
                    self.origin.y = self.bottom();
                    self.size.h = 0;
                }
            }
        }
        self
    }
}

impl GPoint {
    pub const fn new(x: i16, y: i16) -> Self {
        Self { x, y }
    }

    pub const fn offset(self, by: GSize) -> Self {
        Self {
            x: self.x + by.w,
            y: self.y + by.h,
        }
    }

    pub const fn with_size(self, size: GSize) -> GRect {
        GRect { origin: self, size }
    }
}

impl GSize {
    pub const fn new(w: i16, h: i16) -> Self {
        Self { w, h }
    }

    pub const fn with_origin(self, origin: GPoint) -> GRect {
        GRect { origin, size: self }
    }
}
