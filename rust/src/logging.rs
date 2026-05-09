use log::Level;
use std::fmt::Display;

fn emit(target: &'static str, level: Level, event: &str, details: impl Display) {
    log::log!(target: target, level, "event={event} {details}");
}

pub(crate) fn config(level: Level, event: &str, details: impl Display) {
    emit("gitenv::config", level, event, details);
}

pub(crate) fn intent(level: Level, event: &str, details: impl Display) {
    emit("gitenv::intent", level, event, details);
}

pub(crate) fn operation(level: Level, event: &str, details: impl Display) {
    emit("gitenv::operation", level, event, details);
}

pub(crate) fn actions(level: Level, event: &str, details: impl Display) {
    emit("gitenv::actions", level, event, details);
}

pub(crate) fn status(level: Level, event: &str, details: impl Display) {
    emit("gitenv::status", level, event, details);
}

pub(crate) fn system(level: Level, event: &str, details: impl Display) {
    emit("gitenv::system", level, event, details);
}
