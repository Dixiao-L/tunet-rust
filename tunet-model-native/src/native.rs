use mac_address::MacAddress;
use std::{
    ffi::c_void,
    sync::{Arc, RwLock},
};
use tunet_helper::{
    usereg::{NetDetail, NetUser},
    NetState,
};
use tunet_model::UpdateMsg;

#[repr(i32)]
pub enum Action {
    Timer,
    Tick,
    Login,
    Logout,
    Flux,
    Online,
    Details,
}

impl From<Action> for tunet_model::Action {
    fn from(a: Action) -> Self {
        match a {
            Action::Timer => Self::Timer,
            Action::Tick => Self::Tick,
            Action::Login => Self::Login,
            Action::Logout => Self::Logout,
            Action::Flux => Self::Flux,
            Action::Online => Self::Online,
            Action::Details => Self::Details,
        }
    }
}

#[repr(i32)]
pub enum State {
    Auto,
    Net,
    Auth4,
    Auth6,
}

impl From<State> for Option<NetState> {
    fn from(s: State) -> Self {
        match s {
            State::Auto => None,
            State::Net => Some(NetState::Net),
            State::Auth4 => Some(NetState::Auth4),
            State::Auth6 => Some(NetState::Auth6),
        }
    }
}

impl From<Option<NetState>> for State {
    fn from(s: Option<NetState>) -> Self {
        match s {
            None => Self::Auto,
            Some(s) => match s {
                NetState::Net => Self::Net,
                NetState::Auth4 => Self::Auth4,
                NetState::Auth6 => Self::Auth6,
                _ => Self::Auto,
            },
        }
    }
}

#[repr(C)]
pub struct OnlineUser {
    pub address: u32,
    pub login_time: i64,
    pub flux: u64,
    pub mac_address: [u8; 6],
    pub has_mac: bool,
    pub is_local: bool,
}

impl OnlineUser {
    pub fn new(u: &NetUser, mac_addrs: &[MacAddress]) -> Self {
        Self {
            address: u.address.into(),
            login_time: u.login_time.timestamp(),
            flux: u.flux.0,
            mac_address: u.mac_address.map(|mac| mac.bytes()).unwrap_or_default(),
            has_mac: u.mac_address.is_some(),
            is_local: mac_addrs
                .iter()
                .any(|it| Some(it) == u.mac_address.as_ref()),
        }
    }
}

#[repr(C)]
pub struct Detail {
    pub login_time: i64,
    pub logout_time: i64,
    pub flux: u64,
}

impl From<&NetDetail> for Detail {
    fn from(d: &NetDetail) -> Self {
        Self {
            login_time: d.login_time.timestamp(),
            logout_time: d.logout_time.timestamp(),
            flux: d.flux.0,
        }
    }
}

#[repr(C)]
pub struct DetailGroup {
    pub logout_date: i64,
    pub flux: u64,
}

#[repr(C)]
pub struct DetailGroupByTime {
    pub logout_start_time: u32,
    pub flux: u64,
}

pub type MainCallback = Option<extern "C" fn(Model, *mut c_void) -> i32>;
pub type UpdateCallback = Option<extern "C" fn(UpdateMsg, *mut c_void)>;
pub type StringCallback = Option<extern "C" fn(*const u16, *mut c_void)>;
pub type OnlinesForeachCallback = Option<extern "C" fn(*const OnlineUser, *mut c_void) -> bool>;
pub type DetailsForeachCallback = Option<extern "C" fn(*const Detail, *mut c_void) -> bool>;
pub type DetailsGroupedForeachCallback =
    Option<extern "C" fn(*const DetailGroup, *mut c_void) -> bool>;
pub type DetailsGroupedByTimeForeachCallback =
    Option<extern "C" fn(*const DetailGroupByTime, *mut c_void) -> bool>;

pub fn wrap_callback(
    func: UpdateCallback,
    data: *mut c_void,
) -> Option<Arc<dyn Fn(UpdateMsg) + Send + Sync + 'static>> {
    struct TempWrapper {
        func: extern "C" fn(UpdateMsg, *mut c_void),
        data: *mut c_void,
    }

    unsafe impl Send for TempWrapper {}
    unsafe impl Sync for TempWrapper {}

    func.map(move |func| {
        let wrapper = TempWrapper { func, data };
        Arc::new(move |m| {
            // The fields seems to be catched separately in edition 2021.
            // Catch the whole wrapper to avoid compile error.
            let w = &wrapper;
            (w.func)(m, w.data);
        }) as _
    })
}

pub type Model = *const RwLock<tunet_model::Model>;
