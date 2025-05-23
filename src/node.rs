use serde_json::Value;
use serde_json::json;
use urlparse::unquote_plus;
use std::hash::{Hash, Hasher};
use std::fmt::{Display, Formatter, Result};
use std::str::FromStr;
#[allow(unused_imports)]
use log::{error, debug, info, warn};

#[allow(dead_code)]
#[derive(Debug)]
pub struct Node {
    pub proto: String,
    pub uuid: String,
    pub ip: String,
    pub port: u32,
    pub param: Value,
    pub alias: String,

    pub real_ip: String,
    pub area: String,
    pub score: u32,
    pub score_unit: String,
    pub source: String,
    pub raw: String,
}

impl Node{
    pub fn new() -> Node{
        return Node{
            proto: String::new(),
            uuid: String::new(),
            ip: String::new(),
            port: 0,
            param: Value::Null,
            alias: String::new(),
            real_ip: String::new(),
            area: String::new(),
            score: 0,
            score_unit: String::from("ms"),
            source: String::new(),
            raw: String::new(),
        };
    }

    // pub fn is_empty(&self) -> bool{
    //     self.proto.is_empty() && self.ip.is_empty()
    // }
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
trait Descriptive {
    fn describe(&self) -> String;
}

impl Descriptive for Node {
    fn describe(&self) -> String {
        format!("<N {}:{}:{}>", &self.proto[..2], self.ip, self.port)
    }
}

impl Display for Node{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        if self.proto.len() >= 2{
            write!(f, "<N {}:{}:{}>", &self.proto[..2], self.ip, self.port)
        }else{
            write!(f, "<N {}:{}:{}>", &self.proto, self.ip, self.port)
        }
    }
}

impl Eq for Node{}

impl Hash for Node{
    fn hash<H: Hasher>(&self, state: &mut H){
        self.ip.hash(state);
        self.port.hash(state);
        self.uuid.hash(state);
        self.param.as_str().hash(state);
    }
}

impl PartialEq for Node{
    fn eq(&self, other: &Self) -> bool{
        self.ip == other.ip && self.port == other.port && self.uuid == other.uuid && self.param.eq(&other.param)
    }
}

// #[allow(dead_code, unused_variables)]
pub fn create_proxy_conf(worker_name: &str, node: &Node, conf: &Value, port: u64, measure_mode: bool) -> Option<String>{
    let mut setting: Value;
    match node.proto.as_str(){
        "vmess"=>{
            let Some(arg) = node.param.as_object() else {
                error!("{worker_name} node.param convert to hashmap failed!!! param={}", node.param);
                return None;
            };
            let mut outbounds_setting = conf["outboundsSetting_vmess"].clone();
            if arg.contains_key("add"){
                outbounds_setting[0]["settings"]["vnext"][0]["address"] = arg["add"].clone();
            }else if !node.ip.is_empty(){
                outbounds_setting[0]["settings"]["vnext"][0]["address"] = Value::from(node.ip.clone());
            }else{
                outbounds_setting[0]["settings"]["vnext"][0]["address"] = Value::from(node.real_ip.clone());
            }

            outbounds_setting[0]["settings"]["vnext"][0]["port"] = match arg["port"].as_number(){
                Some(o)=>Value::from(o.clone()),
                None=>{
                    // debug!("{} port can't convert {} as number!!!", worker_name, arg["port"]);
                    match serde_json::Number::from_str(arg["port"].as_str().unwrap_or_default()){
                        Ok(o)=>Value::from(o),
                        Err(e)=>{
                            debug!("{worker_name} port can't convert {} to number!!! {e}", arg["port"]);
                            json!(0)
                        }
                    }
                }
            };

            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["id"] = arg["id"].clone();
            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["alterId"] = if arg.contains_key("aid"){
                if arg["aid"].as_str().unwrap_or_default().len()==0{
                    Value::from(0)
                }else{
                    Value::from(arg["aid"].as_u64().unwrap_or_default())
                }
            }else{
                Value::from(64)
            };
            outbounds_setting[0]["streamSettings"]["wsSettings"]["path"] = arg["path"].clone();
            if arg["host"].as_str().unwrap_or_default().len() != 0{
                outbounds_setting[0]["streamSettings"]["wsSettings"]["Host"] = arg["host"].clone()
            }
            // outbounds_setting[0]["streamSettings"]["wsSettings"]["headers"] = json!({});
            outbounds_setting[0]["streamSettings"]["security"] = match arg.get("tls"){
                Some(o)=>{
                    if o.as_str().unwrap_or_default().len()!=0{
                        o.clone()
                    }else{
                        Value::from("none")
                    }
                },
                None=>Value::from("none")
            };
            setting = conf["conf_tpl"].clone();
            setting["inbounds"] = conf["inboundsSetting"].clone();
            setting["outbounds"] = outbounds_setting.clone();
        },
        "vless"=>{
            let Some(arg) = node.param.as_object() else {
                error!("{worker_name} node.param convert to hashmap failed!!! param={}", node.param);
                return None;
            };
            let mut outbounds_setting = conf["outboundsSetting_vless"].clone();
            outbounds_setting[0]["settings"]["vnext"][0]["address"] = Value::from(node.ip.clone());
            outbounds_setting[0]["settings"]["vnext"][0]["port"] = Value::from(node.port);
            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["id"] = Value::from(node.uuid.clone());
            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["alterId"] = Value::from(0);
            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["email"] = Value::from("wd4mxs@mx.com");
            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["encryption"] = match arg.get("encryption"){
                Some(o)=>{if o.as_str().unwrap_or_default().len()>0{
                    o.clone()
                }else{
                    Value::from("none")
                }},
                None=>Value::from("none")
            };
            outbounds_setting[0]["settings"]["vnext"][0]["users"][0]["flow"] = arg.get("flow").map_or_else(||json!(""), |o|o.clone());
            let mut network = arg.get("type").map_or_else(||String::new(), |o|o.to_string());
            if network.starts_with('"'){
                network =  network.strip_prefix('"').unwrap_or_default().strip_suffix('"').unwrap_or_default().to_string();
                // warn!("{} network in \"\" form {}", worker_name, network);
            }

            // 修正一下
            if network == "vless"{
                network = arg.get("network").map_or_else(||String::new(), |o|o.to_string());
                if network.starts_with('"'){
                    network =  network.strip_prefix('"').unwrap_or_default().strip_suffix('"').unwrap_or_default().to_string();
                    // warn!("{} network in \"\" form {}", worker_name, network);
                }
            }
            //

            outbounds_setting[0]["streamSettings"]["network"] = Value::from(network.clone());
            let mut security = arg.get("security").map_or_else(||String::new(), |o|o.to_string());
            if security.starts_with('"'){
                security =  security.strip_prefix('"').unwrap_or_default().strip_suffix('"').unwrap_or_default().to_string();
                // warn!("{} security in \"\" form {}", worker_name, security);
            }
            outbounds_setting[0]["streamSettings"]["security"] = Value::from(security.clone());
            match security.as_str(){
                "tls"=>{
                    outbounds_setting[0]["streamSettings"]["tlsSettings"] = json!({
                        "serverName": arg.get("sni").map_or_else(||Value::from(node.ip.clone()), |o|o.clone()),
                        "allowInsecure": false,
                        "show": false,
                        "fingerprint": arg.get("fp").map_or_else(||arg.get("fp").map_or_else(||json!("chrome"), |o|o.clone()), |o|o.clone())});
                    if arg.contains_key("alpn"){
                        outbounds_setting[0]["streamSettings"]["tlsSettings"]["alpn"] = match unquote_plus(arg["alpn"].as_str().unwrap_or_default()){
                            Ok(o)=>{let x:Vec<&str>=o.split(',').collect(); json!(x)},
                            Err(e)=>{error!("{worker_name} alpn unquote failed!!! {e}"); json!([])}
                        };
                    }

                },
                "reality"=>{
                    outbounds_setting[0]["streamSettings"]["realitySettings"] = json!({
                        "fingerprint": arg.get("fingerprint").map_or_else(||arg.get("fp").map_or_else(||json!("chrome"), |o|o.clone()), |o|o.clone()),
                        "serverName": arg.get("sni").map_or_else(||Value::from(node.ip.clone()), |o|o.clone()),
                        "show": false,
                        "publicKey": arg.get("public-key").map_or_else(||arg.get("pbk").map_or_else(||json!(""), |o|o.clone()), |o|o.clone()),
                        "shortId": arg.get("short-id").map_or_else(||arg.get("sid").map_or_else(||json!(""), |o|o.clone()), |o|o.clone()),
                        "spiderX": unquote_plus(arg.get("spx").map_or_else(||"", |o|o.as_str().unwrap_or_default())).unwrap_or_default()
                        });
                    if !arg.contains_key("public-key") && !arg.contains_key("pbk"){
                        warn!("{worker_name} key 'public-key' 'pbk' not found in {node}, arg={arg:?} node.source={}", node.source);
                    }
                    if !arg.contains_key("short-id") && !arg.contains_key("sid"){
                        warn!("{worker_name} key 'short-id' 'sid' not found in {node}, arg={arg:?} node.source={}", node.source);
                    }
                },
                "" | "none" | "None" | "null" | "Null" =>{
                    if let Value::Object(ref mut tmp) = outbounds_setting[0]["streamSettings"]{
                        tmp.remove_entry("security");
                    }else{
                        error!("{worker_name} remove key 'security' failed!");
                    }
                },
                _ =>{
                    warn!("{worker_name} unknown security={security} in vless, node={node}, node.param={:?}, node.source={}", node.param, node.source);
                }
            }
            match network.as_str(){
                "tcp"=>{},
                "ws"=>{
                    outbounds_setting[0]["streamSettings"]["tlsSettings"] = json!({
                        "serverName": arg.get("sni").map_or_else(||Value::from(node.ip.clone()), |o|o.clone())
                    });
                    outbounds_setting[0]["streamSettings"]["wsSettings"] = json!({
                        "path": arg["path"],
                        "Host": arg.get("host").map_or_else(||Value::from(""), |o|o.clone()),
                        "headers": {}
                    });
                },
                "httpupgrade"=>{
                    outbounds_setting[0]["streamSettings"]["httpupgradeSettings"] = json!({
                        "host": arg.get("host").map_or_else(||Value::from(""), |o|o.clone()),
                        "path": arg.get("path").clone(),
                    });
                },
                "xhttp"=>{
                    outbounds_setting[0]["streamSettings"]["xhttpSettings"] = json!({
                        "path": arg.get("path").clone(),
                    });
                },
                "grpc"=>{
                    outbounds_setting[0]["streamSettings"]["grpcSettings"] = json!({
                        "serviceName": unquote_plus(arg.get("serviceName").map_or_else(||"", |o|o.as_str().unwrap_or_default())).unwrap_or_default(),
                        "multiMode": match arg.get("mode"){
                            Some(o)=>{
                                if o.as_str().unwrap_or_default() == "multi"{
                                    true
                                }else{
                                    false
                                }
                            }
                            None=>false
                        },
                        "idle_timeout": 60,
                        "health_check_timeout": 20,
                        "permit_without_stream": false,
                        "initial_windows_size": 0,
                        });

                },
                "" | "none" | "None" | "null" | "Null" => {
                    if let Value::Object(ref mut tmp) = outbounds_setting[0]["streamSettings"]{
                        tmp.remove_entry("network");
                    }else{
                        error!("{worker_name} remove key 'network' failed!");
                    }
                },
                unknown @ _ => {
                    warn!("{worker_name} unknown network={unknown} in vless, node={node}, node.param={:?}, node.source={}, node.raw={}", node.param, node.source, node.raw);
                }
            }
            setting = conf["conf_tpl"].clone();
            setting["inbounds"] = conf["inboundsSetting"].clone();
            setting["outbounds"] = outbounds_setting.clone();
        },
        "trojan"=>{
            let Some(arg) = node.param.as_object() else {
                error!("{worker_name} node.param convert to hashmap failed!!! param={}", node.param);
                return None;
            };
            let mut outbounds_setting = conf["outboundsSetting_trojan"].clone();
            outbounds_setting[0]["settings"]["servers"][0]["address"] = Value::from(node.real_ip.clone());
            outbounds_setting[0]["settings"]["servers"][0]["port"] = Value::from(node.port);
            outbounds_setting[0]["settings"]["servers"][0]["password"] = arg["password"].clone();
            if arg.contains_key("allowInsecure"){
                outbounds_setting[0]["streamSettings"]["tlsSettings"]["allowInsecure"] = match arg.get("allowInsecure"){
                    Some(o)=>{
                        match o.as_str(){
                            Some(o)=>{
                                if o == "1" || o == "true" || o == "True"{
                                    // trace!("{} trojan got allowInsecure is true/True node={}, node.param={:?}", worker_name, node, arg);
                                    Value::from(true)
                                }
                                else{
                                    // trace!("{} trojan got allowInsecure is NOT true/True node={}, node.param={:?}", worker_name, node, arg);
                                    Value::from(false)
                                }
                            }
                            None=>Value::from(false)
                        }
                    }
                    None=>Value::from(false)
                }
            }

            setting = conf["conf_tpl"].clone();
            setting["inbounds"] = conf["inboundsSetting"].clone();
            setting["outbounds"] = outbounds_setting.clone();
        },
        "ss"=>{
            let Some(arg) = node.param.as_object() else {
                error!("{worker_name} node.param convert to hashmap failed!!! param={}", node.param);
                return None;
            };
            let mut outbounds_setting = conf["outboundsSetting_ss"].clone();
            // outbounds_setting[0]["settings"]["servers"][0]["address"] = Value::from(node.real_ip.clone());
            outbounds_setting[0]["settings"]["servers"][0]["address"] = Value::from(node.ip.clone());
            outbounds_setting[0]["settings"]["servers"][0]["method"] = arg["method"].clone();
            outbounds_setting[0]["settings"]["servers"][0]["password"] = arg["password"].clone();
            outbounds_setting[0]["settings"]["servers"][0]["port"] = Value::from(node.port);

            setting = conf["conf_tpl"].clone();
            setting["inbounds"] = conf["inboundsSetting"].clone();
            setting["outbounds"] = outbounds_setting.clone();
        }
        _=>{
            warn!("{worker_name} unsupport type {}", node.proto);
            setting = Value::Null;
        }
    }

    if measure_mode && !setting.is_null(){  // 测试模式
        // debug!("{} in measure_mode, adjust inbounds ...\norg: {}", worker_name, serde_json::to_string_pretty(&setting["inbounds"]).unwrap_or_default());
        match setting["inbounds"].as_array_mut(){
            Some(o)=>{
                o.swap_remove(0);
                o.swap_remove(0);
                o.remove(1);
                o[0]["listen"] = json!("127.0.0.1");  // 测试模式下只使用本地回环地址
                o[0]["port"] = json!(port);
                setting["routing"] = json!(
                    {"domainStrategy": "AsIs",
                    "domainMatcher": "mph",
                    "rules": [],
                    "balancers": []
                    });

                // trace!("{} in measure_mode, inbounds adjust to: {}", worker_name, serde_json::to_string_pretty(&setting["inbounds"]).unwrap_or_default());
            },
            None=>{
                error!("{worker_name} in measure_mode, can't convert inbounds as array_mut");
            }
        }
    }
    if !measure_mode && !setting.is_null(){  // 正式模式
        if conf["proxy_output_file"] != Value::Null{  // v2ray的日志输出
            setting["log"]["access"] = conf["proxy_output_file"].clone();
            setting["log"]["error"] = conf["proxy_output_file"].clone();
            setting["log"]["loglevel"] = conf["proxy_output_file_loglevel"].clone();
            debug!("{worker_name} proxy log set to {}", setting["log"]["access"]);
        }else{
            debug!("{worker_name} proxy log not set");
        }
    }

    if !setting.is_null(){
        let s = serde_json::to_string_pretty(&setting).unwrap_or_else(|e|{error!("{worker_name} convert to json failed! {e}"); String::new()});
        // trace!("{} config to send: {}", worker_name, serde_json::to_string_pretty(&setting).unwrap_or_default());
        return Some(s);
    }
    None
}