#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate anyhow;

use crate::config::Config;
use crate::database::DataBase;
use crate::exloli::ExLoli;

use anyhow::Error;
use futures::executor::block_on;
use once_cell::sync::Lazy;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use tokio::time::delay_for;

use std::env;
use std::path::Path;
use std::str::FromStr;
use std::time;

mod bot;
mod config;
mod database;
mod exhentai;
mod exloli;
mod schema;
mod trans;
mod utils;
mod xpath;

static CONFIG: Lazy<Config> = Lazy::new(|| Config::new("config.toml").expect("配置文件解析失败"));
static BOT: Lazy<Bot> = Lazy::new(|| {
    teloxide::BotBuilder::new()
        .token(&CONFIG.telegram.token)
        .parse_mode(ParseMode::HTML)
        .build()
});
static DB: Lazy<DataBase> = Lazy::new(DataBase::init);
static EXLOLI: Lazy<ExLoli> = Lazy::new(|| block_on(ExLoli::new()).expect("登录失败"));

#[tokio::main]
async fn main() {
    pretty_env_logger::formatted_builder()
        .write_style(pretty_env_logger::env_logger::WriteStyle::Auto)
        .filter(Some("teloxide"), log::LevelFilter::Error)
        .filter(
            Some("exloli"),
            log::LevelFilter::from_str(&CONFIG.log_level).expect("LOG 等级设置错误"),
        )
        .init();
    env::set_var("DATABASE_URL", &CONFIG.database_url);

    if let Err(e) = run().await {
        error!("{}", e);
    }
}

async fn run() -> Result<(), Error> {
    let args = env::args().collect::<Vec<_>>();
    let mut opts = getopts::Options::new();
    opts.optflag("", "debug", "调试模式");
    opts.optflag("h", "help", "print this help menu");
    let matches = match opts.parse(&args[1..]) {
        Ok(v) => v,
        Err(e) => panic!("{}", e),
    };

    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", args[0]);
        print!("{}", opts.usage(&brief));
        return Ok(());
    }

    let db_path = env::var("DATABASE_URL").expect("请设置 DATABASE_URL");
    if !Path::new(&db_path).exists() {
        info!("初始化数据库");
        DB.init_database()?;
    }

    let debug = matches.opt_present("debug");

    {
        tokio::spawn(async move { crate::bot::start_bot().await });
    }

    loop {
        if !debug {
            info!("定时更新开始");
            let result = EXLOLI.scan_and_upload().await;
            if let Err(e) = result {
                error!("定时更新出错：{}", e);
            } else {
                info!("定时更新完成");
            }
        }
        info!("休眠中，预计 {} 分钟后开始工作", CONFIG.interval / 60);
        delay_for(time::Duration::from_secs(CONFIG.interval)).await;
    }
}
