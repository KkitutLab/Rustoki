macro_rules! sr {
    ($a:expr) => {
        if let Some(r) = $a {
            r
        } else {
            return;
        }
    };
}

macro_rules! er {
    ($res:expr, $msg:expr $(,)?) => {{
        match $res {
            Ok(v) => v,
            Err(e) => {
                eprintln!("{}: {}", $msg, e);
                return;
            }
        }
    }};
}

pub(crate) use er;
pub(crate) use sr;
