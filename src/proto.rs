use std::collections::HashMap;
use url::Url;
use urlparse::unquote_plus;
#[allow(unused_imports)]
use log::{info, debug, trace, warn, error};
use serde_json::Value;
use serde_json::json;
use serde_yaml::Value as Yamlv;  
use tokio::time;
use scraper::{Html, Selector};
use regex::Regex;
use std::time::Duration;
use crate::node::Node;
use crate::tools;
use crate::tools::b64d;
// use crate::tools::check_addr;
use async_channel::Sender;
use chrono;

pub async fn data2node_one(worker_name: &String, s: &String, url: &String, node_out: &Sender<Node>) -> () {
    let mut l_data: Vec<String>;
    if let Some(raw_s) = b64d(s, url, true).await{
        l_data = raw_s.lines().map(|c| c.to_string()).filter(|x|x.trim().len()>0).collect();
        trace!("{worker_name} after split to lines.len={}, url={url}", l_data.len());
    }
    else{
        // 尝试当成没有base64编码的处理
        l_data = s.lines().map(|c| c.to_string()).filter(|x|x.trim().len()>0).collect();
        trace!("{worker_name} after split to lines, l_data.len={}, url={url}", l_data.len());
        if l_data.is_empty(){
            return;
        }
    }

    let mut nr = 0;
    for (_i, mut _data) in l_data.iter_mut().enumerate() {
        // debugonly
        // if _i >= 50{
        //     warn!("{} debug break", worker_name);
        //     break;
        // }

        // 检查是否包含htmlentity &amp; 有的话转换为 &
        if _data.contains("&amp;") {
            *_data = _data.replace("&amp;", "&");
        }

        let p: Url;
        // trace!("{} processing _data {}", worker_name, _data);
        match Url::parse(_data.as_str()){
            Ok(ps)=> {p=ps;},
            Err(err)=>{
                trace!("{worker_name} url parse got {err}, url{}=|{}|, source={url}", if _data.len()<128{""}else{"(part)"}, if _data.len()<128{_data.to_string()}else{_data[..128].to_string()});
                continue;
            }
        }
        // trace!("{} now p={}", worker_name, p);
        let proto = p.scheme();
        match proto {
            "vmess" => {  // base64(json(key: <id><add><path><port><host><aid><tls><ps>))
                // trace!("{} got vmess data", worker_name);
                let b64_str = p.host().map_or(String::new(), |s|s.to_string());
                if b64_str.is_empty(){
                    warn!("{worker_name} no host found!");
                    continue;
                }
                let mut json_str=b64d(&b64_str.to_string(), url, false).await.unwrap_or_default();
                if json_str.is_empty(){
                    // debug!("{} b64 decode failed for {} from {}", worker_name, b64_str, url);
                    // continue;

                    // 假定vmess:://后面都是b64编码的
                    json_str = b64d(&_data[8..].to_string(), url, false).await.unwrap_or_default();
                    if json_str.is_empty() {
                        continue;
                    }
                }
                let mut x: Value = serde_json::from_str(json_str.as_str()).unwrap_or_default(); //(Value::Null, |s|s);
                if x == Value::Null{
                    trace!("{worker_name} json load failed {json_str}");
                    continue;
                }
                if let None = x.get("path") {
                    x["path"] = Value::String(String::from("/"));
                }
                if let None = x.get("host") {
                    x["host"] = Value::String(String::new());
                }

                let add = String::from(x["add"].as_str().unwrap_or_default());
                let uuid = String::from(x["id"].as_str().unwrap_or_default());
                let mut alias = if x.get("ps") == None {
                    add.clone()
                } else {
                    String::from(x["ps"].as_str().unwrap_or_default())
                };
                alias = tools::adjust_alias(alias);
                let mut port: u32 = x["port"].as_str().map_or(0, |s|s.parse().unwrap_or_default());
                if port == 0 {
                    port = x["port"].as_u64().map_or(0 as u32, |s|s as u32);
                    if port == 0 {
                        warn!("{worker_name} port is 0, skip!!! {x:?}");
                        continue;
                    }
                }

                // trace!("{} proto {} uuid {}, ip {}, port {}, alias {:?}", worker_name, proto, uuid, add, port, alias);
                if let Err(e) = node_out.send(Node {
                    proto: proto.to_string(),
                    param: x,
                    uuid,
                    ip: add,
                    port,
                    alias,
                    source: String::from(url),
                    real_ip: String::new(),
                    area: String::new(),
                    score: 0,
                    score_unit: String::from("ms"),
                    raw: _data.clone(),
                }).await {
                    debug!("{worker_name} putting data got error!!! {e}");
                };
            },
            "vless" => {  // <uuid>@<ip>:<port>?encryption=<>&security=<>&sni=<>&type=<>&host=<>&path=<>#<alias>
                // trace!("{} got vless data", worker_name);
                let params = p.query_pairs();
                // let p_split = Regex::new(":|@").unwrap();
                // let fields: Vec<&str> = p_split.splitn(p.path(), 3).collect();
                // trace!("{} fileds: {:?}", worker_name, fields);
                let mut param: HashMap<String, String> = HashMap::new();
                for _item in params {
                    param.insert(_item.0.to_string(), _item.1.to_string());
                }
                if let None = param.get("path") {
                    param.insert(String::from("path"), String::from("/"));
                }
                if let None = param.get("security") {
                    param.insert(String::from("security"), String::from("none"));
                }
                if let None = param.get("type") {
                    // trace!("{} parse vless, no 'type' in param, url = {}", worker_name, url);
                    continue;
                }
                let uuid = p.username().to_string();
                let mut ip = p.host().map_or(String::new(), |s|s.to_string());
                // check ip6
                if ip.starts_with('[') && ip.ends_with(']'){
                    ip = ip.strip_prefix('[').unwrap_or_default().strip_suffix(']').unwrap_or_default().to_string();
                    // trace!("{} strip ipv6 to {}", worker_name, ip);
                }

                let port: u32 = p.port().map_or(0, |s|s as u32);
                let mut alias: String = unquote_plus(p.fragment().unwrap_or_default()).unwrap_or_default();
                alias = tools::adjust_alias(alias);
                // trace!("{} proto {} uuid {}, ip {}, port {}, alias {:?} param: {:?}", worker_name, proto, uuid, ip, port, alias, param);

                if let Err(e) = node_out.send(Node {
                    proto: proto.to_string(),
                    uuid,
                    ip,
                    port,
                    param: serde_json::to_value(param).unwrap_or_default(),
                    alias,
                    real_ip: String::new(),
                    area: String::new(),
                    score: 0,
                    score_unit: String::from("ms"),
                    source: url.to_string(),
                    raw: _data.clone(),
                }).await {
                    debug!("{worker_name} putting data got error!!! {e}");
                };
            },
            "trojan" => {  // <password>@<ip>:<port>?security=<>&sni=<>&type=<>&headerType=<>#<alias>

                // debug!("{} trojan data usrname:{} host:{} port:{} query:{} fragment:{}", worker_name, p.username(), p.host_str().unwrap_or_default(), p.port().unwrap_or_default(), p.query().unwrap_or_default(), unquote_plus(p.fragment().unwrap_or_default()).unwrap_or_default());
                let password = p.username();
                let ip = p.host_str().unwrap_or_default().to_string();
                let port: u32 = p.port().unwrap_or_default() as u32;
                let params = p.query_pairs();
                let mut param: HashMap<String, String> = HashMap::new();
                for _item in params {
                    param.insert(_item.0.to_string(), _item.1.to_string());
                }
                param.insert(String::from("password"), password.to_string());
                let mut alias = unquote_plus(p.fragment().unwrap_or_default()).unwrap_or_default();
                alias = tools::adjust_alias(alias);
                if let Err(e) = node_out.send(Node {
                    proto: proto.to_string(),
                    uuid: String::new(),
                    ip,
                    port,
                    param: serde_json::to_value(param).unwrap_or_default(),
                    alias, 
                    real_ip: String::new(),
                    area: String::new(),
                    score: 0,
                    score_unit: String::from("ms"),
                    source: url.to_string(),
                    raw: _data.clone(),
                }).await {
                    debug!("{worker_name} putting data got error!!! {e}");
                };
            },
            "ss" => {  // base64(<method>:<password>)@<ip>:<port>#<alias>
                let s: String=b64d(&String::from(p.host_str().unwrap_or_default()), url, false).await.unwrap_or_default();
                if s.is_empty(){
                    let mut ip = p.host_str().unwrap_or_default();
                    let port: u32 = p.port().unwrap_or_default().into();
                    if ip.starts_with('[') && ip.ends_with(']'){  // ipv6
                        ip = ip.strip_prefix('[').unwrap_or_default().strip_suffix(']').unwrap_or_default();
                    }
                    let mut _method: String;
                    let mut _password: String;
                    let _tmp = b64d(&unquote_plus(p.username().to_string()).unwrap_or_default(), url, false).await.unwrap_or_default();
                    if _tmp.is_empty(){  // not base64 encoded
                        _method = String::from(p.username());
                        _password = String::from(p.password().unwrap_or_default());
                    } else{
                        let _tmp: Vec<&str> = _tmp.splitn(2, ':').collect();
                        if _tmp.len() != 2{
                            debug!("{} parse method/pass failed! _tmp={:?}", worker_name, _tmp);
                            continue;
                        }
                        _method = String::from(_tmp[0]);
                        _password = String::from(_tmp[1]);
                    }
                    // trace!("{} method={}, pass={}, _data={}", worker_name, _method, _password, _data);
                    let params = p.query_pairs();
                    let mut param: HashMap<String, String> = HashMap::new();
                    for _item in params {
                        param.insert(_item.0.to_string(), _item.1.to_string());
                    }

                    // 尝试修复方法密码只有其一的情况，有方法无密码，认为方法字段存的是密码；有密码无方法，认为方法是aes-256-gcm
                    if _method.len() > 0 && _password.len() == 0{
                        _password = _method.clone();
                        _method = String::from("aes-256-gcm");
                    }else if _method.len() == 0 && _password.len() >0{
                        if _password.len() <= 16{
                            _method = String::from("aes-256-cfb");
                        }else{
                            _method = String::from("aes-256-gcm");
                        }
                    }

                    param.insert(String::from("method"), _method);
                    param.insert(String::from("password"), _password);
                    let mut alias = unquote_plus(p.fragment().unwrap_or_default()).unwrap_or_default();
                    alias = tools::adjust_alias(alias);
                    // debug!("{} add ss node ip {} port {}, param {:?} alias {}", worker_name, ip, port, param, alias);
                    if let Err(e) = node_out.send(Node {
                        proto: proto.to_string(),
                        uuid: String::new(),
                        ip: ip.to_string(),
                        port,
                        param: serde_json::to_value(param).unwrap_or_default(),
                        alias, 
                        real_ip: String::new(),
                        area: String::new(),
                        score: 0,
                        score_unit: String::from("ms"),
                        source: url.to_string(),
                        raw: _data.clone(),
                    }).await {
                        debug!("{worker_name} putting data got error!!! {e}");
                    };

                }
                else{
                    let re = regex::Regex::new(":|@").unwrap();
                    let _tmp: Vec<&str> = re.splitn(s.as_str(), 4).collect();
                    if _tmp.len() != 4{
                        debug!("{} parse failed, {:?}", worker_name, s);
                        continue;
                    }
                    let mut _method = String::from(_tmp[0]);
                    let mut _password = String::from(_tmp[1]);
                    let ip = String::from(_tmp[2]);
                    let port: u32 = _tmp[3].parse().unwrap_or_default();
                    let mut alias = unquote_plus(p.fragment().unwrap_or_default()).unwrap_or_default();
                    alias = tools::adjust_alias(alias);
                    let mut param: HashMap<String, String> = HashMap::new();

                    // 尝试修复方法密码只有其一的情况，有方法无密码，认为方法字段存的是密码；有密码无方法，认为方法是aes-256-gcm
                    if _method.len() > 0 && _password.len() == 0{
                        _password = _method.clone();
                        _method = String::from("aes-256-gcm");
                    }else if _method.len() == 0 && _password.len() >0{
                        if _password.len() <= 16{
                            _method = String::from("aes-256-cfb");
                        }else{
                            _method = String::from("aes-256-gcm");
                        }
                    }

                    param.insert(String::from("method"), _method);
                    param.insert(String::from("password"), _password);

                    // debug!("{} add ss node ip {} port {}, param {:?} alias {},", worker_name, ip, port, param, alias);
                    if let Err(e) = node_out.send(Node {
                        proto: proto.to_string(),
                        uuid: String::new(),
                        ip,
                        port,
                        param: serde_json::to_value(param).unwrap_or_default(),
                        alias, 
                        real_ip: String::new(),
                        area: String::new(),
                        score: 0,
                        score_unit: String::from("ms"),
                        source: url.to_string(),
                        raw: _data.clone(),
                    }).await {
                        debug!("{worker_name} putting data got error!!! {e}");
                    };
                }

            },
            "hysteria" | "hysteria2" | "hy2" | "ssr" | "tuic" | "http" | "https" | "socks" | "socks5" => {
                trace!("{worker_name} not implemented for {proto}");
            },
            o => {
                debug!("{worker_name} unknown schema: {o}");
            }
        }

        nr += 1;

        if nr % 10 == 0{
            time::sleep(Duration::from_millis(20)).await;
        }
    }
}

pub async fn parse_yaml(worker_name: &String, s: &String, url: &String, node_out: &Sender<Node>) -> () {
    // trace!("{} processing yaml data from {}", worker_name, url);
    // let yaml_data: Yamlv = serde_yaml::from_str(s).unwrap_or_default();
    let yaml_data: HashMap<String, Yamlv> = serde_yaml::from_str(s).unwrap_or_default();
    if yaml_data.contains_key("proxies") {
        let l_proxy: Vec<Yamlv> = match yaml_data.get("proxies"){
            Some(Yamlv::Sequence(seq)) => seq.clone(),
            _=> Vec::new()
        };
        // debug!("{} got {} proxies", worker_name, l_proxy.len());
        let mut nr = 0;
        for _proxy in l_proxy{
            match _proxy.get("type"){
                Some(_type)=>match _type.as_str().unwrap_or_default(){
                    "ssr" | "hysteria" | "hysteria2" | "hy2" | "tuic" => {
                        trace!("{} not implemented for {}", worker_name, _type.as_str().unwrap_or_default());
                        continue;
                    },
                    _ => {}
                },
                None => {
                    warn!("{} can\'t get porxy type, {:?} skip !!! {}", worker_name, _proxy, url);
                    continue;
                }
            }
            let ip: String = String::from(_proxy.get("server").unwrap_or_else(||&Yamlv::Null).as_str().unwrap_or_default());
            let id: String = String::from(_proxy.get("uuid").unwrap_or_else(||&Yamlv::Null).as_str().unwrap_or_default());
            let port: u32 = _proxy.get("port").unwrap_or_else(||&Yamlv::Null).as_u64().unwrap_or_default() as u32;
            if port == 0 {
                warn!("{} port is 0 !!! {:?}", worker_name, _proxy);
            }
            let aid: u32 = _proxy.get("alterId").unwrap_or_else(||&Yamlv::Null).as_u64().unwrap_or_default() as u32;
            let tls: String = String::from(_proxy.get("tls").unwrap_or_else(||&Yamlv::Null).as_str().unwrap_or_default());
            // let dft: BTreeMap<String, Yamlv> = BTreeMap::new();
            let path: String;
            let mut host: HashMap<String, String> = HashMap::new();
            let mut _t = _proxy.get("ws-opts").unwrap_or_else(||&Yamlv::Null);
            if *_t != Yamlv::Null{
                path = String::from(_t.get("path").unwrap_or_else(||&Yamlv::Null).as_str().unwrap_or_default());
                _t = _t.get("headers").unwrap_or_else(||&Yamlv::Null);
                if *_t != Yamlv::Null{
                    host = match _t.get("host"){
                        Some(o)=>{
                            let mut m : HashMap<String, String> = HashMap::new();
                            if let Yamlv::Mapping(_m) = o{
                                for (_k, _v) in _m.into_iter(){
                                    if let (Yamlv::String(_key), Yamlv::String(_value)) = (_k, _v){
                                        m.insert(_key.clone(), _value.clone());
                                    }
                                }
                            }
                            m
                        }
                        None=>{
                            HashMap::new()
                        }
                    };
                }
            }else{
                path = String::from("/");
            }

            let mut _param = json!({
                "ip": ip,
                "id": id,
                "port": port,
                "aid": aid,
                "tls": tls,
                "path": path,
                "host": host,
            });

            match _proxy.get("type"){
                Some(_type)=>match _type.as_str().unwrap_or_default(){
                    "ss"=>{
                        let _c: Yamlv = _proxy.get("cipher").map_or_else(||Yamlv::String(String::new()), |o|o.clone());
                        _param["method"] = Value::String(String::from(_c.as_str().unwrap_or_default()));
                    },
                    "vless"=>{
                        let _c: Yamlv = _proxy.get("tls").map_or_else(||Yamlv::String(String::new()), |o|o.clone());
                        _param["security"] = Value::String(String::from(_c.as_str().unwrap_or_default()));
                    },
                    "trojan"=>{
                        let _c: Yamlv = _proxy.get("skip-cert-verify").map_or_else(||Yamlv::Bool(true), |o|o.clone());
                        _param["allowInsecure"] = Value::Bool(_c.as_bool().unwrap_or_default());
                    },
                    _=>{}
                },
                None=>{}
            }

            let _yaml2json = serde_json::to_value(_proxy.clone()).unwrap_or_default();
            tools::merge_json_objects(&mut _param, &_yaml2json);
            let mut alias = _param.get("name").map_or_else(||String::new(), |o|unquote_plus(o.as_str().unwrap_or_default()).unwrap_or_default());
            alias = tools::adjust_alias(alias);

            if let Err(e) = node_out.send(Node {
                proto: String::from(_param.get("type").unwrap_or_else(||&Value::Null).as_str().unwrap_or_default()),
                uuid: id,
                ip,
                port,
                param: serde_json::to_value(_param).unwrap_or_default(),
                alias, 
                real_ip: String::new(),
                area: String::new(),
                score: 0,
                score_unit: String::from("ms"),
                source: url.to_string(),
                raw: serde_yaml::to_string(&_proxy).unwrap_or_default(),
            }).await {
                debug!("{worker_name} putting got error!!! {e}");
            };
            nr += 1;

            if nr % 10 == 0{
                time::sleep(Duration::from_millis(20)).await;
            }
        }
    }
    else{
        error!("{worker_name} get 'proxies' failed! url={url}");  
    }
}

pub async fn get_mibei_url(worker_name: &str) -> Option<String>{
    trace!("{worker_name} try to get mibei url...");
    let url = "https://www.mibei77.com/";
    let mut ret = String::new();
    let mut lastest_url = String::new();
    match reqwest::get(url).await {
        Ok(o) => {
            if let Ok(s) = o.text().await {
                trace!("{worker_name} got {} byte(s) from {url}", s.len());
                let doc = Html::parse_document(s.as_str());
                let sel = Selector::parse("article.blog-post.hentry.index-post.post-0 > div.entry-header > h2.entry-title > a").unwrap();
                if let Some(_item) = doc.select(&sel).next(){
                    if let Some(x) = _item.attr("href"){
                        lastest_url = String::from(x);
                    }
                } else {
                    warn!("{worker_name} can\'t find lastest page url!!!");
                }
            }
        },
        Err(e) => {
            warn!("{worker_name} fetching {url} got err {e}");
        }
    }

    if lastest_url.len() > 0{
        trace!("{worker_name} got {lastest_url}");
        match reqwest::get(lastest_url.clone()).await{
            Ok(o) => {
                if let Ok(s) = o.text().await {
                    trace!("{worker_name} got {} bytes from {lastest_url}", s.len());
                    let re = Regex::new(r"http://mm\.mibei77\.com/.+?\.txt").unwrap();
                    match re.find(&s){
                        Some(x) => {
                            ret = String::from(x.as_str());
                            trace!("{worker_name} got mibei url {ret}");
                        },
                        None => {
                            warn!("{worker_name} can't found mibei url");
                        }
                    };
                } else {
                    warn!("{worker_name} can't get content from {lastest_url} !!!");
                }

            },
            Err(e) => {
                warn!("{worker_name} can't get content from {lastest_url}, err: {e} !!!");
            }
        }
    }

    Some(ret)
}

pub async fn get_tolinkshare_data(worker_name: &str) -> Option<(String, String)>{
    let url = "https://mirror.ghproxy.com/https://raw.githubusercontent.com/tolinkshare/freenode/main/README.md";
    let mut ret = (String::from(url), String::new());
    let mut content = String::new();
    match reqwest::get(url).await {
        Ok(o) => {
            if let Ok(s) = o.text().await {
                trace!("{worker_name} got {} byte(s) from {url}", s.len());
                content = String::from(s);
            } else {
                warn!("{worker_name} can't get content from {url} !!!");
            }
        },
        Err(e) => {
            warn!("{worker_name} fetching {url} got err {e}");
        }
    }

    if let Some(idx) = content.find("免费v2rayN节点列表") {
        let re = Regex::new(r"(?s)```(.+?)```").unwrap();
        if let Some(m) = re.captures(&content[idx..]) {
            ret.1 = String::from(&m[1]);
            trace!("{worker_name} got {} byte(s) content from {url}", ret.1.len());
        }
    }

    Some(ret)
}

pub async fn get_vpnnet_data(worker_name: &str) -> Option<(String, String)>{
    let url = "https://mirror.ghproxy.com/https://raw.githubusercontent.com/VpnNetwork01/vpn-net/main/README.md";
    let mut ret = (String::from(url), String::new());
    let mut content = String::new();
    match reqwest::get(url).await {
        Ok(o) => {
            if let Ok(s) = o.text().await {
                trace!("{worker_name} got {} byte(s) from {url}", s.len());
                content = String::from(s);
            } else {
                warn!("{worker_name} can't get content from {url} !!!");
            }
        },
        Err(e) => {
            trace!("{worker_name} fetching {url} got err {e}");
        }
    }

    if let Some(idx) = content.find("免费节点") {
        let re = Regex::new(r"(?s)```(.+?)```").unwrap();
        if let Some(m) = re.captures(&content[idx..]) {
            ret.1 = String::from(&m[1]);
            trace!("{worker_name} got {} byte(s) content from {url}", ret.1.len());
        }
    }

    Some(ret)
}

pub async fn get_shareclash_url(worker_name: &str) -> Option<Vec<String>>{
    let url = "https://mirror.ghproxy.com/https://raw.githubusercontent.com/shareclash/shareclash.github.io/main/README.md";
    let mut ret = vec![];
    let mut content = String::new();
    match reqwest::get(url).await {
        Ok(o) => {
            if let Ok(s) = o.text().await {
                trace!("{worker_name} got {} byte(s) from {url}", s.len());
                content = String::from(s);
            } else {
                warn!("{worker_name} can't get content from {url} !!!");
            }
        },
        Err(e) => {
            trace!("{worker_name} fetching {url} got err {e}");
        }
    }

    if let Some(idx) = content.find("V2ray订阅链接") {
        let re = Regex::new(r"(?im)^-\s+(https://.+?\.txt)").unwrap();
        for (_, [_url]) in re.captures_iter(&content[idx..]).map(|c| c.extract()) {
            trace!("{worker_name} got url {}", _url);
            ret.push(String::from(_url));
        } 
        trace!("{worker_name} got {} url(s) content from {url}", ret.len());
    }

    Some(ret)
}

pub async fn get_v2rayclashnode_url(worker_name: &str) -> Option<Vec<String>>{
    let url = "https://mirror.ghproxy.com/https://raw.githubusercontent.com/v2rayclashnode/v2rayclashnode.github.io/main/README.md";
    let mut ret = vec![];
    let mut content = String::new();
    match reqwest::get(url).await {
        Ok(o) => {
            if let Ok(s) = o.text().await {
                trace!("{worker_name} got {} byte(s) from {url}", s.len());
                content = String::from(s);
            } else {
                warn!("{worker_name} can't get content from {url} !!!");
            }
        },
        Err(e) => {
            trace!("{worker_name} fetching {url} got err {e}");
        }
    }

    if let Some(idx) = content.find("V2ray订阅链接") {
        let re = Regex::new(r"(?im)^-\s+(https://.+?\.txt)").unwrap();
        for (_, [_url]) in re.captures_iter(&content[idx..]).map(|c| c.extract()) {
            trace!("{worker_name} got url {}", _url);
            ret.push(String::from(_url));
        } 
        trace!("{worker_name} got {} url(s) content from {url}", ret.len());
    }

    Some(ret)
}

// 直接返回"trjan:://xxxx"形式的节点列表
pub async fn get_sharkdoor_url(worker_name: &str) -> Option<Vec<(String, String)>>{
    let s_lasthour = (chrono::Local::now() - chrono::Duration::hours(1)).format("%Y-%m/%d日%H时30分.md").to_string();
    let s_url = String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/sharkDoor/vpn-free-nodes/refs/heads/master/node-list/") + s_lasthour.as_str();
    let url = s_url.as_str();
    let mut ret = vec![];
    let mut content = String::new();
    match reqwest::get(url).await {
        Ok(o) => {
            if let Ok(s) = o.text().await {
                trace!("{worker_name} got {} byte(s) from {url}", s.len());
                content = String::from(s);
            } else {
                warn!("{worker_name} can't get content from {url} !!!");
            }
        },
        Err(e) => {
            trace!("{worker_name} fetching {url} got err {e}");
        }
    }

    if let Some(idx) = content.find("链接|") {
        let re = Regex::new(r"(?im)\|(trojan:\/\/.+?)\|$").unwrap();
        for (_, [_node]) in re.captures_iter(&content[idx..]).map(|c| c.extract()) {
            trace!("{worker_name} got url {}", _node);
            ret.push((String::from(url), String::from(_node)));
        } 
        trace!("{worker_name} got {} url(s) content from {url}", ret.len());
    }

    Some(ret)
}