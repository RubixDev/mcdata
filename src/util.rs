//! Other Minecraft related types.

use std::ops::Range;

/// A Minecraft BlockPos storing an integer coordinate in 3D.
///
/// The [`x`](Self::x), [`y`](Self::y), and [`z`](Self::z) components are stored as [`i32`]s.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BlockPos {
    /// The `x` component of this vector.
    pub x: i32,
    /// The `y` component of this vector.
    pub y: i32,
    /// The `z` component of this vector.
    pub z: i32,
}

impl BlockPos {
    /// The position at (0, 0, 0).
    pub const ORIGIN: Self = Self::new(0, 0, 0);

    /// Create a new [`BlockPos`] given values for [`x`](Self::x), [`y`](Self::y), and
    /// [`z`](Self::z).
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    /// Compute the total volume of the box containing [`ORIGIN`](Self::ORIGIN) and `self`.
    pub const fn volume(&self) -> u32 {
        self.x.unsigned_abs() * self.y.unsigned_abs() * self.z.unsigned_abs()
    }

    /// Create a new [`BlockPos`] with the [absolute values](i32::abs) of each component.
    pub const fn abs(&self) -> Self {
        BlockPos {
            x: self.x.abs(),
            y: self.y.abs(),
            z: self.z.abs(),
        }
    }
}

/// A 3-dimensional cuboid composed of a [origin position](Self::origin) and the [size](Self::size).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cuboid {
    /// The position of the origin corner.
    pub origin: BlockPos,

    /// The size of this cuboid.
    ///
    /// This size should generally be kept positive.
    pub size: BlockPos,
}

impl Cuboid {
    /// Create a new [`Cuboid`] given its [`origin`](Self::origin) and [`size`](Self::size).
    pub const fn new(origin: BlockPos, size: BlockPos) -> Self {
        Self { origin, size }
    }

    /// Get the range of possible x coordinates within this cuboid.
    pub const fn x_range(&self) -> Range<i32> {
        self.origin.x..self.origin.x + self.size.x
    }

    /// Get the range of possible y coordinates within this cuboid.
    pub const fn y_range(&self) -> Range<i32> {
        self.origin.y..self.origin.y + self.size.y
    }

    /// Get the range of possible z coordinates within this cuboid.
    pub const fn z_range(&self) -> Range<i32> {
        self.origin.z..self.origin.z + self.size.z
    }
}

impl std::fmt::Debug for BlockPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

macro_rules! vec_op {
    ($type:ty, $trait:ident, $assign_trait:ident, $fn:ident, $assign_fn:ident, $op:tt) => {
        impl std::ops::$trait for $type {
            type Output = Self;

            fn $fn(self, rhs: Self) -> Self::Output {
                Self {
                    x: self.x $op rhs.x,
                    y: self.y $op rhs.y,
                    z: self.z $op rhs.z,
                }
            }
        }

        vec_assign_op!($type, $op, $type, $assign_trait, $assign_fn);
    };
}

macro_rules! vec_scalar_op {
    ($type:ty, $scalar:ty, $trait:ident, $assign_trait:ident, $fn:ident, $assign_fn:ident, $op:tt) => {
        impl std::ops::$trait<$scalar> for $type {
            type Output = Self;

            fn $fn(self, rhs: $scalar) -> Self::Output {
                Self {
                    x: self.x $op rhs,
                    y: self.y $op rhs,
                    z: self.z $op rhs,
                }
            }
        }

        vec_assign_op!($type, $op, $scalar, $assign_trait, $assign_fn);
    };
}

macro_rules! vec_assign_op {
    ($type:ty, $op:tt, $rhs:ty, $trait:ident, $fn:ident) => {
        impl std::ops::$trait<$rhs> for $type {
            fn $fn(&mut self, rhs: $rhs) {
                *self = *self $op rhs;
            }
        }
    }
}

vec_op!(BlockPos, Add, AddAssign, add, add_assign, +);
vec_op!(BlockPos, Sub, SubAssign, sub, sub_assign, -);
vec_scalar_op!(BlockPos, i32, Mul, MulAssign, mul, mul_assign, *);
vec_scalar_op!(BlockPos, i32, Div, DivAssign, div, div_assign, /);

impl std::ops::Mul for BlockPos {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }
}
vec_assign_op!(BlockPos, *, BlockPos, MulAssign, mul_assign);

impl std::ops::Neg for BlockPos {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }
}
