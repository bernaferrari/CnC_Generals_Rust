// Auto-generated C++ compatibility shim for strtok_r

pub fn strtok_r<'a>(s: &'a str, delim: &str) -> std::vec::IntoIter<&'a str> {
    s.split(delim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .into_iter()
}
