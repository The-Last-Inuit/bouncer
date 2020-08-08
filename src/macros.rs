#![macro_use]

macro_rules! fail {
    ($expr:expr) => {
        return Err(::std::convert::From::from($expr));
    };
}

macro_rules! invalid_type_error {
    ($v:expr, $det:expr) => {{
        fail!(invalid_type_error_inner!($v, $det))
    }};
}

macro_rules! invalid_type_error_inner {
    ($v:expr, $det:expr) => {
        redis::RedisError::from((
            redis::ErrorKind::TypeError,
            "Response was of incompatible type",
            format!("{:?} (response was {:?})", $det, $v),
        ))
    };
}
