use core::cell::RefCell;

use alloc::rc::Rc;
use embassy_net::Stack;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, channel::Sender};
use embassy_time::Duration;
use esp_backtrace as _;

use esp_wifi::wifi::{WifiDevice, WifiStaDevice};

use picoserve::{
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
    Router,
};

use crate::{net::wait_for_network, MoveCommand, QUEUE_SIZE};

pub const WEB_TASK_POOL_SIZE: usize = 1;

async fn get_root(
    Query(query): Query<MoveCommand>,
    State(sender): State<Rc<RefCell<Sender<'static, NoopRawMutex, MoveCommand, QUEUE_SIZE>>>>,
) -> impl IntoResponse {
    log::info!("Received request to move stepper");
    sender.borrow().send(query).await;
    "hello world!"
}

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
pub async fn web_task(
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    config: &'static picoserve::Config<Duration>,
    sender: Sender<'static, NoopRawMutex, MoveCommand, QUEUE_SIZE>,
) -> ! {
    wait_for_network(stack).await;

    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    let app = Router::new().route("/", get(get_root));

    loop {
        let mut socket =
            embassy_net::tcp::TcpSocket::new(stack, &mut tcp_rx_buffer, &mut tcp_tx_buffer);

        log::info!("Listening on port {:?}", port);
        if let Err(e) = socket.accept(port).await {
            log::warn!("accept error: {:?}", e);
            continue;
        }

        log::info!("Received connection from {:?}", socket.remote_endpoint());

        let state = Rc::new(RefCell::new(sender));
        match picoserve::serve_with_state(&app, config, &mut http_buffer, socket, &state).await {
            Ok(handled_requests_count) => {
                log::info!("{handled_requests_count} requests handled",);
            }
            Err(err) => log::error!("{err:?}"),
        }
    }
}
