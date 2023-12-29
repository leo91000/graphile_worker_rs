#[derive(Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum RunTaskError<E> {
    TaskPanic,
    TaskAborted,
    TaskError(E),
}

#[derive(Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct SpawnTaskResult<E> {
    pub duration: std::time::Duration,
    pub result: Result<(), RunTaskError<E>>,
}

impl<E> SpawnTaskResult<E> {
    pub fn duration(&self) -> std::time::Duration {
        self.duration
    }

    pub fn result(&self) -> &Result<(), RunTaskError<E>> {
        &self.result
    }
}

impl<E> RunTaskError<E> {
    pub fn is_panic(&self) -> bool {
        matches!(self, RunTaskError::TaskPanic)
    }

    pub fn is_aborted(&self) -> bool {
        matches!(self, RunTaskError::TaskAborted)
    }

    pub fn is_task_error(&self) -> bool {
        matches!(self, RunTaskError::TaskError(_))
    }
}
