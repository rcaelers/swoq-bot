mod swoq_interface {
    tonic::include_proto!("swoq.interface");
}
mod boulder_info;
mod default_observer;
mod game;
mod game_observer;
mod goal;
mod pathfinding;
mod strategy;
mod swoq;
mod visualizer;
mod visualizing_observer;
mod world_state;

use default_observer::DefaultObserver;
use dotenv::dotenv;
use game::Game;
use std::env;
use std::sync::{Arc, Mutex, mpsc};
use swoq::GameConnection;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use visualizer::run_visualizer;
use visualizing_observer::VisualizingObserver;
use world_state::WorldState;

fn get_env_var_i32(key: &str) -> Option<i32> {
    env::var(key).ok().and_then(|val| val.parse::<i32>().ok())
}

fn init_logging() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("robbot=debug,info"));

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    init_logging();

    let user_id = env::var("SWOQ_USER_ID")
        .expect("SWOQ_USER_ID environment variable is required, see README.md");
    let user_name = env::var("SWOQ_USER_NAME")
        .expect("SWOQ_USER_NAME environment variable is required, see README.md");
    let host =
        env::var("SWOQ_HOST").expect("SWOQ_HOST environment variable is required, see README.md");
    let level = get_env_var_i32("SWOQ_LEVEL");
    let seed = get_env_var_i32("SWOQ_SEED");
    let replays_folder = env::var("SWOQ_REPLAYS_FOLDER").ok();
    let enable_viz = env::var("SWOQ_VISUALIZER")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);

    tracing::info!("Visualizer enabled: {}", enable_viz);

    if enable_viz {
        let shared_state: Arc<Mutex<Option<WorldState>>> = Arc::new(Mutex::new(None));
        let game_state = Arc::clone(&shared_state);

        let (log_tx, log_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();

        std::thread::spawn(move || {
            let _ = ready_rx.recv();

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let connection = GameConnection::new(user_id, user_name, host, replays_folder)
                    .await
                    .unwrap();
                let mut game = Game::new(connection, VisualizingObserver::new(game_state, log_tx));
                let _ = game.run(level, seed).await;
            });
        });

        run_visualizer(shared_state, ready_tx, log_rx);
    } else {
        let connection = GameConnection::new(user_id, user_name, host, replays_folder).await?;
        let mut game = Game::new(connection, DefaultObserver);
        game.run(level, seed).await?;
    }

    Ok(())
}
