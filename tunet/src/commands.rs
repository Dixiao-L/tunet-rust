use async_trait::async_trait;
use clap::Parser;
use enum_dispatch::enum_dispatch;
use futures_util::{pin_mut, stream::TryStreamExt};
use itertools::Itertools;
use mac_address::MacAddressIterator;
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::{cmp::Reverse, ffi::OsString};
use termcolor::{Color, ColorChoice, StandardStream};
use termcolor_output as tco;
use tunet_helper::{usereg::*, *};
use tunet_settings_cli::*;
use tunet_suggest::TUNetHelperExt;

fn get_flux_color(f: &Flux, total: bool) -> Color {
    let flux = f.0;
    if flux == 0 {
        Color::Cyan
    } else if flux < if total { 20_000_000_000 } else { 2_000_000_000 } {
        Color::Yellow
    } else {
        Color::Magenta
    }
}

#[async_trait]
#[enum_dispatch(TUNet)]
pub trait TUNetCommand {
    async fn run(&self) -> Result<()>;
}

#[enum_dispatch]
#[derive(Debug, Parser)]
#[clap(about, version, author)]
pub enum TUNet {
    #[clap(name = "login", about = "登录")]
    Login,
    #[clap(name = "logout", about = "注销")]
    Logout,
    #[clap(name = "status", about = "查看在线状态")]
    Status,
    #[clap(name = "online", about = "查询在线IP")]
    Online,
    #[clap(name = "connect", about = "上线IP")]
    UseregConnect,
    #[clap(name = "drop", about = "下线IP")]
    UseregDrop,
    #[clap(name = "detail", about = "流量明细")]
    Detail,
    #[clap(name = "deletecred", about = "删除用户名和密码")]
    DeleteCred,
    #[clap(name = "cui", about = "启动命令行界面")]
    Cui,
    #[clap(name = "gui", about = "启动图形界面")]
    Gui,
}

#[derive(Debug, Parser)]
pub struct Login {
    #[clap(long, short = 's')]
    /// 连接方式
    host: Option<NetState>,
}

#[async_trait]
impl TUNetCommand for Login {
    async fn run(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_cred()?;
        let c = TUNetConnect::new_with_suggest(self.host, cred, client).await?;
        let res = c.login().await?;
        println!("{}", res);
        save_cred(c.cred()).await
    }
}

#[derive(Debug, Parser)]
pub struct Logout {
    #[clap(long, short = 's')]
    /// 连接方式
    host: Option<NetState>,
}

#[async_trait]
impl TUNetCommand for Logout {
    async fn run(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_username()?;
        let c = TUNetConnect::new_with_suggest(self.host, cred, client).await?;
        let res = c.logout().await?;
        println!("{}", res);
        Ok(())
    }
}
#[derive(Debug, Parser)]
pub struct Status {
    #[clap(long, short = 's')]
    /// 连接方式
    host: Option<NetState>,
}

#[async_trait]
impl TUNetCommand for Status {
    async fn run(&self) -> Result<()> {
        let client = create_http_client()?;
        let c =
            TUNetConnect::new_with_suggest(self.host, Arc::new(NetCredential::default()), client)
                .await?;
        let f = c.flux().await?;
        let stdout = StandardStream::stdout(ColorChoice::Auto);
        let mut stdout = tco::ResetGuard::Owned(stdout);
        tco::writeln!(
            stdout,
            "{}用户 {}{}",
            fg!(Some(Color::Cyan)),
            reset!(),
            f.username
        )?;
        tco::writeln!(
            stdout,
            "{}流量 {}{}{}",
            fg!(Some(Color::Cyan)),
            fg!(Some(get_flux_color(&f.flux, true))),
            bold!(true),
            f.flux
        )?;
        tco::writeln!(
            stdout,
            "{}时长 {}{}",
            fg!(Some(Color::Cyan)),
            fg!(Some(Color::Green)),
            f.online_time
        )?;
        tco::writeln!(
            stdout,
            "{}余额 {}{}",
            fg!(Some(Color::Cyan)),
            fg!(Some(Color::Yellow)),
            f.balance
        )?;
        Ok(())
    }
}

#[derive(Debug, Parser)]
pub struct Online {}

#[async_trait]
impl TUNetCommand for Online {
    async fn run(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_cred()?;
        let c = UseregHelper::new(cred, client);
        c.login().await?;
        let us = c.users();
        let mac_addrs = MacAddressIterator::new()
            .map(|it| it.collect::<Vec<_>>())
            .unwrap_or_default();
        let stdout = StandardStream::stdout(ColorChoice::Auto);
        let mut stdout = tco::ResetGuard::Owned(stdout);
        tco::writeln!(
            stdout,
            "    IP地址            登录时间         流量        MAC地址"
        )?;

        pin_mut!(us);
        while let Some(u) = us.try_next().await? {
            let is_self = mac_addrs
                .iter()
                .any(|it| Some(it) == u.mac_address.as_ref());
            tco::writeln!(
                stdout,
                "{}{:15} {}{:20} {}{:>8} {}{} {}{}",
                fg!(Some(Color::Yellow)),
                u.address,
                fg!(Some(Color::Green)),
                u.login_time,
                fg!(Some(get_flux_color(&u.flux, true))),
                u.flux,
                fg!(Some(Color::Cyan)),
                u.mac_address.map(|a| a.to_string()).unwrap_or_default(),
                fg!(Some(Color::Magenta)),
                if is_self { "本机" } else { "" }
            )?;
        }
        save_cred(c.cred()).await
    }
}

#[derive(Debug, Parser)]
pub struct UseregConnect {
    #[clap(long, short)]
    /// IP地址
    address: Ipv4Addr,
}

#[async_trait]
impl TUNetCommand for UseregConnect {
    async fn run(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_cred()?;
        let c = UseregHelper::new(cred, client);
        c.login().await?;
        let res = c.connect(self.address).await?;
        println!("{}", res);
        save_cred(c.cred()).await
    }
}

#[derive(Debug, Parser)]
pub struct UseregDrop {
    #[clap(long, short)]
    /// IP地址
    address: Ipv4Addr,
}

#[async_trait]
impl TUNetCommand for UseregDrop {
    async fn run(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_cred()?;
        let c = UseregHelper::new(cred, client);
        c.login().await?;
        let res = c.drop(self.address).await?;
        println!("{}", res);
        save_cred(c.cred()).await
    }
}

#[derive(Debug, Parser)]
pub struct Detail {
    #[clap(long, short, default_value = "logout")]
    /// 排序方式
    order: NetDetailOrder,
    #[clap(long, short)]
    /// 倒序
    descending: bool,
    #[clap(long, short)]
    /// 按日期分组
    grouping: bool,
}

impl Detail {
    async fn run_detail(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_cred()?;
        let c = UseregHelper::new(cred, client);
        c.login().await?;
        let details = c.details(self.order, self.descending);
        let stdout = StandardStream::stdout(ColorChoice::Auto);
        let mut stdout = tco::ResetGuard::Owned(stdout);
        tco::writeln!(stdout, "      登录时间             注销时间         流量")?;
        let mut total_flux = Flux(0);

        pin_mut!(details);
        while let Some(d) = details.try_next().await? {
            tco::writeln!(
                stdout,
                "{}{:20} {:20} {}{:>8}",
                fg!(Some(Color::Green)),
                d.login_time,
                d.logout_time,
                fg!(Some(get_flux_color(&d.flux, false))),
                d.flux
            )?;
            total_flux.0 += d.flux.0;
        }
        tco::writeln!(
            stdout,
            "{}总流量 {}{}{}",
            fg!(Some(Color::Cyan)),
            fg!(Some(get_flux_color(&total_flux, true))),
            bold!(true),
            total_flux
        )?;
        save_cred(c.cred()).await
    }

    async fn run_detail_grouping(&self) -> Result<()> {
        let client = create_http_client()?;
        let cred = read_cred()?;
        let c = UseregHelper::new(cred, client);
        c.login().await?;
        let details = c
            .details(NetDetailOrder::LogoutTime, self.descending)
            .try_collect::<Vec<_>>()
            .await?;
        let mut details = details
            .into_iter()
            .group_by(|detail| detail.logout_time.date())
            .into_iter()
            .map(|(key, group)| (key, Flux(group.map(|detail| detail.flux.0).sum::<u64>())))
            .collect::<Vec<_>>();
        match self.order {
            NetDetailOrder::Flux => {
                if self.descending {
                    details.sort_unstable_by_key(|(_, flux)| Reverse(*flux));
                } else {
                    details.sort_unstable_by_key(|(_, flux)| *flux);
                }
            }
            _ => {
                if self.descending {
                    details.sort_unstable_by_key(|(date, _)| Reverse(date.day()));
                }
            }
        }
        let stdout = StandardStream::stdout(ColorChoice::Auto);
        let mut stdout = tco::ResetGuard::Owned(stdout);
        tco::writeln!(stdout, " 登录日期    流量")?;
        let mut total_flux = Flux(0);
        for (date, flux) in details {
            tco::writeln!(
                stdout,
                "{}{:10} {}{:>8}",
                fg!(Some(Color::Green)),
                date,
                fg!(Some(get_flux_color(&flux, true))),
                flux
            )?;
            total_flux.0 += flux.0;
        }
        tco::writeln!(
            stdout,
            "{}总流量 {}{}{}",
            fg!(Some(Color::Cyan)),
            fg!(Some(get_flux_color(&total_flux, true))),
            bold!(true),
            total_flux
        )?;
        save_cred(c.cred()).await
    }
}

#[async_trait]
impl TUNetCommand for Detail {
    async fn run(&self) -> Result<()> {
        if self.grouping {
            self.run_detail_grouping().await
        } else {
            self.run_detail().await
        }
    }
}

#[derive(Debug, Parser)]
pub struct DeleteCred {}

#[async_trait]
impl TUNetCommand for DeleteCred {
    async fn run(&self) -> Result<()> {
        delete_cred()
    }
}

async fn run_external(command: &str, args: &[OsString]) -> Result<()> {
    let self_path = std::env::current_exe()?;
    let command = format!("tunet-{}", command);
    let command = if let Some(ext_path) = self_path.parent() {
        let mut ext_path = ext_path.to_path_buf();
        ext_path.push(command);
        ext_path
    } else {
        command.into()
    };
    subprocess::Exec::cmd(command).args(args).join()?;
    Ok(())
}

#[derive(Debug, Parser)]
pub struct Cui {
    #[clap(multiple_values = true)]
    ext_cmd: Vec<OsString>,
}

#[async_trait]
impl TUNetCommand for Cui {
    async fn run(&self) -> Result<()> {
        run_external("cui", &self.ext_cmd).await
    }
}

#[derive(Debug, Parser)]
pub struct Gui {
    #[clap(multiple_values = true)]
    ext_cmd: Vec<OsString>,
}

#[async_trait]
impl TUNetCommand for Gui {
    async fn run(&self) -> Result<()> {
        run_external("gui", &self.ext_cmd).await
    }
}
