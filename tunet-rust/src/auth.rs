use super::*;
use authtea::AuthTea;
use crypto::digest::Digest;
use crypto::hmac::Hmac;
use crypto::mac::Mac;
use crypto::md5::Md5;
use crypto::sha1::Sha1;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::blocking::Client;
use rustc_serialize::hex::ToHex;
use serde_json::{self, Value};
use std::string::String;

const BASE64N: &[u8] = b"LVoJPiCN2R8G90yg+hmFHuacZ1OWMnrsSTXkYpUq/3dlbfKwv6xztjI7DeBE45QA";
const BASE64PAD: u8 = b'=';

fn base64(t: &[u8]) -> String {
    let a = t.len();
    let len = (a + 2) / 3 * 4;
    let mut u = vec![b'\0'; len];
    let mut ui = 0;
    for o in (0..a).step_by(3) {
        let h = ((t[o] as u32) << 16) + (if o + 1 < a { (t[o + 1] as u32) << 8 } else { 0 }) + (if o + 2 < a { t[o + 2] as u32 } else { 0 });
        for i in 0..4 {
            if o * 8 + i * 6 > a * 8 {
                u[ui] = BASE64PAD;
            } else {
                u[ui] = BASE64N[(h >> (6 * (3 - i)) & 0x3F) as usize];
            }
            ui += 1;
        }
    }
    unsafe { String::from_utf8_unchecked(u) }
}

pub struct AuthConnect<'a> {
    credential: NetCredential,
    client: &'a Client,
    ver: i32,
    additional_ac_ids: &'a [i32],
}

const AC_IDS: [i32; 5] = [1, 25, 33, 35, 37];

lazy_static! {
    static ref AC_ID_REGEX: Regex = Regex::new(r"/index_([0-9]+)\.html").unwrap();
}

impl<'a> AuthConnect<'a> {
    pub fn from_cred_client(u: String, p: String, client: &'a Client, ac_ids: &'a [i32]) -> Self {
        Self::from_cred_client_v(u, p, client, 4, ac_ids)
    }

    pub fn from_cred_client_v6(u: String, p: String, client: &'a Client, ac_ids: &'a [i32]) -> Self {
        Self::from_cred_client_v(u, p, client, 6, ac_ids)
    }

    fn from_cred_client_v(u: String, p: String, client: &'a Client, v: i32, ac_ids: &'a [i32]) -> Self {
        AuthConnect { credential: NetCredential::from_cred(u, p), client, ver: v, additional_ac_ids: ac_ids }
    }

    fn challenge(&self) -> Result<String> {
        let res = self.client.get(&format!("https://auth{0}.tsinghua.edu.cn/cgi-bin/get_challenge?username={1}&double_stack=1&ip&callback=callback", self.ver, self.credential.username)).send()?;
        let t = res.text()?;
        let json: Value = serde_json::from_str(&t[9..t.len() - 1])?;
        match &json["challenge"] {
            Value::String(s) => Ok(s.to_string()),
            _ => Ok(String::new()),
        }
    }

    fn get_ac_id(&self) -> Result<i32> {
        let res = self.client.get(if self.ver == 4 { "http://3.3.3.3/" } else { "http://[333::3]/" }).send()?;
        let t = res.text()?;
        match AC_ID_REGEX.captures(&t) {
            Some(cap) => Ok(cap[1].parse::<i32>().unwrap()),
            _ => Err(NetHelperError::NoAcIdErr),
        }
    }

    fn do_log<F>(&self, action: F) -> Result<String>
    where
        F: Fn(i32) -> Result<String>,
    {
        for ac_id in &AC_IDS {
            let res = action(*ac_id);
            if res.is_ok() {
                return res;
            }
        }
        for ac_id in self.additional_ac_ids {
            let res = action(*ac_id);
            if res.is_ok() {
                return res;
            }
        }
        let ac_id = self.get_ac_id()?;
        return action(ac_id);
    }
}

fn parse_response(t: &str) -> Result<(bool, String)> {
    let json: Value = serde_json::from_str(&t[9..t.len() - 1])?;
    if let Value::String(error) = &json["error"] {
        if let Value::String(error_msg) = &json["error_msg"] {
            return Ok((error == "ok", format!("error: {}; error_msg: {}", error, error_msg)));
        }
    }
    Ok((false, String::new()))
}

impl<'a> NetHelper for AuthConnect<'a> {
    fn login(&self) -> Result<String> {
        self.do_log(|ac_id| {
            let token = self.challenge()?;
            let mut md5 = Md5::new();
            md5.input_str(&token);
            let mut hmac = Hmac::new(md5, &[]);
            let password_md5 = hmac.result().code().to_hex();
            let p_mmd5 = "{MD5}".to_owned() + &password_md5;
            let encode_json = serde_json::json!({
                "username":self.credential.username,
                "password":self.credential.password,
                "ip":"",
                "acid":ac_id,
                "enc_ver":"srun_bx1"
            });
            let tea = AuthTea::new(token.as_bytes());
            let info = "{SRBX1}".to_owned() + &base64(&tea.encrypt_str(&encode_json.to_string()));
            let mut sha1 = Sha1::new();
            sha1.input_str(&format!("{0}{1}{0}{2}{0}{4}{0}{0}200{0}1{0}{3}", token, self.credential.username, password_md5, info, ac_id));
            let params = [("action", "login"), ("ac_id", &ac_id.to_string()), ("double_stack", "1"), ("n", "200"), ("type", "1"), ("username", &self.credential.username), ("password", &p_mmd5), ("info", &info), ("chksum", &sha1.result_str()), ("callback", "callback")];
            let res = self.client.post(&format!("https://auth{0}.tsinghua.edu.cn/cgi-bin/srun_portal", self.ver)).form(&params).send()?;
            let t = res.text()?;
            let (suc, msg) = parse_response(&t)?;
            if suc {
                Ok(msg)
            } else {
                Err(NetHelperError::LogErr(msg))
            }
        })
    }

    fn logout(&self) -> Result<String> {
        self.do_log(|ac_id| {
            let token = self.challenge()?;
            let encode_json = serde_json::json!({
                "username":self.credential.username,
                "ip":"",
                "acid":ac_id,
                "enc_ver":"srun_bx1"
            });
            let tea = AuthTea::new(token.as_bytes());
            let info = "{SRBX1}".to_owned() + &base64(&tea.encrypt_str(&encode_json.to_string()));
            let mut sha1 = Sha1::new();
            sha1.input_str(&format!("{0}{1}{0}{3}{0}{0}200{0}1{0}{2}", token, self.credential.username, info, ac_id));
            let params = [("action", "logout"), ("ac_id", &ac_id.to_string()), ("double_stack", "1"), ("n", "200"), ("type", "1"), ("username", &self.credential.username), ("info", &info), ("chksum", &sha1.result_str()), ("callback", "callback")];
            let res = self.client.post(&format!("https://auth{0}.tsinghua.edu.cn/cgi-bin/srun_portal", self.ver)).form(&params).send()?;
            let t = res.text()?;
            let (suc, msg) = parse_response(&t)?;
            if suc {
                Ok(msg)
            } else {
                Err(NetHelperError::LogErr(msg))
            }
        })
    }
}

impl<'a> NetConnectHelper for AuthConnect<'a> {
    fn flux(&self) -> Result<NetFlux> {
        let res = self.client.get(&format!("https://auth{0}.tsinghua.edu.cn/rad_user_info.php", self.ver)).send()?;
        Ok(NetFlux::from_str(&res.text()?))
    }
}
