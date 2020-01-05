use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct WorkQueue {
    queue: Arc<Mutex<Vec<PathBuf>>>,
}

impl WorkQueue {
    pub fn new(to_build: Vec<PathBuf>) -> Self {
        Self {
            queue: Arc::new(Mutex::new(to_build)),
        }
    }

    pub fn push(&mut self, path: PathBuf) {
        self.queue
            .lock()
            .expect("Failed to get lock on WorkQueue")
            .push(path);
    }
}

impl std::iter::Iterator for WorkQueue {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        self.queue
            .lock()
            .expect("Failed to get lock on WorkQueue")
            .pop()
    }
}
