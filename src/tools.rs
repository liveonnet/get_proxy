use std::error::Error;
use std::sync::Arc;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use std::path::PathBuf;
use std::io::Read;
use std::fs::File;
use serde_json::Value;
use serde_json::map::Entry;
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use log::{debug, trace, warn, error};
use reqwest::Client;
use tokio::sync::Notify;
use tokio::time::timeout;
#[allow(unused_imports)]
use tokio::process::Command;
// #[allow(unused_imports)]
// use toml::Value as tvalue;
use regex::Regex;
use crate::node::Node;

#[allow(unused_macros)]
macro_rules! assert_types {
    ($($var:ident : $ty:ty),*) => { $(let _: & $ty = & $var;)* }
}

pub fn get_lan_ip() -> String{
    let sk = UdpSocket::bind("0.0.0.0:0").unwrap();
    let r: SocketAddr = SocketAddr::from_str("8.8.8.8:80").unwrap();
    let _ = sk.connect(r);
    sk.local_addr().unwrap().ip().to_string()
}

#[allow(dead_code)]
fn repr(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\n' => result.push_str("\\n"),
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            other => result.push(other),
        }
    }
    result
}

// 添加'='后自动尝试base64解码
pub async fn b64d(s: &String, url: &str, show: bool) -> Option<String> {
    let s = s.trim_end_matches('\n').trim_end_matches('\r');
    let mut result = None;
    for i in 0..4 {
        let a_s = [s, &"=".repeat(i)].concat();
        match STANDARD.decode(a_s) {
            Ok(bytes) => {
                result = Some(String::from_utf8(bytes).ok()?);
                break;
            },
            Err(e) => if show {
                    if s.len() >= 500 {
                        trace!("decode failed {e}, padding {i}, s[0..200] = {}, url = {url}", &s[0..100]);
                    } else {
                        trace!("decode failed {e}, padding {i}, s = {s}, url = {url}");
                    }
                },
        }
    }
    if None == result{
        if show{
            if s.len() >= 500 {
                trace!("64decode failed s[0..200] = {} url = {url}", &s[0..100]);
            } else {
                trace!("64decode failed s = {s} url = {url}");
            }
        }
    }
    result
}

// 去掉windows统一路径前面的\?\
pub fn remove_extended_prefix(pathbuf: &PathBuf) -> PathBuf {
    if let Some(prefix) = pathbuf.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        PathBuf::from(prefix)
    } else {
        pathbuf.clone()
    }
}


#[allow(dead_code)]
// 读取文件内容，返回String
pub fn read_file(file_path: &str) -> Option<String>{
    let mut path = PathBuf::new();
    path.push(file_path);
    if !path.is_absolute(){
        path = path.canonicalize().unwrap_or_else(|e|{debug!("file not found! {e}"); PathBuf::new()});
        debug!("now path is {path:?}");
    }
    if path.as_os_str().is_empty(){
        return None;
    }
    let mut s = String::new();
    let Ok(mut f) = File::open(path.as_path()) else {
        error!("fail to load file {path:?}");
        return None;
    };
    f.read_to_string(&mut s).unwrap_or_default();
    debug!("read {} bytes from file {:?}", s.len(), remove_extended_prefix(&path));

    Some(s)
}

// 调整别名，转为utf8，去掉空格和制表符
pub fn adjust_alias(s: String) -> String{
    let mut alias = textcode::utf8::decode_to_string(s.as_bytes());
    alias = alias.replace(" ", "").replace("\t", "");
    alias
}


// 合并两个json对象，将json_value2合并到json_value1
pub fn merge_json_objects(json_value1: &mut Value, json_value2: &Value) {
    let map1 = match json_value1 {
        Value::Object(map1) => map1,
        _ => panic!("json_value1 is not an object"),
    };

    let map2 = match json_value2 {
        Value::Object(map2) => map2,
        _ => panic!("json_value2 is not an object"),
    };

    for (key, value2) in map2 {
        match map1.entry(key.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(value2.clone());
            },
            Entry::Occupied(mut entry) => {
                let existing_value = entry.get_mut();
                match (existing_value.clone(), value2.clone()) {
                    (Value::Object(_), Value::Object(_)) => {
                        merge_json_objects(existing_value, value2);
                    },
                    (_, _) => {
                        entry.insert(value2.clone());
                    },
                }
            },
        };
    }
}

// 尝试访问代理，返回成功次数和最小延时
pub async fn check_proxy(worker_name: &str, node: &Node, client: &Client, url: &str, max_retry: u32, min_succ: u32, show_fail: bool, e_exit: Arc<Notify>) -> (u32, u32){
    let mut nr_succ = 0;
    let mut min_latency = 999999u128;
    for _nr in 1..=max_retry{
        let req = client.head(url).build().unwrap();
        let start = SystemTime::now();
        match client.execute(req).await{
            Ok(_)=>{
                let latency = SystemTime::now().duration_since(start).unwrap_or_default().as_millis();
                if latency < min_latency{
                    min_latency = latency;
                }
                nr_succ += 1;
                if nr_succ >= min_succ{   // 已达标
                    break;
                }else if nr_succ + (max_retry - _nr) < min_succ{  // 即使后面都成功也达不到min_succ则退出
                    break;
                }
            },
            Err(e)=>{
                if show_fail {
                    warn!("{worker_name} failed {node} {e}");
                }

                // 提前退出的情况：被主动拒绝10061，tls握手出错，解码出错；这几种情况再试也不会成功
                if e.is_connect() && e.is_request() {
                    if let Some(x) = e.source(){
                        if let Some(xx) = x.source() {
                            // warn!("{worker_name} request got sourc.source {}", xx);
                            if let Some(xxx) = xx.source() {
                                // warn!("{worker_name} request got source.source.source {}", xxx);
                                if xxx.to_string().contains("10061") {
                                    // warn!("{worker_name} request got {xxx}");
                                    trace!("{worker_name} q failed {node} {xxx}");
                                    break;
                                }
                            }
                            if xx.to_string().contains("tls handshake eof") {
                                // warn!("{worker_name} request got {xx}");
                                trace!("{worker_name} q failed {node} {xx}");
                                break;
                            }
                        }
                    }
                } else if e.is_decode(){
                    trace!("{worker_name} q failed {node} {e}");
                    break;
                }

                if nr_succ + (max_retry - _nr) < min_succ{
                    trace!("{worker_name} failed {node} {e}");
                    break;
                }
                match timeout(Duration::from_secs(2), e_exit.notified()).await {
                    Ok(_) => {
                        debug!("{worker_name} got exit flag");
                        break;
                    },
                    Err(_) => {}
                }
            }
        };
    }

    // if min_latency != 999999{
    //     debug!("{} check {} return nr_succ {} min_latency {}", worker_name, node, nr_succ, min_latency as u32);
    // }
    (nr_succ, min_latency as u32)
}


#[allow(dead_code)]
// #[cfg(unix)]
pub async fn get_distributor_id() -> String {
    let re = Regex::new(r"(?mR)^ID=(\w+)$").unwrap();
    let mut s = String::new();
    match Command::new("/bin/cat").arg("/etc/os-release").output().await {
        Ok(output) => {
            let cmd_out = String::from_utf8(output.stdout).unwrap_or_else(|_| String::from("no stdout content"));
            match re.captures(&cmd_out) {
                Some(m) => {
                    s = String::from(&m[1]);
                },
                None => {
                    warn!("no ID found !!!");
                }
            }
            // let cmd_err = String::from_utf8(output.stderr).unwrap_or_else(|_| String::from("no stderr content"));
            // let value: tvalue = toml::from_str(cmd_out.as_str()).unwrap();
            // debug!("cmd_out= {cmd_out} cmd_err= {cmd_err} value= {value}");
            // match value.get("ID") {
            //     Some(x) => { s = String::from(x.as_str().unwrap()); },
            //     None => { warn!("no ID found !!!"); }
            // }
        },
        Err(err) => {
            warn!("cmd exec failed!!! {err}");
        }
    }

    s
}
