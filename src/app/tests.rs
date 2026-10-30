use super::*;

mod state;
mod workspace;

pub(super) fn ready_app() -> App {
    let mut app = App::create(egui::Context::default(), "test".into());
    let done = app.completed.recv_timeout(Duration::from_secs(3)).unwrap();
    let (sender, receiver) = mpsc::channel();
    sender.send(done).unwrap();
    app.completed = receiver;
    app.receive();
    app
}

pub(super) fn finish(app: &mut App, work: Work) {
    let result = execute(work.job);
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Completed {
            target: None,
            revision: work.revision,
            reset_view: work.reset_view,
            label: work.label,
            user_action: work.user_action,
            result,
            elapsed: Duration::ZERO,
        })
        .unwrap();
    app.completed = receiver;
    app.receive();
}
