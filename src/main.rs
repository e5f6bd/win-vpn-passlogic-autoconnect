use std::{path::PathBuf, process::Command, str::FromStr};

use anyhow::{bail, Context};
use clap::Parser;
use deranged::RangedU8;
use dirs::config_dir;
use itertools::Itertools;
use lazy_format::lazy_format;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use url::Url;

fn main() -> anyhow::Result<()> {
    let args = Opts::parse();
    let config_path = (args.config_path)
        .or_else(|| Some(config_dir()?.join("wvpa.toml")))
        .context("Please specify config_path.")?;
    let config: Config = toml::from_str(&fs_err::read_to_string(config_path)?)?;
    let matrix = Matrix::fetch(config.matrix_url)?;
    let password = config.password.generate(matrix);
    if args.just_print_password {
        println!("{password}");
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    let exit_status = Command::new("scutil")
        .args(["--nc", "start", &config.vpn_name])
        .args(["--password", &password])
        .args(["--secret", &config.secret])
        .status()?;
    #[cfg(not(target_os = "macos"))]
    let exit_status = Command::new("rasdial.exe")
        .args([config.vpn_name, config.username, password])
        .status()?;
    if !exit_status.success() {
        bail!("Process terminated with exit code {exit_status}");
    }
    Ok(())
}

#[derive(Parser)]
struct Opts {
    config_path: Option<PathBuf>,
    #[clap(short = 'p', long)]
    just_print_password: bool,
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Config {
    matrix_url: Url,
    vpn_name: String,
    #[cfg(not(target_os = "macos"))]
    username: String,
    #[serde_as(as = "DisplayFromStr")]
    password: Password,
    #[cfg(target_os = "macos")]
    secret: String,
}

#[derive(Debug)]
struct Password {
    matrix_entries: [MatrixPosition; 8],
    suffix: String,
}

#[derive(Clone, Copy, Debug)]
struct MatrixPosition {
    table: RangedU8<0, 3>,
    position: RangedU8<0, 16>,
}

type MatrixElement = RangedU8<0, 10>;
#[derive(Debug)]
struct Matrix([[MatrixElement; 16]; 3]);
impl Matrix {
    fn fetch(url: Url) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::new();
        let html = Html::parse_document(&client.get(url).send()?.text()?);
        Self::parse(&html)
    }

    fn parse(html: &Html) -> anyhow::Result<Self> {
        let selector_table = Selector::parse("table.randamNumbarWidth").unwrap();
        let selector_p = Selector::parse("p").unwrap();
        Ok(Self(
            html.select(&selector_table)
                .map(|table| {
                    table
                        .select(&selector_p)
                        .map(|e| e.text().collect::<String>().parse::<MatrixElement>())
                        .collect::<Result<Vec<_>, _>>()?
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("The table do not have exactly 16 elements"))
                })
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| anyhow::anyhow!("There are not exactly three tables"))?,
        ))
    }

    fn get(&self, key: MatrixPosition) -> MatrixElement {
        self.0[key.table.get() as usize][key.position.get() as usize]
    }
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
                anyhow::Ok(MatrixPosition {
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

impl Password {
    fn generate(&self, matrix: Matrix) -> String {
        let prefix = lazy_format!(("{}", matrix.get(key)) for key in self.matrix_entries);
        format!("{prefix}{}", self.suffix)
    }
}
