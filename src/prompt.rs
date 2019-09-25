pub enum Command {
    Balance,
    Help,
    Pubkey,
    Pay {
        value: u64,
        to: arc_libclient::secp256k1::PublicKey,
    },
}
#[derive(Debug)]
pub enum CommandError {
    NoCommand,
    UnknownCommand,
    InvalidArgument { message: String },
    ArgumentCount { expected: usize },
}
fn assert_no_more<I: Iterator>(mut iter: I, expected: usize) -> Result<(), CommandError> {
    match iter.next() {
        Some(_) => Err(CommandError::ArgumentCount { expected }),
        None => Ok(()),
    }
}
impl std::str::FromStr for Command {
    type Err = CommandError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut words = s.split_ascii_whitespace();
        match words.next() {
            Some(command) => match command {
                "balance" => {
                    assert_no_more(words, 0)?;
                    Ok(Command::Balance)
                }
                "help" => {
                    assert_no_more(words, 0)?;
                    Ok(Command::Help)
                }
                "pubkey" => {
                    assert_no_more(words, 0)?;
                    Ok(Command::Pubkey)
                }
                "pay" => match (words.next(), words.next()) {
                    (Some(amount), Some(recipient)) => {
                        let value = match amount.parse() {
                            Ok(a) => a,
                            Err(e) => {
                                return Err(CommandError::InvalidArgument {
                                    message: format!("invalid value: {}", e),
                                })
                            }
                        };
                        let to = match recipient.parse() {
                            Ok(r) => r,
                            Err(e) => {
                                return Err(CommandError::InvalidArgument {
                                    message: format!("invalid public key: {}", e),
                                })
                            }
                        };
                        assert_no_more(words, 2)?;
                        Ok(Command::Pay { value, to })
                    }
                    _ => Err(CommandError::ArgumentCount { expected: 2 }),
                },
                _ => Err(CommandError::UnknownCommand),
            },
            None => Err(CommandError::NoCommand),
        }
    }
}
