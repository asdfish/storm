use {
    parking_lot::{Condvar, Mutex},
    std::{
        mem::{
            MaybeUninit,
            replace,
        },
        sync::Arc,
    },
};

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let data = Arc::new((Mutex::new(MaybeUninit::uninit()), Condvar::new()));

    (Sender(Arc::clone(&data)), Receiver(data))
}

pub struct Sender<T>(Arc<(Mutex<MaybeUninit<T>>, Condvar)>);
impl<T> Sender<T> {
    pub fn send(self, message: T) {
        let mut lock = self.0.0.lock();
        lock.write(message);

        self.0.1.notify_one();
    }
}
pub struct Receiver<T>(Arc<(Mutex<MaybeUninit<T>>, Condvar)>);
impl<T> Receiver<T> {
    pub fn recv(self) -> T {
        let mut lock = self.0.0.lock();
        self.0.1.wait(&mut lock);

        // SAFETY: we only wake up once the writing is finished
        unsafe { replace(&mut *lock, MaybeUninit::uninit()).assume_init() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oneshot() {
        use std::{
            time::Duration,
            thread,
        };

        // check we do not pass because of luck
        (0..1_000)
            .for_each(|_| {
                let (tx, rx) = channel();

                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(1));
                    tx.send(true);
                });

                assert_eq!(rx.recv(), true);
            });
    }
}
