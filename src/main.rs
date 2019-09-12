use arc_libclient::{for_balance_udpate, Wallet};
use futures::Future;
use std::thread;

#[macro_use]
extern crate log;

use rustyline::{error::ReadlineError, Editor};

use std::io::prelude::*;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "arc_cliwallet", about = "A CLI wallet for ensicoin")]
struct Config {
    #[structopt(
        long,
        short,
        help = "prompt history file, defaults to $HOME/.arc_cliwallet_history",
        parse(from_os_str)
    )]
    pub history: Option<PathBuf>,
    #[structopt(
        long,
        short,
        help = "wallet storage, defaults to $HOME/.wallet.ron",
        parse(from_os_str)
    )]
    pub storage: Option<PathBuf>,
    #[structopt(
        long,
        short,
        help = "key to decode the wallet, can be suplied by stdin"
    )]
    pub key: Option<String>,
}

fn main() {
    let config = Config::from_args();
    let storage = match config.storage {
        Some(s) => s,
        None => {
            let mut s = dirs::home_dir().expect("Home dir");
            s.push(".wallet.ron");
            s
        }
    };
    let wallet = if storage.exists() {
        let key = match config.key {
            Some(k) => k,
            None => {
                let mut k = String::new();
                std::io::stdin()
                    .read_to_string(&mut k)
                    .expect("Key input error");
                k
            }
        };
        let key = match base64::decode(&key) {
            Ok(k) => k,
            Err(e) => {
                error!("Key is in invalid format: {}", e);
                return;
            }
        };
        unimplemented!()
    } else {
        let (wallet, key) = Wallet::with_random_key(storage).expect("Wallet creation");
        println!("Auth key: {}", base64::encode(key.as_ref()));
        println!("Save it to be able to access your wallet");
        wallet
    };
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
