use arc_libclient::{for_balance_udpate, Wallet};
use futures::Future;

#[macro_use]
extern crate log;

use rustyline::{error::ReadlineError, Editor};

mod prompt;
use prompt::{Command, CommandError};

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "arc_cliwallet", about = "A CLI wallet for ensicoin")]
struct Config {
    #[structopt(long)]
    pub debug: bool,
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
    simplelog::TermLogger::init(
        if config.debug {
            simplelog::LevelFilter::Debug
        } else {
            simplelog::LevelFilter::Info
        },
        simplelog::Config::default(),
        simplelog::TerminalMode::Mixed,
    )
    .unwrap();
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

fn spawn_a_future<F: Future<Item = (), Error = ()> + Send + 'static>(
    future: F,
    name: Option<String>,
) -> Result<(), std::io::Error> {
    std::thread::Builder::new()
        .name(match name {
            Some(n) => format!("{} runner", n),
            None => "Future runner".to_owned(),
        })
        .spawn(move || {
            let mut runtime = tokio::runtime::current_thread::Runtime::new().unwrap();
            let handle = runtime.handle();
            std::thread::spawn(move || {
                handle.spawn(future).expect("Spawning on handle failed");
            })
            .join()
            .expect("Runner thread failed");

            runtime.run().expect("Runner runtime failed");
        })
        .map(|_| ())
}

fn run(wallet: arc_libclient::Data, address: http::Uri, key: Vec<u8>, storage: std::path::PathBuf) {
    println!("Pub key: {}", wallet.read().pub_key);
    let save_wallet = wallet.clone();
    let runner = for_balance_udpate(address.clone(), wallet.clone(), move |balance| {
        debug!("Balance update: {}", balance);
        save_wallet
            .read()
            .save(storage.clone(), &key)
            .expect("Could not save wallet");
        Ok(())
    });

    spawn_a_future(
        runner.map_err(|e| eprintln!("Fatal error: {:?}", e)),
        Some("balance update".to_owned()),
    )
    .expect("spawning balance updater failed");

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
                match line.parse() {
                    Ok(Command::Balance) => println!("Balance: {}", wallet.read().balance()),
                    Ok(Command::Pubkey) => println!("Public key: {}", wallet.read().pub_key),
                    Ok(Command::Help) => {
                        println!("\thelp: prints this help");
                        println!("\tbalance: prints the balance");
                        println!("\tpubkey: prints the public key");
                        println!("\tpay <value> <public_key>:pay value XEC to the public_key");
                    }
                    Ok(Command::Pay { value, to }) => {
                        spawn_a_future(
                            arc_libclient::pay_to(address.clone(), wallet.clone(), value, &to)
                                .expect("creating payment")
                                .map_err(|e| error!("Error in payment: {:?}", e))
                                .map(|_| ()),
                            Some(format!("payment to {}", to)),
                        )
                        .expect("Payment thread");
                    }
                    Err(CommandError::NoCommand) => (),
                    Err(CommandError::ArgumentCount { expected: n }) => {
                        eprintln!("Expected {} arguments", n)
                    }
                    Err(CommandError::InvalidArgument { message }) => {
                        eprintln!("Invalid argument: {}", message)
                    }
                    Err(CommandError::UnknownCommand) => eprintln!("Unknown command"),
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
