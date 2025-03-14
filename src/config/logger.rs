pub trait Logger {
    fn error(&self, f: &dyn Fn());
    fn status(&self, f: &dyn Fn());
}

pub struct Null;
impl Logger for Null {
    fn error(&self, _: &dyn Fn()) {}
    fn status(&self, _: &dyn Fn()) {}
}
pub struct Quiet;
impl Logger for Quiet {
    fn error(&self, f: &dyn Fn()) {
        f()
    }
    fn status(&self, _: &dyn Fn()) {}
}
pub struct Verbose;
impl Logger for Verbose {
    fn error(&self, f: &dyn Fn()) {
        f()
    }
    fn status(&self, f: &dyn Fn()) {
        f()
    }
}
