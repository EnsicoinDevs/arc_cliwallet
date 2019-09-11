use arc_libclient::{for_balance_udpate, Wallet};
use futures::Future;
use std::thread;

#[macro_use]
extern crate log;

use rustyline::{error::ReadlineError, Editor};

fn main() {
    let (wallet, key) = Wallet::with_random_key("wallet.ron").expect("Wallet creation");
    println!("Auth key: {}", base64::encode(key.as_ref()));
    println!("Pub key: {}", wallet.read().pub_key);
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
    )
    .unwrap();
    let runner = for_balance_udpate(
        "http://localhost:4225".parse().unwrap(),
        wallet.clone(),
        |balance| {
            debug!("Balance update: {}", balance);
            Ok(())
        },
    );

    thread::Builder::new()
        .name("Updater".to_owned())
        .spawn(|| tokio::run(runner.map_err(|e| eprintln!("Fatal error: {:?}", e))))
        .expect("Balance updater failed");

    let mut rl = Editor::<()>::new();
    if let Some(mut home) = dirs::home_dir() {
        home.push(".arc_cliwallet_history");
        if rl.load_history(&home).is_err() {
            debug!("Creating history file");
        }
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                let line = line.trim();
                match line {
                    "balance" => println!("Balance: {}", wallet.read().balance()),
                    _ => eprintln!("Unknown command"),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("Bye !");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }
    if let Some(mut home) = dirs::home_dir() {
        home.push(".arc_cliwallet_history");
        if let Err(e) = rl.save_history(&home) {
            eprintln!("Could not save history: {}", e)
        }
    }
}
