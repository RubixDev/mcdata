//! Other Minecraft related types.

/// A Minecraft BlockPos storing an integer coordinate in 3D.
///
/// The [`x`](Self::x), [`y`](Self::y), and [`z`](Self::z) components are stored as [`i32`]s and
/// can thus be both positive and negative.
/// If only positive values should be allowed, use [`UVec3`] instead.
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
    pub const fn volume(&self) -> usize {
        self.x.unsigned_abs() as usize
            * self.y.unsigned_abs() as usize
            * self.z.unsigned_abs() as usize
    }

    /// Convert this position into a [`UVec3`] with the [absolute values](i32::unsigned_abs) of
    /// each component.
    pub const fn abs(&self) -> UVec3 {
        UVec3 {
            x: self.x.unsigned_abs(),
            y: self.x.unsigned_abs(),
            z: self.x.unsigned_abs(),
        }
    }
}

/// A Positive integer coordinate in 3D.
///
/// The [`x`](Self::x), [`y`](Self::y), and [`z`](Self::z) components are stored as [`u32`]s and
/// can thus only be positive.
/// If negative values should also be allowed, use [`BlockPos`] instead.
#[derive(Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UVec3 {
    /// The `x` component of this vector.
    pub x: u32,
    /// The `y` component of this vector.
    pub y: u32,
    /// The `z` component of this vector.
    pub z: u32,
}

impl UVec3 {
    /// The position at (0, 0, 0).
    pub const ORIGIN: Self = Self::new(0, 0, 0);

    /// Create a new [`UVec3`] given values for [`x`](Self::x), [`y`](Self::y), and
    /// [`z`](Self::z).
    pub const fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    /// Compute the total volume of the box containing [`ORIGIN`](Self::ORIGIN) and `self`.
    pub const fn volume(&self) -> usize {
        self.x as usize * self.y as usize * self.z as usize
    }
}

macro_rules! vec_debug {
    ($type:ty) => {
        impl std::fmt::Debug for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "({}, {}, {})", self.x, self.y, self.z)
            }
        }
    };
}
vec_debug!(BlockPos);
vec_debug!(UVec3);
