use ariadne_core::WatcherSink;
use ariadne_watcher::NotifyWatcher;

#[test]
fn notify_watcher_implements_watcher_sink_port() {
    fn assert_sink<T: WatcherSink>() {}
    assert_sink::<NotifyWatcher>();
}
