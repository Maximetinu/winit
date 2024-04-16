#![allow(clippy::disallowed_methods, clippy::single_match)]

use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::{Fullscreen, WindowBuilder},
};

extern crate console_error_panic_hook;
use std::panic;

pub fn main() -> Result<(), impl std::error::Error> {
    use winit::platform::web::WindowExtWebSys;

    // not strictly needed, but useful to see the panic in the console
    panic::set_hook(Box::new(console_error_panic_hook::hook));

    let event_loop = EventLoop::new().unwrap();

    let builder = WindowBuilder::new().with_title("A fantastic window!");
    #[cfg(wasm_platform)]
    let builder = {
        use winit::platform::web::WindowBuilderExtWebSys;
        builder.with_append(true)
    };
    let window = builder.build(&event_loop).unwrap();

    #[cfg(wasm_platform)]
    let log_list = wasm::insert_canvas_and_create_log_list(&window);

    event_loop.run(move |event, elwt| {
        #[cfg(wasm_platform)]
        wasm::log_event(&log_list, &event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => elwt.exit(),
            Event::AboutToWait => {
                window.request_redraw();
            }
            Event::WindowEvent {
                window_id,
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: Key::Character(c),
                                state: ElementState::Released,
                                ..
                            },
                        ..
                    },
            } if window_id == window.id() && c == "f" => {
                if window.fullscreen().is_some() {
                    window.set_fullscreen(None);
                } else {
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                }
            }
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                window_id,
            } if window_id == window.id() => {
                // Panicking in user code, during a redraw requested, while holding a lock, causes a 2nd panic when refreshing the tab.
                // I can reproduce locking any web_sys object, like the canvas or a `WebGl2RenderingContext`
                let canvas = window.canvas().unwrap();
                let mutex = std::sync::Mutex::new(canvas);
                {
                    let mut guard = mutex.lock().unwrap();
                    let _canvas = &mut *guard;
                    panic!("Simulating a panic in user code!");
                    // expected: the JS console should only see this panic happen, in the form of an exception
                    // actual: we see this panic and, when refreshing the tab, for barely 1 second, we see another BorrowMutError panic inside of Winit's code
                    // (it doesn't happen every time, but most of the times)
                    // repro steps: just run this code with `cargo run-wasm --example web`, check the JS console, and refresh. If it doesn't happen the 1st time, repeat a couple of times.
                }
            }
            _ => (),
        }
    })
}

#[cfg(wasm_platform)]
mod wasm {
    use std::num::NonZeroU32;

    use softbuffer::{Surface, SurfaceExtWeb};
    use wasm_bindgen::prelude::*;
    use winit::{
        event::{Event, WindowEvent},
        window::Window,
    };

    #[wasm_bindgen(start)]
    pub fn run() {
        console_log::init_with_level(log::Level::Debug).expect("error initializing logger");

        #[allow(clippy::main_recursion)]
        let _ = super::main();
    }

    pub fn insert_canvas_and_create_log_list(window: &Window) -> web_sys::Element {
        use winit::platform::web::WindowExtWebSys;

        let canvas = window.canvas().unwrap();
        let mut surface = Surface::from_canvas(canvas.clone()).unwrap();
        surface
            .resize(
                NonZeroU32::new(canvas.width()).unwrap(),
                NonZeroU32::new(canvas.height()).unwrap(),
            )
            .unwrap();
        let mut buffer = surface.buffer_mut().unwrap();
        buffer.fill(0xFFF0000);
        buffer.present().unwrap();

        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let body = document.body().unwrap();

        let style = &canvas.style();
        style.set_property("margin", "50px").unwrap();
        // Use to test interactions with border and padding.
        //style.set_property("border", "50px solid black").unwrap();
        //style.set_property("padding", "50px").unwrap();

        let log_header = document.create_element("h2").unwrap();
        log_header.set_text_content(Some("Event Log"));
        body.append_child(&log_header).unwrap();

        let log_list = document.create_element("ul").unwrap();
        body.append_child(&log_list).unwrap();
        log_list
    }

    pub fn log_event(log_list: &web_sys::Element, event: &Event<()>) {
        log::debug!("{:?}", event);

        // Getting access to browser logs requires a lot of setup on mobile devices.
        // So we implement this basic logging system into the page to give developers an easy alternative.
        // As a bonus its also kind of handy on desktop.
        let event = match event {
            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => None,
            Event::WindowEvent { event, .. } => Some(format!("{event:?}")),
            Event::Resumed | Event::Suspended => Some(format!("{event:?}")),
            _ => None,
        };
        if let Some(event) = event {
            let window = web_sys::window().unwrap();
            let document = window.document().unwrap();
            let log = document.create_element("li").unwrap();

            let date = js_sys::Date::new_0();
            log.set_text_content(Some(&format!(
                "{:02}:{:02}:{:02}.{:03}: {event}",
                date.get_hours(),
                date.get_minutes(),
                date.get_seconds(),
                date.get_milliseconds(),
            )));

            log_list
                .insert_before(&log, log_list.first_child().as_ref())
                .unwrap();
        }
    }
}
