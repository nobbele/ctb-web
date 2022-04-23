#[derive(Copy, Clone, Default, Eq, Hash, PartialEq, PartialOrd, Ord, Debug)]
pub struct Frozen<T>(pub T);

impl<T> Frozen<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> std::ops::Deref for Frozen<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: std::fmt::Display> std::fmt::Display for Frozen<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        self.0.fmt(f)
    }
}
