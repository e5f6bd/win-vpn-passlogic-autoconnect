use std::{path::PathBuf, str::FromStr};

use anyhow::bail;
use clap::Parser;
use deranged::RangedU8;
use itertools::Itertools;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use url::Url;

fn main() -> anyhow::Result<()> {
    let args = Opts::parse();
    let config: Config = toml::from_str(&fs_err::read_to_string(args.config_path)?)?;

    println!("{config:?}");
    Ok(())
}

#[derive(Parser)]
struct Opts {
    config_path: PathBuf,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Config {
    matrix_url: Url,
    vpn_name: String,
    username: String,
    #[serde_as(as = "DisplayFromStr")]
    password: Password,
}

#[derive(Debug)]
struct Password {
    matrix_entries: [MatrixEntry; 8],
    suffix: String,
}

#[derive(Debug)]
struct MatrixEntry {
    table: RangedU8<0, 3>,
    position: RangedU8<0, 16>,
}

impl FromStr for Password {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if !s.is_char_boundary(16) {
            anyhow::bail!("Invalid format");
        }
        let (s, t) = s.split_at(16);
        let matrix_entries = s
            .chars()
            .tuples()
            .map(|(x, y)| {
                let parse = |c: char, radix| match c.to_digit(radix) {
                    Some(x) => Ok(x as u8),
                    None => bail!("Failed to parse character {c:?} base {radix}"),
                };
                anyhow::Ok(MatrixEntry {
                    table: parse(x, 10)?.try_into()?,
                    position: parse(y, 16)?.try_into()?,
                })
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .unwrap();
        let suffix = t.to_owned();
        Ok(Self {
            matrix_entries,
            suffix,
        })
    }
}
