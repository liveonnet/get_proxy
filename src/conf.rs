use log::error;
use serde_json::Value;  
use std::env;
use crate::tools;
use crate::tools::get_lan_ip;

pub fn load_config(file_path: &str) -> Option<Value>{
    let Some(mut conf) = load_conf_file(file_path) else {
        return None;
    };
    let ip = get_lan_ip();
    conf["proxy_host"] = Value::String(ip);
    assert_eq!(conf["inboundsSetting"][0]["tag"], Value::String(String::from("http")));
    conf["inboundsSetting"][0]["listen"] = conf["proxy_host"].clone();
    conf["inboundsSetting"][0]["port"] = conf["proxy_port"].clone();
    conf["inboundsSetting"][0]["settings"]["accounts"][0]["user"] = conf["proxy_user"].clone();
    conf["inboundsSetting"][0]["settings"]["accounts"][0]["pass"] = conf["proxy_pass"].clone();

    assert_eq!(conf["inboundsSetting"][1]["tag"], Value::String(String::from("socks")));
    conf["inboundsSetting"][1]["listen"] = conf["proxy_host"].clone();
    conf["inboundsSetting"][1]["port"] = conf["proxy_socks_port"].clone();
    conf["inboundsSetting"][1]["settings"]["accounts"][0]["user"] = conf["proxy_user"].clone();
    conf["inboundsSetting"][1]["settings"]["accounts"][0]["pass"] = conf["proxy_pass"].clone();

    assert_eq!(conf["inboundsSetting"][2]["tag"], Value::String(String::from("http_noauth")));
    conf["inboundsSetting"][2]["listen"] = conf["proxy_host"].clone();
    conf["inboundsSetting"][2]["port"] = Value::Number((conf["proxy_port"].as_u64().unwrap_or_default() + 1).into());

    assert_eq!(conf["inboundsSetting"][3]["tag"], Value::String(String::from("socks_noauth")));
    conf["inboundsSetting"][3]["listen"] = conf["proxy_host"].clone();
    conf["inboundsSetting"][3]["port"] = Value::Number((conf["proxy_socks_port"].as_u64().unwrap_or_default() + 1).into());

    // 目前为止这个配置项没有使用
    conf["proxies"]["http://"] = Value::String(format!("http://{}:{}/", conf["proxy_host"].as_str().unwrap_or_default(), conf["proxy_port"].as_u64().unwrap_or_default()).to_string());

    if env::consts::OS == "windows"{
        conf["v2ray_path"] = conf["v2ray_path_win"].clone();
        conf["conf_basepath"] = conf["conf_basepath_win"].clone();
        conf["qqwry_path"] = conf["qqwry_path_win"].clone();
        conf["pac_server_file"] = conf["pac_server_file_win"].clone();
        conf["proxy_output_file"] = conf["proxy_output_file_win"].clone();
    }else{
        conf["v2ray_path"] = conf["v2ray_path_linux"].clone();
        conf["conf_basepath"] = conf["conf_basepath_linux"].clone();
        conf["qqwry_path"] = conf["qqwry_path_linux"].clone();
        conf["pac_server_file"] = conf["pac_server_file_linux"].clone();
        conf["proxy_output_file"] = conf["proxy_output_file_linux"].clone();
    };

    return Some(conf);
}

fn load_conf_file(file_path: &str) -> Option<Value>{
    match tools::read_file(file_path){
        Some(s) => {
            let ret: Value = match serde_json::from_str(s.as_str()){
                Ok(s)=>s,
                Err(e)=>{
                    error!("config json load failed {e}");
                    return None;
                }
            };
            return Some(ret);
        }
        None => {return None;}
    }
}
