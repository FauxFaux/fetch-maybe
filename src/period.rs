use failure::bail;

struct Parts<'s> {
    inner: &'s str,
}

impl<'s> Parts<'s> {
    fn new(inner: &str) -> Parts {
        Parts { inner }
    }
}

impl<'s> Iterator for Parts<'s> {
    type Item = (&'s str, char);

    fn next(&mut self) -> Option<(&'s str, char)> {
        self.inner
            .find(|c: char| c.is_ascii_alphabetic())
            .map(|next| {
                let (init, point) = self.inner.split_at(next);
                self.inner = &point[1..];
                (init, point.as_bytes()[0].to_ascii_uppercase() as char)
            })
    }
}

pub fn parse_duration(s: &str) -> Result<chrono::Duration, failure::Error> {
    if let Ok(secs) = s.parse() {
        return Ok(chrono::Duration::seconds(secs));
    }

    bail!("can't parse as a duration: {:?}", s)
}
