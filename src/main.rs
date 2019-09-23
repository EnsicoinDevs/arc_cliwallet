use arc_libclient::{for_balance_udpate, Wallet};
use futures::Future;

#[macro_use]
extern crate log;

use rustyline::{error::ReadlineError, Editor};

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
    #[structopt(
        long,
        short,
        help = "gRPC address of a node",
        default_value = "http://localhost:4225"
    )]
    pub address: http::Uri,
}

fn main() {
    let config = Config::from_args();
    let storage = match config.storage {
        Some(s) => s,
        None => {
            let mut s = dirs::home_dir().expect("Home dir");
            s.push(".wallet.json");
            s
        }
    };
    if storage.exists() {
        println!("Loading wallet from file !");
        let key = match config.key {
            Some(k) => k,
            None => rpassword::read_password_from_tty(Some("Decryption key: "))
                .expect("Error inputing password"),
        };
        let key = match base64::decode(&key) {
            Ok(k) => k,
            Err(e) => {
                error!("Key is in invalid format: {}", e);
                return;
            }
        };
        run(
            Wallet::open(storage.clone(), &key).expect("Wallet loading"),
            config.address,
            key,
            storage,
        );
    } else {
        println!("Creating new wallet");
        let address = config.address;
        tokio::run(
            Wallet::with_random_key(storage.clone(), address.clone())
                .expect("Wallet creation")
                .and_then(|(wallet, key)| {
                    println!("Auth key: {}", base64::encode(&key));
                    println!("Save it to be able to access your wallet");
                    Ok(run(wallet, address, key, storage))
                })
                .map_err(|e| error!("Error running wallet: {:?}", e)),
        );
    };
}

fn run(wallet: arc_libclient::Data, address: http::Uri, key: Vec<u8>, storage: std::path::PathBuf) {
    println!("Pub key: {}", wallet.read().pub_key);
    simplelog::TermLogger::init(
        simplelog::LevelFilter::Info,
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
    )
    .unwrap();
    let save_wallet = wallet.clone();
    let runner = for_balance_udpate(address, wallet.clone(), move |balance| {
        info!("Balance update: {}", balance);
        save_wallet
            .read()
            .save(storage.clone(), &key)
            .expect("Could not save wallet");
        Ok(())
    });

    std::thread::Builder::new()
        .name("Runner".to_owned())
        .spawn(|| {
            let mut runtime = tokio::runtime::current_thread::Runtime::new().unwrap();
            let handle = runtime.handle();
            std::thread::spawn(move || {
                handle
                    .spawn(runner.map_err(|e| eprintln!("Fatal error: {:?}", e)))
                    .expect("Spawning on handle failed");
            })
            .join()
            .expect("Runner thread failed");

            runtime.run().expect("Runner runtime failed");
        })
        .expect("Other tokio failed");

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
