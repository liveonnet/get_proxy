mod qqwry;
mod proto;
mod node;
mod tools;
mod launcher;
mod conf;

use node::Node;
use log::{debug, info, warn, error, trace};
use log::LevelFilter;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::JoinSet;
use tokio::time::timeout;

#[cfg(windows)]
use tokio::signal::windows::ctrl_c;

#[cfg(unix)]
use tokio::signal;

use async_channel::{bounded, Receiver, Sender};
use async_priority_channel;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::error;
use std::env;
// use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io;
use std::io::{stdin,stdout};
// use std::io::Read;
use std::io::Write as io_write;
use std::net::ToSocketAddrs;
use std::time::{SystemTime, UNIX_EPOCH};
use std::result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;
use std::time::Duration;
use bytes::BytesMut;
// use std::path::{Path, PathBuf};
use std::path::Path;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use serde_json::Value;  
use clap::{Command, Arg, value_parser};
use reqwest::header;
use reqwest::header::{HeaderName, HeaderValue};
use chrono::{self, Datelike, Local};
use rand::seq::SliceRandom;
use tokio::net::TcpListener;
use futures::{
    channel::mpsc::channel,
    SinkExt,
};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use dnsclientx::DNSClient;
use regex::Regex;

type MyResult<T> = result::Result<T, Box<dyn error::Error>>;

#[allow(dead_code)]
async fn test() {
    // let _ = proto::get_tolinkshare_data("test").await;
    // let _ = proto::get_vpnnet_data("test").await;

    // let _ = proto::get_mibei_url("test").await;
    // let v2 = PathBuf::from("D:/app/v2rayN-With-Core/bin/v2fly_v5/v2ray.exe").as_path().to_str().unwrap_or_default().to_string();
    // let co = PathBuf::from("d:/vmess_104.21.23.231.json").as_path().to_str().unwrap_or_default().to_string();

    // match tokio::process::Command::new(v2)
    //     .arg("run")
    //     .arg("-config")
    //     .arg(co)
    //     .spawn(){

    // match tokio::process::Command::new(format!("{} run -config {}",v2.as_path().to_str().unwrap_or_default(), co.as_path().to_str().unwrap_or_default())).spawn(){
    //         Ok(child)=>{
    //             tokio::time::sleep(Duration::from_millis(300)).await;
    //             info!("ok, {}", child.id().unwrap());
    //         },
    //         Err(err)=>{
    //             error!("spawn failed! {}", err);
    //         }
    // }
    // return;

    // let path = Path::new("d:/t.yaml");
    // let mut s = String::new();
    // if let Ok(mut f) = File::open(&path){
    //     f.read_to_string(&mut s).unwrap_or_default();
    // }
    // debug!("read {} bytes from file", s.len());
    // proto::parse_yaml(&String::from("fake"), &s, &String::from("fake_url"), &Value::Null).await;

    if false{
    let mut wry = qqwry::QQWry::from(String::from("d:/vsprj/test_rust/get_proxy/qqwry.dat"));
    for _ip in ["221.165.136.149", "192.168.256.1", "1.2.3.4", "172.64.134.48"]{
        let loc = wry.read_ip_location(_ip).await;
        debug!("{} [{}, {}] got {} {}", _ip, qqwry::u32_to_ip(loc.start_ip), qqwry::u32_to_ip(loc.end_ip), loc.country, loc.area);
    }
    }

    // 多线程，互斥信号
    if false{
    let counter = Arc::new(Mutex::new(0));
    let mut handles = vec![];
    for _ in 0..10 {
        let counter = Arc::clone(&counter);
        let handle = thread::spawn(move || {
            let mut num = counter.lock().unwrap();
            *num += 1;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
    println!("Reuslt: {}", *counter.lock().unwrap());
    }

}

#[tokio::main]
async fn main() {
    let matches = Command::new("get_proxy")
        .version("0.2")
        .author("kevin")
        .about("pick a fast proxy")
        .arg(Arg::new("in_measure_mode")
            .short('t')
            .long("measure_mode")
            .default_value("false")
            .default_missing_value("true")
            .value_parser(value_parser!(bool))
            .num_args(0..=1)
            .help("in measure_mode, just picking, not launch"))
        .arg(Arg::new("pick_one")
            .short('p')
            .long("pick_one")
            .default_value("false")
            .default_missing_value("true")
            .value_parser(value_parser!(bool))
            .num_args(0..=1)
            .help("pick_one need user select one url to get node at startup"))
        .arg(Arg::new("config_file")
            .short('c')
            .long("conf_file")
            .required(true)
            .help("specify a conf file path"))
        .arg(Arg::new("log_level")
            .short('d')
            .long("log_level")
            .default_value("trace")
            .default_missing_value("info")
            .value_parser(clap::builder::PossibleValuesParser::new(["trace", "debug", "info", "error"]))
            .num_args(0..=1)
            .help("set debug log output level"))
        .get_matches();

    let mut lb = env_logger::Builder::new();
    lb.format(|buf, record| {
            // let tim = buf.timestamp_millis();
            // let tim = buf.timestamp_seconds();
            let tim = Local::now().format("%Y-%m-%d_%H:%M:%S");
            writeln!(
                buf,
                "{tim}|{:.1}|{}|{}:{}",
                record.level(),
                record.module_path_static().unwrap(),
                record.line().unwrap(),
                record.args()
            )
        })
        // .filter(None, LevelFilter::Trace)  // level for my module
        .filter_module("reqwest", LevelFilter::Info) // level for reqwest
        .filter_module("rustls", LevelFilter::Info) // level for rustls
        .filter_module("html5ever", LevelFilter::Info)
        .filter_module("selectors", LevelFilter::Info)
        .filter_module("mio_pool", LevelFilter::Info)
        .filter_module("mio", LevelFilter::Info)
        .filter_module("mio::pool", LevelFilter::Info)
        // .format_timestamp_micros()
        .format_indent(Some(2));

    match matches.get_one::<String>("log_level").unwrap().as_str(){
        "trace"=>{ lb.filter_level(LevelFilter::Trace); trace!("now log level is trace");},
        "debug"=>{ lb.filter_level(LevelFilter::Debug); debug!("now log level is debug");},
        "info"=>{ lb.filter_level(LevelFilter::Info); info!("now log level is info");},
        "error"=>{ lb.filter_level(LevelFilter::Error); error!("now log level is error");},
        _=>{error!("unsupport log level!!!");}
    }
    lb.init();
    trace!("this is trace log");
    debug!("this is debug log");
    info!("this is info log");

    let machine_kind = if cfg!(unix) {
        "unix"
    } else if cfg!(windows) {
        "windows"
    } else {
        "unknown"
    };
    info!("running on {} os.", machine_kind);

    let measure_mode = matches.get_one::<bool>("in_measure_mode").unwrap();
    if *measure_mode{
        info!("in measure mode");
    }else{
        info!("in prod mode");
    }

    let pick_one = matches.get_one::<bool>("pick_one").unwrap();
    if *pick_one{
        info!("in pick_one mode");
    }

    // test().await;
    // return;

    let conf:Value = conf::load_config(matches.get_one::<String>("config_file").unwrap()).unwrap_or_default();
    let conf_string = serde_json::to_string_pretty(&conf).unwrap_or_default();
    trace!("conf content:\n{}", conf_string);
    let e_exit: Arc<Notify> = Arc::new(Notify::new());
    let e_ip_changed: Arc<Notify> = Arc::new(Notify::new());
    let conf_c = Arc::new(conf);

    // 监听程序退出信号
    tokio::spawn(ctrl_c_handler(e_exit.clone()));

    // 开启 pac 服务
    tokio::spawn(pac_server(conf_c.clone(), e_ip_changed.clone(), e_exit.clone()));
    let _ = do_all(conf_c.clone(), &e_ip_changed, &e_exit, *measure_mode, *pick_one).await;
    info!("exit.");
}

async fn ctrl_c_handler(e_exit: Arc<Notify>){
    let mut now: time::Instant;
    loop {
        now = time::Instant::now();

        #[cfg(windows)]
        let mut s = ctrl_c().unwrap();
        #[cfg(windows)]
        info!("wait for ctrl_c ...");
        #[cfg(windows)]
        s.recv().await.unwrap();

        #[cfg(unix)]
        info!("wait for ctrl_c ...");
        #[cfg(unix)]
        signal::ctrl_c().await.unwrap();

        // e_exit.notify_waiters();
        if now.elapsed() < time::Duration::from_secs(1) {
            info!("should exit");
            e_exit.notify_waiters();
            // break;
        } else {
            info!("press ctrl+c repeatly in 1 sec to exit.");
            continue;
        }
    }
}

// 处理过程：
// url -从链接取数据-> data -解析数据构造Node节点-> node -对节点中的域名进行解析获取ip地址-> ip -根据节点ip获取地区信息并过滤-> area -对节点进行排重-> uniq -对节点进行tcp链接测试-> good -对节点进行真实连接进行测试-> candidate -做为备用项使用
async fn do_all(conf: Arc<Value>, e_ip_changed: &Arc<Notify>, e_exit: &Arc<Notify>, measure_mode: bool, pick_one: bool) -> MyResult<String> {
    let b_stop_process = Arc::new(RwLock::new(false));
    let (url_out, url_in) = bounded(5);
    let (data_out, data_in) = bounded(5);
    let (node_out, node_in) = bounded(50);  // todo: 调整容量
    let (ip_out, ip_in) = bounded(50);
    let (area_out, area_in) = bounded(50);
    let (uniq_out, uniq_in) = bounded(50);  // todo: 调整容量
    let (good_out, good_in) = async_priority_channel::bounded::<Node, u32>(50); // todo: 调整容量
    let max_candidate: u64 = conf["candidate_max"].as_u64().unwrap();
    let (candidate_out, candidate_in) = async_priority_channel::bounded::<Node, u32>(max_candidate);
    let e_clear = Arc::new(Notify::new());

    let e_candidate_change = Arc::new(Notify::new());

    let _ = tokio::spawn(dispatch(conf.clone(), url_in.clone(), url_out.clone(),
                                    data_in.clone(), data_out.clone(),
                                    node_in.clone(),
                                    uniq_in.clone(),
                                    good_in.clone(),
                                    candidate_in.clone(),
                                    e_candidate_change.clone(),
                                    e_clear.clone(), e_exit.clone(),
                                    b_stop_process.clone(),
                                    measure_mode,
                                    pick_one
                                )
                        );

    // let nr_workers = num_cpus::get() * 2;
    // let num_threads = std::env::var("NUM_THREADS").unwrap_or("4".to_string()).parse::<usize>().unwrap();
    // 设置线程数，通常与CPU核心数相同
    let nr_workers = env::var("NUM_THREADS")
        .unwrap_or_else(|_| "4".to_string())
        .parse::<usize>()?;
    info!("use {} worker(s)", nr_workers);

    #[cfg(unix)]
    {
    let distributor_id = tools::get_distributor_id().await;
    info!("distributor_id={distributor_id}");
    }

    let mut st_u2d = JoinSet::new();
    for i in 1..=nr_workers{
        st_u2d.spawn(url2data(i, url_in.clone(), data_out.clone(), e_exit.clone(), b_stop_process.clone()));
    }

    let mut st_d2n = JoinSet::new();
    for i in 1..=nr_workers {
        st_d2n.spawn(data2node(i, data_in.clone(), node_out.clone(), e_exit.clone(), b_stop_process.clone()));
    }

    let mut st_n2i = JoinSet::new();
    for i in 1..=nr_workers {
        st_n2i.spawn(node2ip(i, node_in.clone(), ip_out.clone(), e_exit.clone(), b_stop_process.clone()));
    }

    let _ = tokio::spawn(ip2area(1, ip_in.clone(), area_out.clone(), e_exit.clone(), b_stop_process.clone(), conf.clone()));

    if false{ // debugonly
    if let Some(s) = tools::read_file("d:/tmp.txt"){
        if let Err(e) = data_out.send((String::from("xxx"), s)).await {
            debug!("putting data got err {e}");
        }
    }
    }

    let _ = tokio::spawn(uniq(area_in.clone(), uniq_out.clone(), e_clear.clone(), e_exit.clone(), b_stop_process.clone()));
    tokio::spawn(filter_bad_ip(uniq_in.clone(), good_out.clone(), e_exit.clone(), b_stop_process.clone()));
    tokio::spawn(measure_node(good_in.clone(), candidate_out.clone(), e_candidate_change.clone(), e_exit.clone(), conf.clone(), b_stop_process.clone()));

    service(conf.clone(), candidate_in.clone(), e_candidate_change.clone(), e_ip_changed.clone(), e_exit.clone(), measure_mode).await;

    debug!("do_all done!!!!");
    Ok(String::from("done."))
}

async fn url2data(idx: usize, url_in: Receiver<String>, data_out: Sender<(String, String)>, e_exit: Arc<Notify>, b_stop: Arc<RwLock<bool>>) -> () {
    let worker_name = format!("[u2d-{idx}]");
    let mut exit_flag = false;
    while !exit_flag{
        tokio::select! {
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(url) = url_in.recv() =>{
                {
                    let b_stop = b_stop.read().await;
                    if *b_stop {
                        trace!("{worker_name} b_stop, not process data");
                        continue;
                    }
                }
                match reqwest::get(&url).await {
                    Ok(it) => {
                        if let Ok(s) = it.text().await {
                            trace!("{worker_name} got {} byte(s) data from {url}", s.len());
                            if let Err(e) = data_out.send((url, s)).await {
                                debug!("{worker_name} putting data got err {e}");
                            }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                        }
                    },
                    Err(err) => {
                        trace!("{worker_name} fetching {url} got err {err}");
                    }
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn data2node(idx: usize, data_in: Receiver<(String, String)>, node_out: Sender<Node>, e_exit: Arc<Notify>, b_stop: Arc<RwLock<bool>>) -> () {
    let worker_name = format!("[d2n-{idx}]");

    let mut exit_flag = false;
    while !exit_flag{
        tokio::select!{
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok((url, data)) = data_in.recv() =>{
                {
                    let b_stop = b_stop.read().await;
                    if *b_stop {
                        trace!("{worker_name} b_stop, not process data");
                        continue;
                    }
                }

                if url.ends_with(".yaml") || url.ends_with(".yml") {
                    proto::parse_yaml(&worker_name, &data, &url, &node_out).await;
                } else{
                    proto::data2node_one(&worker_name, &data, &url, &node_out).await;
                }

                {
                    let b_stop = b_stop.read().await;
                    if *b_stop {
                        debug!("{worker_name} b_stop, not put node");
                        continue;
                    }
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn node2ip(idx: usize, node_in: Receiver<Node>, ip_out: Sender<Node>, e_exit: Arc<Notify>, b_stop: Arc<RwLock<bool>>) -> () {
    let worker_name = format!("[data2ip-{idx}]");
    let mut exit_flag = false;
    let l_nameserver = vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53), SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4)), 53)];
    let mut dns = DNSClient::new(l_nameserver);
    dns.set_timeout(Duration::from_secs(5));
    while !exit_flag{
        tokio::select!{
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(mut _node) = node_in.recv() =>{
                {
                    let b = b_stop.read().await;
                    if *b{
                        trace!("{worker_name} b_stop, not process node");
                        continue;
                    }
                }

                let mut _n: Node  = _node;
                let ip: IpAddr = _n.ip.parse().unwrap_or_else(|_|IpAddr::V4(Ipv4Addr::new(0,0,0,0)));
                if ip.is_unspecified(){
                    let l_real_ip = dns.query_a(_n.ip.as_str()).await.unwrap_or_default();
                    if !l_real_ip.is_empty(){
                        _n.real_ip = l_real_ip[0].to_string();
                        if let Err(e) = ip_out.send(_n).await {
                            debug!("{worker_name} putting data got {e}");
                        }
                    }
                    // else{
                    //     // debug!("{} can\'t get ip from {}", worker_name, _n.ip);
                    // }
                }
                else{
                    _n.real_ip = _n.ip.clone();
                    if let Err(e) = ip_out.send(_n).await {
                        debug!("{worker_name} putting data got {e}");
                    }
                }

            }
        }
    }

    info!("{worker_name} done.");
}

async fn ip2area(idx: usize, ip_in: Receiver<Node>, area_out: Sender<Node>, e_exit: Arc<Notify>, b_stop: Arc<RwLock<bool>>, conf: Arc<Value>) -> () {
    let worker_name = format!("[ip2area-{idx}]");
    let mut exit_flag = false;
    let p_area = Regex::new("日本|美国|新加坡|台湾|澳大利亚|韩国|德国|加拿大").unwrap();
    let mut wry = qqwry::QQWry::from(conf["qqwry_path"].as_str().unwrap_or_default().to_string());
    while !exit_flag{
        tokio::select!{
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(mut _node) = ip_in.recv() =>{
                {
                    let b = b_stop.read().await;
                    if *b{
                        trace!("{worker_name} b_stop, not process node");
                        continue;
                    }
                }

                let loc = wry.read_ip_location(_node.real_ip.as_str()).await;
                if loc.country.is_empty() {
                    if !_node.real_ip.contains(':'){
                        warn!("{worker_name} ip to area failed for {}", _node.real_ip);
                    }else{
                        // 带冒号的认为是ipv6，直接认为是允许的area
                        _node.area = String::new();
                        if let Err(e) = area_out.send(_node).await {
                            debug!("{worker_name} putting data got {e}");
                        };
                    }
                    continue;
                }
                if p_area.is_match(loc.country.as_str()) {
                    _node.area = loc.country.clone();
                    if let Err(e) = area_out.send(_node).await {
                        debug!("{worker_name} putting data got {e}");
                        break;
                    };
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn uniq(area_in: Receiver<Node>, uniq_out: Sender<Node>, e_clear: Arc<Notify>, e_exit: Arc<Notify>, b_stop: Arc<RwLock<bool>>) -> () {
    let worker_name = "[uniq]";
    let mut st = HashSet::new();
    let mut exit_flag = false;
    while !exit_flag{
        tokio::select!{
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            _ = e_clear.notified() =>{
                st.clear();
            },
            Ok(_node) = area_in.recv() =>{
                {
                    let b = b_stop.read().await;
                    if *b{
                        trace!("{worker_name} b_stop, not process node");
                        continue;
                    }
                }
                let mut state = DefaultHasher::new();
                Hash::hash(&_node, &mut state);
                let _hash_value = state.finish();
                if !st.contains(&_hash_value){
                    st.insert(_hash_value);
                    if let Err(e) = uniq_out.send(_node).await {
                        debug!("{worker_name} putting data got {e}");
                    };
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    // debug!("{} put node(s) done.", worker_name);
                }else{
                    // debug!("{} got dup node {} hash:{}", worker_name, _node, _hash_value);
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn filter_bad_ip(uniq_in: Receiver<Node>, good_out: async_priority_channel::Sender<Node, u32>, e_exit: Arc<Notify>, b_stop: Arc<RwLock<bool>>) -> () {
    let worker_name = "[filter]";
    let mut tasks = FuturesUnordered::new();  // 控制同时存在的协程数量
    const MAX_CONCURRENT_TASKS: usize = 200;

    let mut exit_flag= false;
    while !exit_flag{
        tokio::select!{
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(_node) = uniq_in.recv() =>{
                {
                    let b = b_stop.read().await;
                    if *b{
                        trace!("{worker_name} b_stop, not process node");
                        continue;
                    }
                }

                let tx = good_out.clone();
                let cur_b_stop = b_stop.clone();
                let task = tokio::spawn(async move{
                    let addr = format!("{}:{}", _node.real_ip, _node.port).to_socket_addrs().unwrap().next().unwrap();
                    let start = SystemTime::now();
                    match timeout(Duration::from_secs(3), TcpStream::connect(addr)).await{
                        Ok(Ok(mut stream)) => {
                            let _latency = SystemTime::now().duration_since(start).unwrap_or_default().as_millis();
                            // debug!("{} {}:{} accessable {}ms", worker_name, _node.ip, _node.port, _latency);
                            let _ = stream.shutdown().await;

                            {
                                let b = cur_b_stop.read().await;
                                if *b{
                                    debug!("{worker_name} b_stop, not put node");
                                    return;
                                }
                            }
                            if let Err(e) = tx.send(_node, 999999u32 - _latency as u32).await {
                                debug!("{worker_name} putting data got {e}");
                            };
                        },
                        Ok(Err(err)) => {
                            trace!("{worker_name} connect to {_node} failed {err}, addr={addr}");
                        },
                        Err(_e)=>{
                            // trace!("{} failed to connect to {} {}", worker_name, _node, _e);
                        }
                    }
                });

                tasks.push(task);
                if tasks.len() >= MAX_CONCURRENT_TASKS{
                    // trace!("{} reach max_concurrent_task, wait ...", worker_name);
                    let _ = tasks.select_next_some().await;
                    // trace!("{} wait done for max_concurrent_task", worker_name);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }

        }
    }

    info!("{worker_name} done.");
}

async fn measure_node(good_in: async_priority_channel::Receiver<Node, u32>, candidate_out: async_priority_channel::Sender<Node, u32>, e_candidate_change: Arc<Notify>, e_exit: Arc<Notify>, conf: Arc<Value>, b_stop_process: Arc<RwLock<bool>>) -> () {
    let worker_name = "[measure]";
    let start_port = conf["port_range"][0].as_u64().unwrap_or_default();
    let end_port = conf["port_range"][1].as_u64().unwrap_or_default();
    let max_concurrent = end_port - start_port + 1;
    let (port_tx, mut port_rx) = mpsc::channel(max_concurrent as usize);
    for _i in start_port..=end_port{
        let _ = port_tx.send(_i).await;
    }
    // set default headers
    let mut headers = header::HeaderMap::new();
    if let Value::Object(_m) = &conf["headers"]{
        for (_k, _v) in _m{
            let _h_k = HeaderName::from_bytes(_k.as_str().as_bytes()).unwrap_or_else(|e|{error!("{worker_name} convert key {} failed! {e}", _k.as_str()); HeaderName::from_static("")});
            let _h_v = HeaderValue::from_str(_v.as_str().unwrap_or_default()).unwrap_or_else(|e|{error!("{worker_name} convert value {} failed! {e}", _v.as_str().unwrap()); HeaderValue::from_static("")});
            headers.insert(_h_k, _h_v);
        }
    }else{
        error!("{worker_name} default headers set failed!");
    }

    let mut exit_flag = false;
    while !exit_flag{
        tokio::select! {
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok((_node, _)) = good_in.recv() =>{
                {
                    let b_stop = b_stop_process.read().await;
                    if *b_stop {
                        trace!("{worker_name} b_stop, not process good_in");
                        continue;
                    }
                }
                let tx = candidate_out.clone();
                let cur_conf = conf.clone();
                let cur_port: u64;
                match port_rx.recv().await{
                    Some(o) => {cur_port=o},
                    None => {
                        error!("{worker_name} rx recv got None !!! channel closed?");
                        break;
                    }
                }
                let cur_port_tx = port_tx.clone();
                let cur_good_in = good_in.clone();
                let cur_e_candidate_change = e_candidate_change.clone();
                let cur_headers = headers.clone();
                let cur_worker_name = format!("{}-{}", worker_name, cur_port);
                let cur_b_stop = b_stop_process.clone();
                let cur_e_exit = e_exit.clone();
                tokio::spawn(async move{
                    let (mut child, file_path) = match launcher::launcher_proxy(cur_worker_name.as_str(), &_node, &cur_conf, cur_port, true).await{
                        Some((a,b))=>{(a,b)},
                        None=>{
                            warn!("{cur_worker_name} launcher failed!!! {_node}");
                            let _ = cur_port_tx.send(cur_port).await;
                            return
                        }
                    };

                    // check again, debug_only
                    if let None = child.id(){
                        warn!("{cur_worker_name} proxy process exited?");
                    }

                    let cur_proxy = format!("http://127.0.0.1:{cur_port}");
                    trace!("{cur_worker_name} cur_proxy={cur_proxy} for {_node}");
                    let client = reqwest::Client::builder()
                        .proxy(reqwest::Proxy::all(cur_proxy).unwrap())
                        .default_headers(cur_headers)
                        .connect_timeout(Duration::from_secs(5))
                        .timeout(Duration::from_secs(5))
                        .build().unwrap_or_else(|e|{error!("client create failed! {e}"); reqwest::Client::builder().build().unwrap()});

                    let min_succ = 2;
                    let (nr_succ, min_latency) = tools::check_proxy(cur_worker_name.as_str(), &_node, &client, "https://www.youtube.com/@wangzhian/streams", 3, min_succ, false, cur_e_exit).await;
                    let mut b_stop_put = false;
                    {
                        let b_stop = cur_b_stop.read().await;
                        if *b_stop == true{
                            b_stop_put = true;
                            trace!("{cur_worker_name} b_stop, not put to candidate");
                        }
                    }

                    if !b_stop_put && nr_succ >= min_succ{
                        if min_latency < 1000{
                            info!("~~~~~~~~~~~~~~~~ {cur_worker_name} succ {min_latency}ms {} {_node} ~~~~~~~~~~~~~~~", _node.area);
                            if let Err(e) = tx.send(_node, 999999u32 - min_latency).await {
                                debug!("{cur_worker_name} putting data got {e}");
                            }else{
                                cur_e_candidate_change.notify_one();
                                debug!("{cur_worker_name} candidate now {}", tx.len());
                                if tx.is_full(){
                                    info!("{worker_name} candidate queue is full, stop processing.");
                                    let mut b = cur_b_stop.write().await;
                                    *b = true;
                                    while let Ok(_) = cur_good_in.try_recv(){};
                                }
                            };
                        }else{
                            debug!("~~~~~ {cur_worker_name} succ but slow {min_latency}ms {} {_node} ~~~~~", _node.area);
                            trace!("{cur_worker_name} drop slow proxy {min_latency}ms {_node}");
                        }
                        // match timeout(Duration::from_millis(20), cur_e_exit.notified()).await {
                        //     Ok(_) => {
                        //         warn!("{cur_worker_name} got exit flag!");
                        //         return;
                        //     },
                        //     Err(_) => {}
                        // }
                    }

                    if let Some(pid) = child.id(){
                        if let Err(e) = child.kill().await{
                            error!("{cur_worker_name} proxy {pid} kill failed! {e}");
                        };
                    }
                    if !file_path.is_empty(){
                        if let Err(e) = tokio::fs::remove_file(file_path).await{
                            error!("{cur_worker_name} file remove failed! {e}");
                        };
                    }

                    let _ = cur_port_tx.send(cur_port).await;
                });   // end of closure
            }
        }
    }

    info!("{worker_name} done.");
}

async fn service(conf: Arc<Value>, candidate_in: async_priority_channel::Receiver<Node, u32>, e_candidate_change: Arc<Notify>, e_ip_changed: Arc<Notify>, e_exit: Arc<Notify>, measure_mode: bool){
    let worker_name = "[service]";
    let mut pid: u32;
    let mut exit_flag = false;
    let mut cur_ip = tools::get_lan_ip();
    let mut mconf = Value::clone(&conf);
    // set default headers
    let mut headers = header::HeaderMap::new();
    if let Value::Object(_m) = conf["headers"].clone(){
        for (_k, _v) in _m{
            let _h_k = HeaderName::from_bytes(_k.as_str().as_bytes()).unwrap_or_else(|e|{error!("{worker_name} convert key {} failed! {e}", _k.as_str()); HeaderName::from_static("")});
            let _h_v = HeaderValue::from_str(_v.as_str().unwrap_or_default()).unwrap_or_else(|e|{error!("{worker_name} convert value {} failed! {e}",  _v.as_str().unwrap()); HeaderValue::from_static("")});
            headers.insert(_h_k, _h_v);
        }
    }else{
        error!("{worker_name} default headers set failed!");
    }
    let port: u64 = conf["proxy_port"].as_number().map_or_else(||0, |o|o.as_u64().unwrap_or_default());
    // set proxy
    let proxy = reqwest::Proxy::all(format!("http://{}:{port}", conf["proxy_host"].as_str().unwrap())).unwrap().basic_auth(conf["proxy_user"].as_str().unwrap(), conf["proxy_pass"].as_str().unwrap());
    debug!("{worker_name} proxy={proxy:?}");
    while !exit_flag{
        tokio::select! {
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok((_node, mut _prio)) = candidate_in.recv() =>{
                _prio = 999999u32 - _prio;
                debug!("{} pick {}ms {} {}", worker_name, _prio, _node.area, _node);
                e_candidate_change.notify_one();
                if measure_mode{
                    info!("{worker_name} in measure_mode, not launch service, sleep 120 sec ...");
                    tokio::select!{
                        _ = e_exit.notified() =>{
                            debug!("{worker_name} got exit flag");
                            exit_flag = true;
                        },
                        _ = tokio::time::sleep(Duration::from_secs(120)) =>{}
                    }
                    
                }else{
                    let ip = tools::get_lan_ip();  // 重新获取最新ip
                    if cur_ip != ip {
                        info!("ip changed {cur_ip} -> {ip}");
                        if ip == "0.0.0.0" {
                            warn!("network down ?");
                        } else if cur_ip == "0.0.0.0" {
                            warn!("network up ?");
                        }
                        e_ip_changed.notify_waiters();
                        cur_ip = ip;
                        mconf["proxy_host"] = Value::String(cur_ip.clone());
                        mconf["inboundsSetting"][0]["listen"] = Value::String(cur_ip.clone());
                        mconf["inboundsSetting"][1]["listen"] = Value::String(cur_ip.clone());
                        mconf["inboundsSetting"][2]["listen"] = Value::String(cur_ip.clone());
                        mconf["inboundsSetting"][3]["listen"] = Value::String(cur_ip.clone());
                        mconf["proxies"]["http://"] = Value::String(format!("http://{cur_ip}:{port}/"));
                    }
                    let (mut child, file_path) = match launcher::launcher_proxy(worker_name, &_node, &mconf, port, false).await{
                        Some((a,b))=>{(a,b)},
                        None=>{
                            error!("{worker_name} service launch proxy failed!!! {_node}");
                            continue;
                        }
                    };
                    pid = child.id().unwrap();
                    info!("{worker_name} service launched, {} {_node} {}, pid {pid}, conf {file_path}", _node.area, _node.alias);

                    let client = reqwest::Client::builder()
                        .proxy(proxy.clone())
                        .default_headers(headers.clone())
                        .connect_timeout(Duration::from_secs(5))
                        .timeout(Duration::from_secs(5))
                        .build().unwrap_or_else(|e|{error!("client create failed! {e}"); reqwest::Client::builder().build().unwrap()});

                    let mut show_raw = false;
                    loop{
                        let (nr_maxtry, min_succ) = (3, 2);
                        let (nr_succ, min_latency) = tools::check_proxy(worker_name, &_node, &client, "https://www.youtube.com/@wangzhian/streams", nr_maxtry, min_succ, false, e_exit.clone()).await;

                        if nr_succ >= min_succ && min_latency < 2500{
                            if !show_raw {
                                info!("{worker_name} node raw: {}  source: {}", _node.raw, _node.source);
                                show_raw = true;
                            }
                            print!("[{pid}_{min_latency}]");
                            let _ = io::stdout().flush();
                        }else{
                            let mut del_conf = true;
                            match child.try_wait() {
                                Ok(Some(status)) => {
                                    info!("{worker_name} process exited with {status}");
                                    del_conf = false;
                                },
                                Ok(None) => {
                                    // trace!("{worker_name} process exit status is not available now, still running");
                                },
                                Err(e) => {
                                    warn!("{worker_name} error try_wait process!!! {e}");
                                }
                            }

                            if let Err(e) = child.kill().await{
                                warn!("{worker_name} proxy {pid} kill failed! {e}");
                            }
                            if !file_path.is_empty(){
                                if del_conf {
                                    if let Err(e) = tokio::fs::remove_file(file_path).await{
                                        error!("{worker_name} file remove failed! {e}");
                                    };
                                } else {
                                    warn!("{worker_name} proxy exited unexpectly, keep conf file {file_path}");
                                }
                            }
                            info!("{worker_name} service test failed({nr_succ}/{nr_maxtry} {min_latency}ms) !!! picking new one");
                            match timeout(Duration::from_millis(10), e_exit.notified()).await {
                                Ok(_) => {
                                    warn!("{worker_name} got exit flag!");
                                    exit_flag = true;
                                },
                                Err(_) => {}
                            }
                            break;
                        }

                        tokio::select!{
                            _ = e_exit.notified() =>{
                                exit_flag = true;
                                debug!("{worker_name} got exit flag");

                                if let Err(e) = child.kill().await{
                                    error!("{worker_name} proxy {pid} kill failed! {e}");
                                }
                                if !file_path.is_empty(){
                                    if let Err(e) = tokio::fs::remove_file(file_path).await{
                                        error!("{worker_name} file remove failed! {e}");
                                    };
                                }
                                break;
                            }
                            _ = child.wait() => {
                                info!("{worker_name} service killed ?");
                                if !file_path.is_empty(){
                                    if let Err(e) = tokio::fs::remove_file(file_path).await{
                                        error!("{worker_name} file remove failed! {e}");
                                    };
                                }

                                match timeout(Duration::from_secs(1), e_exit.notified()).await{
                                    Ok(_) =>{
                                        warn!("{worker_name} got exit flag?");
                                        exit_flag = true;
                                    },
                                    Err(_) =>{
                                    }
                                }
                                break;
                            },
                            _ = tokio::time::sleep(Duration::from_secs(120)) => {},
                        }
                    }  // end of loop
                    if exit_flag {
                        info!("{worker_name} before pick new one, known exit_flag set.");
                        break;
                    }
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn dispatch(conf: Arc<Value>,
    url_in: Receiver<String>, url_out: Sender<String>,
    data_in: Receiver<(String, String)>, data_out: Sender<(String, String)>,
    node_in: Receiver<Node>,
    uniq_in: Receiver<Node>,
    good_in: async_priority_channel::Receiver<Node, u32>,
    candidate_in: async_priority_channel::Receiver<Node, u32>,
    e_candidate_change: Arc<Notify>,
    e_clear: Arc<Notify>, e_exit: Arc<Notify>,
    b_stop_process: Arc<RwLock<bool>>,
    measure_mode: bool,
    pick_one: bool) {
    let worker_name = "[dispatch]";

    info!("{}\n v2ray path: {}\n conf_basepath: {}\n proxy_host: {}\n pac_server_port: {}\n proxy_http_port: {}\n proxy_socks_port: {}\n proxy_http_port_noauth: {}\n proxy_socks_port_noauth: {}\n proxy_output_file: {}\n measure_mode: {}\n pick_one: {}",
        worker_name, conf["v2ray_path"],
        conf["conf_basepath"],
        conf["proxy_host"],
        conf["pac_server_port"],
        conf["inboundsSetting"][0]["port"],
        conf["inboundsSetting"][1]["port"],
        conf["inboundsSetting"][2]["port"],
        conf["inboundsSetting"][3]["port"],
        conf["proxy_output_file"],
        measure_mode,
        pick_one);

    // let mut recent_put = false;
    let min_candidate: u64 = conf["candidate_min"].as_u64().unwrap();
    let max_candidate: u64 = conf["candidate_max"].as_u64().unwrap();
    let mut time_add = SystemTime::now().checked_sub(Duration::from_secs(3600)).unwrap_or_else(||UNIX_EPOCH);
    let mut exit_flag = false;
    let mut pick_index: usize = usize::MAX;
    while !exit_flag{
        let recent_put = if SystemTime::now().duration_since(time_add).unwrap_or_default().as_secs() >= 90 { false } else { true };
        let remain = candidate_in.len();
        if remain <= min_candidate && !recent_put{
            if url_in.len() == 0 && data_in.len() == 0 && node_in.len() ==0 && uniq_in.len() == 0 && good_in.len() == 0 {
                {
                    let mut b_stop = b_stop_process.write().await;
                    *b_stop = false;
                }
                info!("{worker_name} few candidate {remain}, get more ...");
                e_clear.notify_waiters();
                let today = chrono::Local::now().date_naive();
                let s_today = today.format("%Y%m%d").to_string();
                let yesterday = today.checked_sub_days(chrono::Days::new(1)).unwrap_or_default();
                let s_yesterday = yesterday.format("%Y%m%d").to_string();
                let before_yesterday = yesterday.checked_sub_days(chrono::Days::new(1)).unwrap_or_default();
                let s_before_yesterday = before_yesterday.format("%Y%m%d").to_string();

                let mut urls = vec![
                    // format!("https://freenode.me/wp-content/uploads/{}/{:02}/{:02}{:02}.txt", today.year(), today.month(), today.month(), today.day()),
                    // format!("https://freenode.me/wp-content/uploads/{}/{:02}/{:02}{:02}.txt", yesterday.year(), yesterday.month(), yesterday.month(), yesterday.day()),
                    // format!("https://freenode.me/wp-content/uploads/{}/{:02}/{:02}{:02}.txt", before_yesterday.year(), before_yesterday.month(), before_yesterday.month(), before_yesterday.day()),
                    // format!("https://clashnode.com/wp-content/uploads/{}/{:02}/{}.txt", today.year(), today.month(), s_today),
                    // format!("https://clashnode.com/wp-content/uploads/{}/{:02}/{}.txt", yesterday.year(), yesterday.month(), s_yesterday),
                    format!("https://nodefree.org/dy/{}/{:02}/{}.txt", today.year(), today.month(), s_today),
                    format!("https://nodefree.org/dy/{}/{:02}/{}.txt", yesterday.year(), yesterday.month(), s_yesterday),
                    format!("https://nodefree.org/dy/{}/{:02}/{}.txt", before_yesterday.year(), before_yesterday.month(), s_before_yesterday),
                    format!("https://clashgithub.com/wp-content/uploads/rss/{}.txt", s_today),
                    format!("https://clashgithub.com/wp-content/uploads/rss/{}.txt", s_yesterday),
                    // format!("https://node.oneclash.cc/{}/{:02}/{}.txt", today.year(), today.month(), s_today),
                    // format!("https://node.oneclash.cc/{}/{:02}/{}.txt", yesterday.year(), yesterday.month(), s_yesterday),
                    format!("https://freeclash.org/wp-content/uploads/{}/{:02}/{}.txt", today.year(), today.month(), today.format("%m%d").to_string()),
                    format!("https://freeclash.org/wp-content/uploads/{}/{:02}/{}.txt", yesterday.year(), yesterday.month(), yesterday.format("%m%d").to_string()),
                    format!("https://nodebird.net/wp-content/uploads/{}/{}/{}.txt", today.year(), today.month(), s_today),
                    format!("https://nodebird.net/wp-content/uploads/{}/{}/{}.txt", yesterday.year(), yesterday.month(), s_yesterday),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/umelabs/node.umelabs.dev/master/Subscribe/v2ray.md"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/freefq/free/master/v2"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/ts-sf/fly/main/v2"),
                    String::from("https://cdn.jsdelivr.net/gh/ermaozi01/free_clash_vpn/subscribe/v2ray.txt"),
                    String::from("https://v2ray.neocities.org/v2ray.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Leon406/SubCrawler/master/sub/share/v2"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Leon406/SubCrawler/master/sub/share/tr"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/peasoft/NoMoreWalls/master/list.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/a2470982985/getNode/main/v2ray.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/ermaozi01/free_clash_vpn/main/subscribe/v2ray.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/ripaojiedian/freenode/main/sub"),

                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/learnhard-cn/free_proxy_ss/main/free"),
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Rokate/Proxy-Sub/main/clash/clash_v2ray.yml"),
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Rokate/Proxy-Sub/main/clash/clash_trojan.yml"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Jsnzkpg/Jsnzkpg/Jsnzkpg/Jsnzkpg"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/ZywChannel/free/main/sub"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/free18/v2ray/main/v2ray.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/mfuu/v2ray/master/v2ray"),

                    format!("https://node.onenode.cc/{}/{:02}/{}.txt", today.year(), today.month(), s_today),
                    format!("https://node.onenode.cc/{}/{:02}/{}.txt", yesterday.year(), yesterday.month(), s_yesterday),

                    String::from("https://jiang.netlify.com/"),
                    String::from("https://youlianboshi.netlify.app/"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/eycorsican/rule-sets/master/kitsunebi_sub"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/umelabs/node.umelabs.dev/master/Subscribe/v2ray.md"),

                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/w1770946466/Auto_proxy/main/Long_term_subscription_num"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/mahdibland/ShadowsocksAggregator/master/Eternity"),
                    format!("https://mirror.ghproxy.com/https://raw.githubusercontent.com/w1770946466/Auto_proxy/main/sub/{}/{}.txt", before_yesterday.format("%y%m").to_string(), s_before_yesterday),
                    format!("https://mirror.ghproxy.com/https://raw.githubusercontent.com/w1770946466/Auto_proxy/main/sub/{}/{}.txt", yesterday.format("%y%m").to_string(), s_yesterday),
                    format!("https://mirror.ghproxy.com/https://raw.githubusercontent.com/w1770946466/Auto_proxy/main/sub/{}/{}.txt", today.format("%y%m").to_string(), s_today),
                    // String::from("https://mareep.netlify.app/sub/merged_proxies_new.yaml"),  // https://github.com/vveg26/chromego_merge
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/a2470982985/getNode/main/v2ray.txt"),   // https://github.com/Flik6/getNode
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/freenodes/freenodes/main/clash.yaml"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Barabama/FreeNodes/master/nodes/yudou66.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Barabama/FreeNodes/master/nodes/blues.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Barabama/FreeNodes/master/nodes/zyfxs.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/mheidari98/.proxy/main/all"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/LalatinaHub/Mineral/master/result/nodes"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/zhangkaiitugithub/passcro/main/speednodes.yaml"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/zhangkaiitugithub/SubCrawler/main/sub/share/all"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/yebekhe/V2Hub/main/merged_base64"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Mahdi0024/ProxyCollector/master/sub/proxies.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/v2ray-configs/main/Splitted-By-Protocol/vmess.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/v2ray-configs/main/Splitted-By-Protocol/vless.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/v2ray-configs/main/Splitted-By-Protocol/ss.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/v2ray-configs/main/Splitted-By-Protocol/trojan.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/ALIILAPRO/v2rayNG-Config/main/server.txt"),

                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/bulk-xray-v2ray-vless-vmess-...-configs/main/sub/Japan/config.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/bulk-xray-v2ray-vless-vmess-...-configs/main/sub/United%20States/config.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/bulk-xray-v2ray-vless-vmess-...-configs/main/sub/Germany/config.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/bulk-xray-v2ray-vless-vmess-...-configs/main/sub/Singapore/config.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/bulk-xray-v2ray-vless-vmess-...-configs/main/sub/Austria/config.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Epodonios/bulk-xray-v2ray-vless-vmess-...-configs/blob/main/sub/Canada/config.txt"),

                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/zzz6839/SubCrawler/main/sub/share/all"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/zjfb/SubCrawler/main/sub/share/all"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/QQnight/SubCrawler/main/sub/share/all"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/mahdibland/ShadowsocksAggregator/master/sub/sub_merge.txt"),

                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/zjfb/SubCrawler/main/sub/share/all"),

                    String::from("https://url.cr/api/user.ashx?do=freevpn&ip=127.0.0.1&uuid=67ee96e3-70c5-4741-9105-60d7fd8c42b3"),

                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Mohammadgb0078/IRV2ray/main/vless.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Mohammadgb0078/IRV2ray/main/vmess.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/NiceVPN123/NiceVPN/main/long"),
                    format!("https://tglaoshiji.github.io/nodeshare/{}/{}/{}.txt", today.year(), today.month(), s_today),
                    format!("https://tglaoshiji.github.io/nodeshare/{}/{}/{}.txt", yesterday.year(), yesterday.month(), s_yesterday),
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/AzadNetCH/Clash/main/AzadNet_META_IRAN-Direct.yml"),  里面没有符合地区的
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/AzadNetCH/Clash/main/AzadNet_iOS.txt"),  里面没有符合地区的
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/vxiaov/free_proxies/main/clash/clash.provider.yaml"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Surfboardv2ray/TGParse/main/splitted/mixed"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Surfboardv2ray/Proxy-sorter/main/submerge/converted.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Surfboardv2ray/v2ray-worker-sub/master/sub"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Surfboardv2ray/Subs/main/Raw"),
                    String::from("https://tgscan.onrender.com/sub3"),
                    String::from("https://tgscan.onrender.com/sub5"),
                    String::from("https://tgscan.onrender.com/sub9/base64"),
                    String::from("https://tgscan.onrender.com/sub10/base64"),
                    String::from("https://sub.tgzdyz2.xyz/sub"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/iboxz/free-v2ray-collector/main/main/mix"),
                    String::from("https://www.xrayvip.com/free.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/ndsphonemy/proxy-sub/main/default.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Huibq/TrojanLinks/master/links/trojan"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Huibq/TrojanLinks/master/links/vmess"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Huibq/TrojanLinks/master/links/ss"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/hkpc/V2ray-Configs/main/All_Configs_base64_Sub.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/m3hdio1/v2ray_sub/main/v2ray_sub.txt"),
//                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/MrMohebi/xray-proxy-grabber-telegram/master/collected-proxies/clash-meta/all.yaml"), 
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/MrMohebi/xray-proxy-grabber-telegram/master/collected-proxies/row-url/all.txt"),
//                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/MrMohebi/xray-proxy-grabber-telegram/master/collected-proxies/row-url/actives.txt"),
                    String::from("https://uploadserver.sialkcable.ir/v2ray/config.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Everyday-VPN/Everyday-VPN/main/subscription/main.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Memory2314/VMesslinks/main/links/vmess"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/lagzian/SS-Collector/main/mix.txt"),
                    String::from("https://mirror.ghproxy.com/https://github.com/v2clash/V2ray-Configs/raw/main/All_Configs_Sub.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Ptechgithub/configs/main/clash12.yaml"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/cf.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/80.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/443.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/grpc.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/vmess.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/ws.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/vless.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/trojan.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/tls.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/dimzon/scaling-sniffle/main/freedom/tcp.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/itsyebekhe/HiN-VPN/main/subscription/base64/vless"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/itsyebekhe/HiN-VPN/main/subscription/base64/trojan"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/itsyebekhe/HiN-VPN/main/subscription/base64/vmess"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/itsyebekhe/HiN-VPN/main/subscription/base64/ss"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/abbasdvd3/clash/main/3.yaml"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/V2RAYCONFIGSPOOL/V2RAY_SUB/main/V2RAY_SUB.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Kwinshadow/TelegramV2rayCollector/main/sublinks/mix.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/MhdiTaheri/V2rayCollector_Py/main/sub/Mix/mix.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/MhdiTaheri/V2rayCollector/main/sub/mix"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/linzjian666/chromego_extractor/main/outputs/clash_meta.yaml"),

                    // 如下5个好像最终地址都一样
                    String::from("https://cdn.jsdelivr.net/gh/a2470982985/getNode@main/v2ray.txt"),    // https://github.com/John19187/v2ray-SSR-Clash-Verge-Shadowrocke
                    format!("https://good.john1959.com/v2ray@a/{}-list-1.txt", yesterday.format("%Y-%-m-%d").to_string()),  // https://github.com/John19187/v2ray-SSR-Clash-Verge-Shadowrocke
                    format!("https://good.john1959.com/v2ray@b/{}-list-2.txt", yesterday.format("%Y-%-m-%d").to_string()),  // https://github.com/John19187/v2ray-SSR-Clash-Verge-Shadowrocke
                    format!("https://good.john1959.com/v2ray@a/{}-list-1.txt", today.format("%Y-%-m-%d").to_string()),  // https://github.com/John19187/v2ray-SSR-Clash-Verge-Shadowrocke
                    format!("https://good.john1959.com/v2ray@b/{}-list-2.txt", today.format("%Y-%-m-%d").to_string()),  // https://github.com/John19187/v2ray-SSR-Clash-Verge-Shadowrocke
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/roosterkid/openproxylist/main/V2RAY_RAW.txt"),
                    String::from("https://q3dlaxpoaq.github.io/APIs/cg0.txt"),
                    String::from("https://q3dlaxpoaq.github.io/APIs/cg1.txt"),
                    String::from("https://q3dlaxpoaq.github.io/APIs/cg2.txt"),
                    String::from("https://q3dlaxpoaq.github.io/APIs/cg3.txt"),
                    String::from("https://q3dlaxpoaq.github.io/APIs/cg4.txt"),


                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/acymz/AutoVPN/main/data/V2.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/HakurouKen/free-node/main/public"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Q3dlaXpoaQ/V2rayN_Clash_Node_Getter/refs/heads/main/APIs/cg0.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Q3dlaXpoaQ/V2rayN_Clash_Node_Getter/refs/heads/main/APIs/cg1.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Q3dlaXpoaQ/V2rayN_Clash_Node_Getter/refs/heads/main/APIs/cg2.txt"),
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Q3dlaXpoaQ/V2rayN_Clash_Node_Getter/refs/heads/main/APIs/cg3.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/Q3dlaXpoaQ/V2rayN_Clash_Node_Getter/refs/heads/main/APIs/cg4.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub1.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub2.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub3.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub4.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub5.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub6.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub7.txt"),
                    String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub8.txt"),
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub9.txt"),
                    // String::from("https://mirror.ghproxy.com/https://raw.githubusercontent.com/nyeinkokoaung404/V2ray-Configs/main/Sub10.txt"),
                ];

                // mibei url
                if pick_one {
                    let mibei_url = proto::get_mibei_url(worker_name).await.unwrap_or_default();
                    if mibei_url.len() > 0 {
                        urls.push(mibei_url);
                        debug!("{worker_name} mibei url added to urls.");
                    }
                }else{
                    let url_out_c = url_out.clone();
                    tokio::spawn(async move{
                        let mibei_url = proto::get_mibei_url(worker_name).await.unwrap_or_default();
                        if mibei_url.len() > 0 {
                            if let Err(e) = url_out_c.send(mibei_url).await{
                                error!("{worker_name} put url failed!!! {e}");
                            }
                            debug!("{worker_name} mibei urls added.");
                        }
                    });
                }

                // shareclash urls
                if pick_one {
                    let shareclash_urls = proto::get_shareclash_url(worker_name).await.unwrap_or_default();
                    if shareclash_urls.len() > 0 {
                        for url in shareclash_urls {
                            urls.push(url);
                        }
                        debug!("{worker_name} shareclash urls added to urls.");
                    }
                }else{
                    let url_out_c = url_out.clone();
                    tokio::spawn(async move{
                        let shareclash_urls = proto::get_shareclash_url(worker_name).await.unwrap_or_default();
                        if shareclash_urls.len() > 0 {
                            for url in shareclash_urls {
                                if let Err(e) = url_out_c.send(String::from(url)).await{
                                    error!("{worker_name} put url failed!!! {e}");
                                }
                            }
                            debug!("{worker_name} shareclash urls added.");
                        }
                    });
                }

                // v2rayclashnode urls
                if pick_one {
                    let l_urls = proto::get_v2rayclashnode_url(worker_name).await.unwrap_or_default();
                    if l_urls.len() > 0 {
                        for url in l_urls {
                            urls.push(url);
                        }
                        debug!("{worker_name} v2rayclashnode urls added to urls.");
                    }
                }else{
                    let url_out_c = url_out.clone();
                    tokio::spawn(async move{
                        let l_urls = proto::get_v2rayclashnode_url(worker_name).await.unwrap_or_default();
                        if l_urls.len() > 0 {
                            for url in l_urls {
                                if let Err(e) = url_out_c.send(String::from(url)).await{
                                    error!("{worker_name} put url failed!!! {e}");
                                }
                            }
                            debug!("{worker_name} v2rayclashnode urls added.");
                        }
                    });
                }


                // 打乱顺序
                if !pick_one {
                    urls = tokio::task::block_in_place(||{
                        let mut url_dup = urls.clone();
                        let mut rng = rand::thread_rng();
                        url_dup.shuffle(&mut rng);
                        url_dup
                    });
                }

                // tolinkshare, vpnnet, sharkdoor data
                if !pick_one{
                    let data_out_c = data_out.clone();
                    tokio::spawn(async move {
                        let _d1 = proto::get_tolinkshare_data("test").await;
                        let _d2 = proto::get_vpnnet_data("test").await;
                        for _ret in [_d1, _d2] {
                            if let Some(_x) = _ret {
                                if let Err(e) = data_out_c.send(_x).await{
                                    error!("{worker_name} put data failed!!! {e}");
                                }
                            }
                        }

                        // 获取几个节点
                        let l_node = proto::get_sharkdoor_url(worker_name).await.unwrap_or_default();
                        if l_node.len() > 0 {
                            for node in l_node {
                                if let Err(e) = data_out_c.send(node).await{
                                    error!("{worker_name} put data failed!!! {e}");
                                }
                            }
                            debug!("{worker_name} sharkdoor urls added to urls.");
                        }
                    });
                }else{
                    warn!("in pick_one mode, tolinkshare and vpnnet skip processing !!!");
                }

                debug!("{worker_name} regular urls len {}", urls.len());
                if pick_one && pick_index == usize::MAX {
                    for (_i, _url) in urls.iter().enumerate() {
                        println!("{_i}: {_url}");
                    }
                    let mut s = String::new();
                    pick_index = loop {
                        s.clear();
                        print!("select by index(0-{}):", urls.len() - 1);
                        let _ = stdout().flush();
                        match stdin().read_line(&mut s){
                            Ok(_) => {
                                if let Ok(n) = s.trim().to_string().parse(){
                                    if n < urls.len() {
                                        break n;
                                    }
                                    println!("out of index!");
                                }else{
                                    println!("bad input!");
                                }
                            },
                            Err(e) => {
                                error!("got error {e}");
                            }
                        }
                    };
                }

                if pick_index != usize::MAX {
                    info!("the index {pick_index} is used: {}", urls[pick_index]);
                    if let Err(e) = url_out.send(String::from(urls[pick_index].clone())).await{
                        error!("{worker_name} put url failed!!! {e}");
                    }
                }else{
                    for url in urls {
                        if let Err(e) = url_out.send(String::from(url)).await{
                            error!("{worker_name} put url failed!!! {e}");
                        }
                    }
                }

                time_add = SystemTime::now();
            }else{
                trace!("{worker_name} in processing");
            }

        }else{
            if remain >= max_candidate{
                if measure_mode{
                    debug!("{worker_name} in measure_node not stop measure node, now {}", candidate_in.len());
                } else {
                    {
                        let mut b_stop = b_stop_process.write().await;
                        if !*b_stop {
                            info!("{worker_name} remain reach max_candidate, stop processing");
                            *b_stop = true;
                        }
                    }
                    if !url_in.is_empty() || !data_in.is_empty() || !node_in.is_empty() || !uniq_in.is_empty() || !good_in.is_empty() {
                        info!("{worker_name} candidate remain {remain}, clear other queue");
                        while let Ok(_) = url_in.try_recv(){};
                        while let Ok(_) = data_in.try_recv(){};
                        while let Ok(_) = node_in.try_recv(){};
                        while let Ok(_) = uniq_in.try_recv(){};
                        while let Ok(_) = good_in.try_recv(){};
                    }
                    else{
                        print!(".{}.", candidate_in.len());
                        let _ = io::stdout().flush();
                    }
                }
            }else{
                trace!("{worker_name} candidate now {}", candidate_in.len());
                print!("_{}_", candidate_in.len());
                let _ = io::stdout().flush();
            }
        }

        tokio::select!{
            _ = e_exit.notified() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            _ = e_candidate_change.notified() =>{
                debug!("{worker_name} candidate changed to {}", candidate_in.len());
            }
            _ = tokio::time::sleep(Duration::from_secs(if recent_put {120} else {60}))=> {
                // print!("Timed out.");
                // let _ = io::stdout().flush();
            }
            else =>{
                warn!("{worker_name} should not be here !!!");
            }

        }
    }

    info!("{worker_name} done.");
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, futures::channel::mpsc::Receiver<Event>)> {
    let (mut tx, rx) = channel(1);
    let watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            futures::executor::block_on(async {
                trace!("got event: {res:?}");
                if let Ok(o) = res {
                    #[cfg(windows)]
                    if o.kind == notify::EventKind::Modify(notify::event::ModifyKind::Any)
                    || o.kind == notify::EventKind::Create(notify::event::CreateKind::Any) {
                        tx.send(o).await.unwrap();
                    }

                    #[cfg(unix)]
                    if o.kind == notify::event::EventKind::Modify(notify::event::ModifyKind::Data(notify::event::DataChange::Any)) {
                    // || o.kind == notify::event::EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::To))
                    // || o.kind == notify::event::EventKind::Modify(notify::event::ModifyKind::Any)
                    // || o.kind == notify::event::EventKind::Create(notify::event::CreateKind::File){
                        tx.send(o).await.unwrap();
                    }
                }
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

async fn pac_server(conf: Arc<Value>, e_ip_changed: Arc<Notify>, e_exit: Arc<Notify>){
    let worker_name = "[pac]";
    let mut proxy_host = conf["proxy_host"].as_str().unwrap_or_default().to_string();  // 只作为初始的ip使用，后续如果ip变更，会动态获取新ip
    let proxy_port: u64 = conf["proxy_port"].as_u64().unwrap_or_default() + 1;
    let mut s_addr = format!("{}:{}", proxy_host, conf["pac_server_port"]);
    let mut addr = s_addr.to_socket_addrs().unwrap().next().unwrap();
    let file_path = conf["pac_server_file"].as_str().unwrap_or_default();
    let mut file_content = String::new();
    if let Some(mut s) = tools::read_file(file_path){
        trace!("{worker_name} file read from {file_path}");
        s = s.replace("{proxy_host}", proxy_host.as_str()).replace("{proxy_port}", proxy_port.to_string().as_str());
        trace!("{worker_name} {file_path}:\n{s}");
        file_content = s;
    } else {
        error!("{worker_name} file reload failed!!!");
    }

    let (mut watcher, mut rx) = async_watcher().unwrap();
    let watch_dir = Path::new(file_path).parent().unwrap();
    watcher.watch(watch_dir, RecursiveMode::NonRecursive).unwrap();
    let mut last_reload = time::Instant::now();

    let mut listener = TcpListener::bind(addr).await.unwrap();

    let mut exit_flag = false;
    info!("{worker_name} pac server started at {s_addr}");
    while !exit_flag {
        let cur_file_content = file_content.clone();
        tokio::select!{
            _ = e_exit.notified() =>{
                info!("{worker_name} got exit flag");
                exit_flag = true;
            },
            _ = e_ip_changed.notified() => {
                proxy_host = tools::get_lan_ip();
                s_addr = format!("{}:{}", proxy_host, conf["pac_server_port"]);
                addr = s_addr.to_socket_addrs().unwrap().next().unwrap();
                listener = TcpListener::bind(addr).await.unwrap();
                info!("{worker_name} notified ip changed, pac server listened on {s_addr}");
                // ip改变，则pac js里面的ip做同样的改变，目前没想好ip为0.0.0.0也就是断网的时候做什么处理
                if let Some(mut s) = tools::read_file(file_path){
                    s = s.replace("{proxy_host}", proxy_host.as_str()).replace("{proxy_port}", proxy_port.to_string().as_str());
                    file_content = s;
                    info!("{worker_name} file reloaded. {file_path}");
                    last_reload = time::Instant::now();
                } else {
                    error!("{worker_name} file reload failed!!!");
                }
            },
            Some(e) = rx.next() => {  // auto reload file on modification
                debug!("{worker_name} detected {:?} {:?}", e.kind, e.paths);
                if e.paths.len() > 0 && e.paths[0].as_path() == Path::new(file_path) && last_reload.elapsed() > time::Duration::from_secs(2) {
                    if let Some(mut s) = tools::read_file(file_path){
                        s = s.replace("{proxy_host}", proxy_host.as_str()).replace("{proxy_port}", proxy_port.to_string().as_str());
                        file_content = s;
                        info!("{worker_name} file reloaded. {file_path}");
                        last_reload = time::Instant::now();
                    } else {
                        error!("{worker_name} file reload failed!!!");
                    }
                }
            },
            Ok((mut stream, new_addr))  = listener.accept() =>{
                tokio::spawn(async move {
                    // parse http header
                    let mut buf = BytesMut::with_capacity(1024);
                    let read = stream.read_buf(&mut buf).await.unwrap();
                    let s = String::from_utf8(buf.to_vec()).unwrap_or_default();
                    trace!("{worker_name} got {read} byte(s) from {new_addr}, content:\n{s}");
                    let mut query_path = String::new();
                    for _line in s.lines() {
                        if _line.starts_with("GET ") {
                            let parts: Vec<&str> = _line.split_whitespace().collect();
                            query_path = String::from(parts[1]);
                            if query_path != "/" {  // 不是 '/' 就返回空内容
                                debug!("{worker_name} {} 200 OK to {new_addr:?} 0 bytes", parts[1]);
                                stream.write_all("HTTP/1.1 200 OK\r\nContent_type: text/plain\r\nContent-Length: 0\r\n\r\n".as_bytes()).await.unwrap();
                                stream.flush().await.unwrap();
                                return;
                            }
                            break;
                        }
                        break;  // only check the first line
                    }

                    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}", cur_file_content.len(), cur_file_content);
                    stream.write_all(response.as_bytes()).await.unwrap();
                    stream.flush().await.unwrap();
                    info!("{worker_name} {new_addr} {query_path} 200 OK {} bytes", cur_file_content.len());
                });
            }
        }
    }

    let _ = watcher.unwatch(file_path.as_ref());
}
