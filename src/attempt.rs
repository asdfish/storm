use {
    crate::config::Verbosity,
    std::{fmt::Display, num::NonZeroUsize},
};

pub const DEFAULT_ATTEMPTS: NonZeroUsize = NonZeroUsize::new(3).unwrap();

pub trait Predicate<E> {
    fn should_redo(&mut self, _: &E) -> bool;
}
impl<F, E> Predicate<E> for F
where
    F: FnMut(&E) -> bool,
{
    fn should_redo(&mut self, result: &E) -> bool {
        self(result)
    }
}

pub struct Always;
impl<E> Predicate<E> for Always {
    fn should_redo(&mut self, _: &E) -> bool {
        true
    }
}

pub trait Logger<E>
where
    E: Display,
{
    fn log(&mut self, n: usize, of: usize, err: &E);
}
pub struct StderrLogger {
    description: &'static str,
    verbosity: Verbosity,
}
impl StderrLogger {
    pub const fn new(description: &'static str, verbosity: Verbosity) -> Self {
        Self {
            description,
            verbosity,
        }
    }
}
impl<E> Logger<E> for StderrLogger
where
    E: Display,
{
    fn log(&mut self, n: usize, of: usize, err: &E) {
        self.verbosity
            .error(|| eprintln!("{} attempt {}/{}: {}", self.description, n, of, err));
    }
}

pub struct Attempt<T, E, L, O, P>
where
    E: Display,
    L: Logger<E>,
    O: FnMut() -> Result<T, E>,
    P: Predicate<E>,
{
    attempts: NonZeroUsize,
    logger: L,
    operation: O,
    predicate: P,
}
impl<T, E, L, O, P> Attempt<T, E, L, O, P>
where
    E: Display,
    L: Logger<E>,
    O: FnMut() -> Result<T, E>,
    P: Predicate<E>,
{
    pub const fn new(attempts: NonZeroUsize, logger: L, operation: O, predicate: P) -> Self {
        Self {
            attempts,
            logger,
            operation,
            predicate,
        }
    }

    /// Attempt to execute [Self::operation] [Self::attempts] times while [Self::predicate] returns
    /// true on errors.
    pub fn execute(&mut self) -> Result<T, E> {
        let mut last_err = None;

        for i in 0..self.attempts.get() {
            let result = (self.operation)();
            match result {
                Ok(output) => return Ok(output),
                Err(err) => {
                    if !self.predicate.should_redo(&err) {
                        return Err(err);
                    }

                    self.logger.log(i, self.attempts.get(), &err);
                    last_err = Some(err);
                }
            }
        }

        Err(last_err.unwrap())
    }
}
