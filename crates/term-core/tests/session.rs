use std::sync::mpsc;
use std::time::{Duration, Instant};

use term_core::{GridSize, Palette, Session, SpawnOptions, TermEvent};

const TIMEOUT: Duration = Duration::from_secs(10);

fn wait_for_text(session: &Session, needle: &str) -> String {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let text = session.snapshot(&Palette::colmena_dark(), true).text();
        if text.contains(needle) {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "no apareció {needle:?}; pantalla:\n{text}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(rx: &mpsc::Receiver<TermEvent>) -> Option<i32> {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(TermEvent::Exited(code)) => return code,
            Ok(_) => {}
            Err(_) => assert!(Instant::now() < deadline, "no llegó TermEvent::Exited"),
        }
    }
}

fn program(unix: &str, windows: &str) -> Option<(String, Vec<String>)> {
    if cfg!(windows) {
        Some(("cmd.exe".into(), vec!["/C".into(), windows.into()]))
    } else {
        Some(("/bin/sh".into(), vec!["-c".into(), unix.into()]))
    }
}

#[test]
fn muestra_la_salida_usa_el_entorno_y_avisa_al_terminar() {
    let (tx, rx) = mpsc::channel();
    let mut env = std::collections::HashMap::new();
    env.insert("COLMENA_TEST".to_string(), "123".to_string());
    let opts = SpawnOptions {
        program: program("echo hola-$COLMENA_TEST", "echo hola-%COLMENA_TEST%"),
        env,
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, move |ev| {
        let _ = tx.send(ev);
    })
    .expect("spawn");

    wait_for_text(&session, "hola-123");
    let code = wait_for_exit(&rx);
    if cfg!(unix) {
        assert_eq!(code, Some(0));
    }
}

#[cfg(unix)]
#[test]
fn exporta_term_xterm_256color() {
    let opts = SpawnOptions {
        program: program("echo term=$TERM", ""),
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, |_| {}).expect("spawn");
    wait_for_text(&session, "term=xterm-256color");
}

#[cfg(unix)]
#[test]
fn lo_que_se_escribe_llega_al_proceso() {
    let opts = SpawnOptions {
        program: program("read x; echo recibido-$x", ""),
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, |_| {}).expect("spawn");
    session.write(b"abc\r".to_vec());
    wait_for_text(&session, "recibido-abc");
}

#[cfg(unix)]
#[test]
fn el_resize_llega_al_proceso() {
    let opts = SpawnOptions {
        program: program("sleep 0.5; stty size; sleep 5", ""),
        ..SpawnOptions::default()
    };
    let session = Session::spawn(opts, |_| {}).expect("spawn");
    session.resize(GridSize::new(100, 30));
    assert_eq!(session.size(), GridSize::new(100, 30));
    wait_for_text(&session, "30 100");
}
