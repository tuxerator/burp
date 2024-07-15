#![allow(dead_code)]

use burp::run;

fn main() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(100 * 1024 * 1024)
        .build()
        .unwrap()
        .block_on(async {
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .init();

            let event_loop = winit::event_loop::EventLoop::new().unwrap();
            let window = winit::window::WindowBuilder::new()
                .with_title("egui + galileo")
                .build(&event_loop)
                .unwrap();

            run(window, event_loop).await;
        });
}
