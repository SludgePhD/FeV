/// ffi_enum! {}
macro_rules! ffi_enum {
    (
        $( #[$attrs:meta] )*
        $v:vis enum $name:ident: $native:ty {
            $(
                $( #[$variant_attrs:meta] )*
                $variant:ident = $value:expr
            ),+
            $(,)?
        }
    ) => {
        $( #[$attrs] )*
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(transparent)]
        $v struct $name(pub(crate) $native);

        #[allow(non_upper_case_globals)]
        impl $name {
            $(
                $( #[$variant_attrs] )*
                $v const $variant: Self = Self($value);
            )+
        }

        #[allow(unreachable_patterns)]
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match *self {
                    $(
                        Self::$variant => f.pad(stringify!($variant)),
                    )+

                    _ => write!(f, "(unknown: {:#x})", self.0),
                }
            }
        }
    };
}

/// This macro enforces that all `bitflags!` types in here are marked
/// `#[repr(transparent)]` and thus FFI-safe.
///
/// bitflags! {}
macro_rules! bitflags {
    ($($t:tt)*) => {
        bitflags::bitflags! {
            #[repr(transparent)]
            $($t)*
        }
    };
}
