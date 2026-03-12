//! Strongly-typed handles for referencing RTS resources without raw pointers.

use std::fmt;

macro_rules! define_handle {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub const INVALID: Self = Self(u32::MAX);

            pub const fn new(id: u32) -> Self {
                Self(id)
            }

            pub const fn is_valid(self) -> bool {
                self.0 != u32::MAX
            }

            pub const fn value(self) -> u32 {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::INVALID
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.is_valid() {
                    write!(f, "{}", self.0)
                } else {
                    write!(f, "INVALID")
                }
            }
        }
    };
}

define_handle!(PlayerHandle);
define_handle!(ObjectHandle);
define_handle!(CommandSetHandle);
define_handle!(ThingTemplateHandle);
define_handle!(SpecialPowerHandle);
define_handle!(UpgradeHandle);

pub type FrameNumber = u32;
