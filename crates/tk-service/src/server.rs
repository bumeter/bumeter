use std::io::{self, BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use interprocess::local_socket::{prelude::*, ListenerOptions, Name, Stream};

use crate::protocol::{Request, Response};

pub type Handler = Arc<dyn Fn(Request) -> Response + Send + Sync>;

pub fn listen(name: Name<'static>) -> io::Result<interprocess::local_socket::Listener> {
    ListenerOptions::new().name(name).create_sync()
}

/// Mỗi kết nối một thread; dừng khi `stop` bật (người dừng phải "chọc" socket một lần).
pub fn serve(listener: interprocess::local_socket::Listener, handler: Handler, stop: Arc<AtomicBool>) {
    for conn in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(conn) = conn else { continue };
        let h = handler.clone();
        std::thread::spawn(move || handle(conn, h));
    }
}

fn handle(conn: Stream, handler: Handler) {
    let mut reader = BufReader::new(conn);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let resp = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handler(req),
            Err(e) => Response::err(0, e.to_string()),
        };
        let Ok(mut out) = serde_json::to_string(&resp) else { return };
        out.push('\n');
        if reader.get_mut().write_all(out.as_bytes()).is_err() {
            return;
        }
    }
}
