use dotenv::dotenv;
use std::env;
use std::sync::{Arc, Mutex, mpsc};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use robbot::infra::{CompositeObserver, DefaultObserver, GameConnection, VisualizingObserver};
use robbot::ui::{GameStateSnapshot, run_visualizer};
use robbot::planners;

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
        .with_thread_ids(true)
        .with_thread_names(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");
}

async fn run_heuristic_game_loop(
    mut game: planners::heuristic::Game,
    level: Option<i32>,
    seed: Option<i32>,
    loop_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // If loop is enabled, restart the game indefinitely when it ends
    if loop_enabled {
        loop {
            tracing::info!("Starting game for level {:?}", level);
            match game.run(level, seed).await {
                Ok(_) => {
                    tracing::info!("Game ended successfully, restarting...");
                }
                Err(e) => {
                    tracing::error!("Game failed: {:?}", e);
                    tracing::error!("HALTING: Loop mode stopped due to game failure");
                    return Err(e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    } else {
        game.run(level, seed).await
    }
}

async fn run_goap_game_loop(
    mut game: planners::goap::Game,
    level: Option<i32>,
    seed: Option<i32>,
    loop_enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // If loop is enabled, restart the game indefinitely when it ends
    if loop_enabled {
        loop {
            tracing::info!("Starting game for level {:?}", level);
            match game.run(level, seed).await {
                Ok(_) => {
                    tracing::info!("Game ended successfully, restarting...");
                }
                Err(e) => {
                    tracing::error!("Game failed: {:?}", e);
                    tracing::error!("HALTING: Loop mode stopped due to game failure");
                    return Err(e);
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    } else {
        game.run(level, seed).await
    }
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
    let loop_enabled = env::var("SWOQ_LOOP")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let goap_enabled = env::var("SWOQ_GOAP_ENABLED")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let goap_max_depth = env::var("SWOQ_GOAP_MAX_DEPTH")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(10);

    tracing::info!("Visualizer enabled: {}", enable_viz);
    tracing::info!("GOAP enabled: {}", goap_enabled);

    if enable_viz {
        let shared_state: Arc<Mutex<Option<GameStateSnapshot>>> = Arc::new(Mutex::new(None));
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
                let composite = CompositeObserver::new(vec![
                    Box::new(DefaultObserver::default()),
                    Box::new(VisualizingObserver::new(game_state, log_tx)),
                ]);

                if goap_enabled {
                    let game = planners::goap::Game::new(connection, composite, goap_max_depth);
                    let _ = run_goap_game_loop(game, level, seed, loop_enabled).await;
                } else {
                    let game = planners::heuristic::Game::new(connection, composite);
                    let _ = run_heuristic_game_loop(game, level, seed, loop_enabled).await;
                }
            });
        });

        run_visualizer(shared_state, ready_tx, log_rx);
    } else {
        let connection = GameConnection::new(user_id, user_name, host, replays_folder).await?;

        if goap_enabled {
            let game =
                planners::goap::Game::new(connection, DefaultObserver::default(), goap_max_depth);
            run_goap_game_loop(game, level, seed, loop_enabled).await?;
        } else {
            let game = planners::heuristic::Game::new(connection, DefaultObserver::default());
            run_heuristic_game_loop(game, level, seed, loop_enabled).await?;
        }
    }

    Ok(())
}
