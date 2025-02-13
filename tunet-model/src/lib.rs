#![forbid(unsafe_code)]

use color_theme::Color as ThemeColor;
use drop_guard::guard;
use futures_util::{pin_mut, TryStreamExt};
use mac_address::*;
use netstatus::*;
use std::borrow::Cow;
use std::net::Ipv4Addr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc::*;
use tunet_helper::{usereg::*, *};
use tunet_suggest as suggest;

pub type UpdateCallback = Arc<dyn Fn(UpdateMsg) + Send + Sync + 'static>;

pub struct Model {
    tx: Sender<Action>,
    pub update: Option<UpdateCallback>,
    pub cred: Arc<NetCredential>,
    pub http: HttpClient,
    pub state: NetState,
    pub status: NetStatus,
    pub accent: ThemeColor,
    pub log: Cow<'static, str>,
    log_busy: BusyBool,
    online_busy: BusyBool,
    detail_busy: BusyBool,
    pub flux: NetFlux,
    pub users: Vec<NetUser>,
    pub details: Vec<NetDetail>,
    pub mac_addrs: Vec<MacAddress>,
    pub del_at_exit: AtomicBool,
}

impl Model {
    pub fn new(tx: Sender<Action>) -> Result<Self> {
        let http = create_http_client()?;

        let mac_addrs = MacAddressIterator::new()
            .map(|it| it.collect::<Vec<_>>())
            .unwrap_or_default();

        Ok(Self {
            update: None,
            tx: tx.clone(),
            cred: Arc::new(NetCredential::default()),
            http,
            state: NetState::Unknown,
            status: NetStatus::current(),
            accent: ThemeColor::accent(),
            log: Cow::default(),
            log_busy: BusyBool::new(tx.clone(), UpdateMsg::LogBusy),
            online_busy: BusyBool::new(tx.clone(), UpdateMsg::OnlineBusy),
            detail_busy: BusyBool::new(tx, UpdateMsg::DetailBusy),
            flux: NetFlux::default(),
            users: Vec::default(),
            details: Vec::default(),
            mac_addrs,
            del_at_exit: AtomicBool::new(false),
        })
    }

    pub fn queue(&self, action: Action) {
        let tx = self.tx.clone();
        tokio::spawn(async move { tx.send(action).await.ok() });
    }

    pub fn handle(&mut self, action: Action) {
        match action {
            Action::Credential(cred) => {
                self.cred = cred;
                self.update(UpdateMsg::Credential);
            }
            Action::UpdateCredential(u, p) => {
                let cred = self.cred.clone();
                let tx = self.tx.clone();
                tokio::spawn(async move {
                    let ac_ids = cred.ac_ids.read().await.clone();
                    tx.send(Action::Credential(Arc::new(NetCredential::new(
                        u, p, ac_ids,
                    ))))
                    .await
                    .ok()
                });
            }
            Action::State(s) => {
                match s {
                    None => {
                        let tx = self.tx.clone();
                        let http = self.http.clone();
                        let status = self.status.clone();
                        tokio::spawn(async move {
                            let state = suggest::suggest_with_status(&http, status).await;
                            tx.send(Action::State(Some(state))).await.ok()
                        });
                    }
                    Some(s) => {
                        self.state = s;
                        self.update(UpdateMsg::State);
                    }
                };
            }
            Action::Timer => {
                self.spawn_timer();
            }
            Action::Tick => {
                if !self.flux.username.is_empty() {
                    self.flux.online_time =
                        Duration(self.flux.online_time.0 + NaiveDuration::seconds(1));
                    self.update(UpdateMsg::Flux);
                }
            }
            Action::Login => {
                self.log = "正在登录".into();
                self.update(UpdateMsg::Log);
                self.spawn_login();
            }
            Action::Logout => {
                self.log = "正在注销".into();
                self.update(UpdateMsg::Log);
                self.spawn_logout();
            }
            Action::Flux => {
                self.spawn_flux();
            }
            Action::LoginDone(s) | Action::LogoutDone(s) => {
                self.log = s.into();
                self.update(UpdateMsg::Log);
            }
            Action::FluxDone(f, s, keep) => {
                if keep {
                    if let Some(s) = s {
                        self.log = s.into();
                        self.update(UpdateMsg::Log);
                    }
                } else {
                    self.log = s.unwrap_or_default().into();
                    self.update(UpdateMsg::Log);
                }
                self.flux = f;
                self.update(UpdateMsg::Flux);
            }
            Action::Online => {
                self.spawn_online();
            }
            Action::OnlineDone(us) => {
                self.users = us;
                self.update(UpdateMsg::Online);
            }
            Action::Connect(addr) => {
                let tx = self.tx.clone();
                let usereg = self.usereg();
                tokio::spawn(async move {
                    usereg.login().await?;
                    usereg.connect(addr).await?;
                    tx.send(Action::Online).await?;
                    Ok::<_, anyhow::Error>(())
                });
            }
            Action::Drop(addr) => {
                let tx = self.tx.clone();
                let usereg = self.usereg();
                tokio::spawn(async move {
                    usereg.login().await?;
                    usereg.drop(addr).await?;
                    tx.send(Action::Online).await?;
                    Ok::<_, anyhow::Error>(())
                });
            }
            Action::Details => {
                self.spawn_details();
            }
            Action::DetailsDone(ds) => {
                self.details = ds;
                self.update(UpdateMsg::Details);
            }
            Action::Update(msg) => {
                self.update(msg);
            }
        }
    }

    pub fn update(&self, msg: UpdateMsg) {
        if let Some(f) = &self.update {
            let f = f.clone();
            tokio::task::spawn_blocking(move || f(msg));
        }
    }

    fn spawn_timer(&self) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            loop {
                interval.tick().await;
                tx.send(Action::Tick).await?;
            }
            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        });
    }

    fn client(&self) -> Option<TUNetConnect> {
        TUNetConnect::new(self.state, self.cred.clone(), self.http.clone()).ok()
    }

    fn usereg(&self) -> UseregHelper {
        UseregHelper::new(self.cred.clone(), self.http.clone())
    }

    fn spawn_login(&self) {
        if let Some(lock) = self.log_busy.lock() {
            let tx = self.tx.clone();
            if let Some(client) = self.client() {
                tokio::spawn(async move {
                    let _lock = lock;
                    let res = client.login().await;
                    let ok = res.is_ok();
                    tx.send(Action::LoginDone(res.unwrap_or_else(|e| e.to_string())))
                        .await?;
                    if ok {
                        Self::flux_impl(client, tx, true).await?;
                    }
                    Ok::<_, anyhow::Error>(())
                });
            }
        }
    }

    fn spawn_logout(&self) {
        if let Some(lock) = self.log_busy.lock() {
            let tx = self.tx.clone();
            if let Some(client) = self.client() {
                tokio::spawn(async move {
                    let _lock = lock;
                    let res = client.logout().await;
                    let ok = res.is_ok();
                    tx.send(Action::LoginDone(res.unwrap_or_else(|e| e.to_string())))
                        .await?;
                    if ok {
                        Self::flux_impl(client, tx, true).await?;
                    }
                    Ok::<_, anyhow::Error>(())
                });
            }
        }
    }

    fn spawn_flux(&self) {
        if let Some(lock) = self.log_busy.lock() {
            let tx = self.tx.clone();
            if let Some(client) = self.client() {
                tokio::spawn(async move {
                    let _lock = lock;
                    Self::flux_impl(client, tx, false).await
                });
            }
        }
    }

    async fn flux_impl(client: TUNetConnect, tx: Sender<Action>, keep_msg: bool) -> Result<()> {
        let flux = client.flux().await;
        match flux {
            Ok(flux) => {
                tx.send(Action::FluxDone(flux, None, keep_msg)).await?;
            }
            Err(err) => {
                tx.send(Action::FluxDone(
                    NetFlux::default(),
                    Some(err.to_string()),
                    keep_msg,
                ))
                .await?
            }
        }
        Ok(())
    }

    fn spawn_online(&self) {
        if let Some(lock) = self.online_busy.lock() {
            let tx = self.tx.clone();
            let usereg = self.usereg();
            tokio::spawn(async move {
                let _lock = lock;
                usereg.login().await?;
                let users = usereg.users();
                pin_mut!(users);
                tx.send(Action::OnlineDone(users.try_collect().await?))
                    .await?;
                Ok::<_, anyhow::Error>(())
            });
        }
    }

    fn spawn_details(&self) {
        if let Some(lock) = self.detail_busy.lock() {
            let tx = self.tx.clone();
            let usereg = self.usereg();
            tokio::spawn(async move {
                let _lock = lock;
                usereg.login().await?;
                let details = usereg.details(NetDetailOrder::LogoutTime, false);
                pin_mut!(details);
                tx.send(Action::DetailsDone(details.try_collect().await?))
                    .await?;
                Ok::<_, anyhow::Error>(())
            });
        }
    }

    pub fn log_busy(&self) -> bool {
        self.log_busy.get()
    }

    pub fn online_busy(&self) -> bool {
        self.online_busy.get()
    }

    pub fn detail_busy(&self) -> bool {
        self.detail_busy.get()
    }

    pub fn set_del_at_exit(&self, v: bool) {
        self.del_at_exit.store(v, Ordering::Release);
    }

    pub fn del_at_exit(&self) -> bool {
        self.del_at_exit.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
pub enum Action {
    Credential(Arc<NetCredential>),
    UpdateCredential(String, String),
    State(Option<NetState>),
    Timer,
    Tick,
    Login,
    LoginDone(String),
    Logout,
    LogoutDone(String),
    Flux,
    FluxDone(NetFlux, Option<String>, bool),
    Online,
    OnlineDone(Vec<NetUser>),
    Connect(Ipv4Addr),
    Drop(Ipv4Addr),
    Details,
    DetailsDone(Vec<NetDetail>),
    Update(UpdateMsg),
}

#[repr(i32)]
#[derive(Debug, Clone, Copy)]
pub enum UpdateMsg {
    Credential,
    State,
    Log,
    Flux,
    Online,
    Details,
    LogBusy,
    OnlineBusy,
    DetailBusy,
}

struct BusyBool {
    lock: Arc<AtomicBool>,
    tx: Sender<Action>,
    msg: UpdateMsg,
}

impl BusyBool {
    pub fn new(tx: Sender<Action>, msg: UpdateMsg) -> Self {
        Self {
            lock: Arc::new(AtomicBool::new(false)),
            tx,
            msg,
        }
    }

    pub fn get(&self) -> bool {
        self.lock.load(Ordering::Acquire)
    }

    pub fn lock(&self) -> Option<impl Drop> {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            let msg = self.msg;
            let tx = self.tx.clone();
            tokio::spawn(async move {
                tx.send(Action::Update(msg)).await.ok();
            });
            Some(guard(
                (self.lock.clone(), self.tx.clone(), self.msg),
                |(lock, tx, msg)| {
                    lock.store(false, Ordering::Release);
                    tokio::spawn(async move {
                        tx.send(Action::Update(msg)).await.ok();
                    });
                },
            ))
        } else {
            None
        }
    }
}
