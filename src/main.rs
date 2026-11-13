mod conf;
mod launcher;
mod node;
mod proto;
mod qqwry;
mod tools;

use log::LevelFilter;
use log::{debug, error, info, trace, warn};
use node::Node;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Notify, RwLock};
use tokio::task::{Id, JoinSet};
use tokio::time::timeout;

#[cfg(windows)]
use tokio::signal::windows::ctrl_c;

#[cfg(unix)]
use tokio::signal;

use async_channel::{bounded, Receiver, Sender};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::error;
// use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io;
use std::io::{stdin, stdout};
// use std::io::Read;
use bytes::BytesMut;
use std::fmt::Write;
use std::io::Write as io_write;
use std::net::ToSocketAddrs;
use std::result;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
// use std::path::{Path, PathBuf};
use chrono::{self, Datelike, Local};
use clap::{value_parser, Arg, Command};
use dnsclientx::DNSClient;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use futures::{channel::mpsc::channel, SinkExt};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use rand::seq::SliceRandom;
use rand::thread_rng;
use regex::Regex;
use reqwest::header;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Client;
use serde_json::Value;
use socket2::{Domain, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

type MyResult<T> = result::Result<T, Box<dyn error::Error>>;

#[allow(dead_code)]
async fn test() {
    // let _ = proto::get_tolinkshare_data("test").await;
    // let _ = proto::get_vpnnet_data("test").await;

    let _s = proto::get_v2rayclashx_url("test", "v2rayclashx").await;
    if let Some((_source, _urls)) = _s {
        debug!("{_source} {} urls got: {_urls:?}", _urls.len());
    }
    // let _s = proto::get_v2ssr_url("test").await.unwrap_or_default();
    // if !_s.is_empty() {
    // info!("url is {_s}");
    // }
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

    if false {
        let mut wry = qqwry::QQWry::from(String::from("d:/vsprj/test_rust/get_proxy/qqwry.dat"));
        for _ip in [
            "221.165.136.149",
            "192.168.256.1",
            "1.2.3.4",
            "172.64.134.48",
        ] {
            let loc = wry.read_ip_location(_ip).await;
            debug!(
                "{} [{}, {}] got {} {}",
                _ip,
                qqwry::u32_to_ip(loc.start_ip),
                qqwry::u32_to_ip(loc.end_ip),
                loc.country,
                loc.area
            );
        }
    }

    // 多线程，互斥信号
    if false {
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
        .arg(
            Arg::new("in_measure_mode")
                .short('t')
                .long("measure_mode")
                .default_value("false")
                .default_missing_value("true")
                .value_parser(value_parser!(bool))
                .num_args(0..=1)
                .help("in measure_mode, just picking, not launch"),
        )
        .arg(
            Arg::new("pick_one")
                .short('p')
                .long("pick_one")
                .default_value("false")
                .default_missing_value("true")
                .value_parser(value_parser!(bool))
                .num_args(0..=1)
                .help("pick_one need user select one url to get node at startup"),
        )
        .arg(
            Arg::new("config_file")
                .short('c')
                .long("conf_file")
                .required(true)
                .help("specify a conf file path"),
        )
        .arg(
            Arg::new("log_level")
                .short('d')
                .long("log_level")
                .default_value("trace")
                .default_missing_value("info")
                .value_parser(clap::builder::PossibleValuesParser::new([
                    "trace", "debug", "info", "error",
                ]))
                .num_args(0..=1)
                .help("set debug log output level"),
        )
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
    .filter_module("tungstenite::handshake::client", LevelFilter::Info)
    // .format_timestamp_micros()
    .format_indent(Some(2));

    match matches.get_one::<String>("log_level").unwrap().as_str() {
        "trace" => {
            lb.filter_level(LevelFilter::Trace);
            trace!("now log level is trace");
        }
        "debug" => {
            lb.filter_level(LevelFilter::Debug);
            debug!("now log level is debug");
        }
        "info" => {
            lb.filter_level(LevelFilter::Info);
            info!("now log level is info");
        }
        "error" => {
            lb.filter_level(LevelFilter::Error);
            error!("now log level is error");
        }
        _ => {
            error!("unsupport log level!!!");
        }
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
    if *measure_mode {
        info!("in measure mode");
    } else {
        info!("in prod mode");
    }

    let pick_one = matches.get_one::<bool>("pick_one").unwrap();
    if *pick_one {
        info!("in pick_one mode");
    }

    // test().await;
    // return;

    let conf: Value =
        conf::load_config(matches.get_one::<String>("config_file").unwrap()).unwrap_or_default();
    let conf_string = serde_json::to_string_pretty(&conf).unwrap_or_default();
    trace!("conf content:\n{}", conf_string);
    let e_ip_changed: Arc<Notify> = Arc::new(Notify::new());
    let conf_c = Arc::new(conf);

    let e_exit = CancellationToken::new();
    // 监听程序退出信号
    tokio::spawn(ctrl_c_handler(e_exit.clone()));

    // 开启 pac 服务
    tokio::spawn(pac_server(
        conf_c.clone(),
        e_ip_changed.clone(),
        e_exit.clone(),
    ));
    let _ = do_all(
        conf_c.clone(),
        &e_ip_changed,
        &e_exit,
        *measure_mode,
        *pick_one,
    )
    .await;
    info!("exit.");
}

async fn ctrl_c_handler(e_exit: CancellationToken) {
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

        if now.elapsed() < time::Duration::from_secs(1) {
            info!("should exit");
            e_exit.cancel();
            // break;
        } else {
            info!("press ctrl+c repeatly in 1 sec to exit.");
            continue;
        }
    }
}

// 处理过程：
// 从数据源取链接(dispatch)-> url -从链接取数据(url2data)-> data -解析数据构造Node节点(data2node)-> node -对节点中的域名进行解析获取ip地址(node2ip)-> ip -根据节点ip获取地区信息并过滤(ip2area)-> area -对节点进行排重(uniq)-> uniq -对节点进行tcp链接测试(filter_bad_ip)-> good -对节点进行真实连接测试(measure_node)-> candidate -做为备用项使用(service)
async fn do_all(
    conf: Arc<Value>,
    e_ip_changed: &Arc<Notify>,
    e_exit: &CancellationToken,
    measure_mode: bool,
    pick_one: bool,
) -> MyResult<String> {
    let worker_name = "[do_all]";
    let b_stop_process = Arc::new(RwLock::new(false));
    let (url_tx, url_rx) = bounded(15);
    let (data_tx, data_rx) = bounded(30);
    let (node_tx, node_rx) = bounded(1000); // TODO: 调整容量
    let (ip_tx, ip_rx) = bounded(1000);
    let (area_tx, area_rx) = bounded(300);
    let (uniq_tx, uniq_rx) = bounded(300); // TODO: 调整容量
    let (good_tx, good_rx) = async_priority_channel::bounded::<Node, u32>(100); // TODO: 调整容量
    let max_candidate: u64 = conf["candidate_max"].as_u64().unwrap();
    let (candidate_tx, candidate_rx) = async_priority_channel::bounded::<Node, u32>(max_candidate);
    let e_clear = Arc::new(Notify::new());

    let e_candidate_change = Arc::new(Notify::new());

    // 用于跟踪正在运行的任务及其元数据
    let mut active_tasks: HashMap<Id, String> = HashMap::new();
    let mut hdl;
    let mut st_task = JoinSet::new();

    hdl = st_task.spawn(dispatch(
        conf.clone(),
        url_rx.clone(),
        url_tx.clone(),
        data_rx.clone(),
        data_tx.clone(),
        node_rx.clone(),
        uniq_rx.clone(),
        good_rx.clone(),
        candidate_rx.clone(),
        e_candidate_change.clone(),
        e_clear.clone(),
        e_exit.clone(),
        b_stop_process.clone(),
        measure_mode,
        pick_one,
    ));
    active_tasks.insert(hdl.id(), "dispatch".to_string());

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

    for i in 1..=nr_workers {
        hdl = st_task.spawn(url2data(
            i,
            url_rx.clone(),
            data_tx.clone(),
            e_exit.clone(),
            b_stop_process.clone(),
        ));
        active_tasks.insert(hdl.id(), format!("u2d-{i:02}"));
    }

    for i in 1..=nr_workers {
        hdl = st_task.spawn(data2node(
            i,
            data_rx.clone(),
            node_tx.clone(),
            e_exit.clone(),
            b_stop_process.clone(),
        ));
        active_tasks.insert(hdl.id(), format!("d2n-{i:02}"));
    }

    for i in 1..=nr_workers * 8 {
        hdl = st_task.spawn(node2ip(
            i,
            node_rx.clone(),
            ip_tx.clone(),
            e_exit.clone(),
            b_stop_process.clone(),
        ));
        active_tasks.insert(hdl.id(), format!("n2i-{i:02}"));
    }

    hdl = st_task.spawn(ip2area(
        1,
        ip_rx.clone(),
        area_tx.clone(),
        e_exit.clone(),
        b_stop_process.clone(),
        conf.clone(),
    ));
    active_tasks.insert(hdl.id(), "i2a".to_string());

    if false {
        // DEBUGONLY
        if let Some(s) = tools::read_file("d:/tmp.txt") {
            if let Err(e) = data_tx.send((String::from("xxx"), s)).await {
                debug!("putting data got err {e}");
            }
        }
    }

    hdl = st_task.spawn(uniq(
        area_rx.clone(),
        uniq_tx.clone(),
        e_clear.clone(),
        e_exit.clone(),
        b_stop_process.clone(),
    ));
    active_tasks.insert(hdl.id(), "uniq".to_string());

    hdl = st_task.spawn(filter_bad_ip(
        uniq_rx.clone(),
        good_tx.clone(),
        e_exit.clone(),
        b_stop_process.clone(),
    ));
    active_tasks.insert(hdl.id(), "filter".to_string());
    hdl = st_task.spawn(measure_node(
        good_rx.clone(),
        candidate_tx.clone(),
        e_candidate_change.clone(),
        e_exit.clone(),
        conf.clone(),
        b_stop_process.clone(),
    ));
    active_tasks.insert(hdl.id(), "measure".to_string());

    hdl = st_task.spawn(service(
        conf.clone(),
        candidate_rx.clone(),
        e_candidate_change.clone(),
        e_ip_changed.clone(),
        e_exit.clone(),
        measure_mode,
    ));
    active_tasks.insert(hdl.id(), "service".to_string());

    debug!("{worker_name} joining task ...");
    let mut output = String::new();
    while let Some(rslt) = st_task.join_next_with_id().await {
        match rslt {
            Ok((id, _)) => {
                if let Some(name) = active_tasks.remove(&id) {
                    trace!("{worker_name} task [{}] joined", name);
                }
            }
            Err(e) => {
                if let Some(name) = active_tasks.remove(&e.id()) {
                    warn!("{worker_name} task [{}] failed: {:?}", name, e);
                }
            }
        }

        if active_tasks.len() > 5 {
            debug!("{worker_name} left {} tasks", active_tasks.len());
        } else {
            for (id, name) in active_tasks.iter() {
                let _ = write!(output, "{id}:{name} ");
            }
            debug!("{worker_name} now left {} task: {output}", st_task.len());
            output.clear();
        }
    }

    info!("{worker_name} done!!!!");
    Ok(String::from("done."))
}

async fn url2data(
    idx: usize,
    url_rx: Receiver<String>,
    data_tx: Sender<(String, String)>,
    e_exit: CancellationToken,
    b_stop: Arc<RwLock<bool>>,
) -> () {
    let worker_name = format!("[u2d-{idx}]");
    let mut exit_flag = false;
    let client = Client::builder()
        .timeout(Duration::from_secs(20)) // 总超时：10秒
        .connect_timeout(Duration::from_secs(5)) // 连接超时：5秒
        .build()
        .unwrap();
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(url) = url_rx.recv() =>{
                {
                    let b_stop = b_stop.read().await;
                    if *b_stop {
                        trace!("{worker_name} b_stop, not process data");
                        continue;
                    }
                }
                tokio::select! {
                    _ = e_exit.cancelled() => {
                        debug!("{worker_name} got exit flag while fetching {url}");
                        break;
                    },
                    rslt = client.get(&url).send() => {
                        match rslt {
                            Ok(it) => {
                                if let Ok(s) = it.text().await {
                                    if !s.is_empty() {
                                        info!("{worker_name} got {} byte(s) data from {url}", s.len());
                                    } else {
                                        warn!("{worker_name} got {} byte(s) data from {url}", s.len());
                                    }
                                    tokio::select!{
                                        _ = e_exit.cancelled() => {
                                            debug!("{worker_name} got exit flag");
                                            break;
                                        }
                                        rslt = data_tx.send((url, s)) => {
                                            if let Err(e) = rslt {
                                                debug!("{worker_name} putting data got err {e}");
                                            }
                                        }
                                    }
                                    tokio::time::sleep(Duration::from_millis(20)).await;
                                }
                            },
                            Err(err) => {
                                info!("{worker_name} fetching {url} got err: {err}");
                            }

                        }
                    }
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn data2node(
    idx: usize,
    data_rx: Receiver<(String, String)>,
    node_tx: Sender<Node>,
    e_exit: CancellationToken,
    b_stop: Arc<RwLock<bool>>,
) -> () {
    let worker_name = format!("[d2n-{idx}]");

    let mut exit_flag = false;
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok((url, data)) = data_rx.recv() =>{
                {
                    let b_stop = b_stop.read().await;
                    if *b_stop {
                        trace!("{worker_name} b_stop, not process data");
                        continue;
                    }
                }

                if url.ends_with(".yaml") || url.ends_with(".yml") {
                    proto::parse_yaml(&worker_name, &data, &url, &e_exit, &node_tx).await;
                } else{
                    proto::data2node_one(&worker_name, &data, &url, &e_exit, &node_tx).await;
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

async fn node2ip(
    idx: usize,
    node_rx: Receiver<Node>,
    ip_tx: Sender<Node>,
    e_exit: CancellationToken,
    b_stop: Arc<RwLock<bool>>,
) -> () {
    let worker_name = format!("[n2i-{idx:02}]");
    let mut exit_flag = false;
    let l_nameserver = vec![
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, 4)), 53),
        // SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 26, 56, 26)), 53),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53),
    ];
    let mut dns = DNSClient::new(l_nameserver);
    dns.set_timeout(Duration::from_secs(5));
    let (mut nr_millsec, mut nr_processed, mut nr_skipped) = (0_u128, 0_u64, 0_u64);
    let mut start;
    let mut show_time = SystemTime::now();
    let mut delta;
    while !exit_flag {
        let now_interval = SystemTime::now()
            .duration_since(show_time)
            .unwrap_or_default()
            .as_secs();
        if now_interval >= 60 {
            trace!(
                "{worker_name} {}s {} = {nr_processed}@{:.1}/s + {nr_skipped}",
                now_interval,
                nr_processed + nr_skipped,
                nr_processed as f64 / (nr_millsec as f64 / 1000_f64)
            );
            show_time = SystemTime::now();
            nr_processed = 0;
            nr_skipped = 0;
            nr_millsec = 0;
        }
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(mut _node) = node_rx.recv() =>{
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
                    start = SystemTime::now();
                    let l_real_ip = dns.query_a(_n.ip.as_str()).await.unwrap_or_default();
                    delta = SystemTime::now().duration_since(start).unwrap_or_default().as_millis();
                    nr_millsec += delta;
                    nr_processed += 1;
                    if !l_real_ip.is_empty(){
                        _n.real_ip = l_real_ip[0].to_string();
                        tokio::select!{
                            _ = e_exit.cancelled() => {
                                debug!("{worker_name} got exit flag");
                                break;
                            },
                            rslt = ip_tx.send(_n) => {
                                if let Err(e) = rslt {
                                    debug!("{worker_name} putting data got {e}");
                                }
                            }
                        }
                    }
                    // else{
                    //     // debug!("{} can\'t get ip from {}", worker_name, _n.ip);
                    // }
                }
                else{
                    nr_skipped += 1;
                    _n.real_ip = _n.ip.clone();
                    tokio::select!{
                        _ = e_exit.cancelled() => {
                            debug!("{worker_name} got exit flag");
                            break;
                        },
                        rslt = ip_tx.send(_n) => {
                            if let Err(e) = rslt {
                                debug!("{worker_name} putting data got {e}");
                            }
                        }
                    }
                }

            }
        }
    }

    info!("{worker_name} done.");
}

async fn ip2area(
    idx: usize,
    ip_rx: Receiver<Node>,
    area_tx: Sender<Node>,
    e_exit: CancellationToken,
    b_stop: Arc<RwLock<bool>>,
    conf: Arc<Value>,
) -> () {
    let worker_name = format!("[i2a-{idx}]");
    let mut exit_flag = false;
    let b_use_unallowed_area = conf["use_unallowed_area"].as_bool().unwrap_or(true);
    let p_unallowed_area = Regex::new(conf["unallowed_area"].as_str().unwrap_or_default()).unwrap();
    let b_use_allowed_area = conf["use_allowed_area"].as_bool().unwrap_or(true);
    let p_allowed_area = Regex::new(conf["allowed_area"].as_str().unwrap_or_default()).unwrap();
    debug!(
        "{worker_name} {} unallowed_area: {p_unallowed_area}, {} allowed_area: {p_allowed_area}",
        if b_use_unallowed_area {
            "use"
        } else {
            "NOT USE"
        },
        if b_use_allowed_area { "use" } else { "NOT USE" }
    );
    let mut wry = qqwry::QQWry::from(conf["qqwry_path"].as_str().unwrap_or_default().to_string());
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(mut _node) = ip_rx.recv() =>{
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
                        tokio::select!{
                            _ = e_exit.cancelled() => {
                                debug!("{worker_name} got exit flag");
                                break;
                            },
                            rslt = area_tx.send(_node) => {
                                if let Err(e) = rslt {
                                    debug!("{worker_name} putting data got {e}");
                                }
                            }
                        }
                    }
                    continue;
                }
                if b_use_unallowed_area && p_unallowed_area.is_match(loc.country.as_str()) {
                    continue;
                }
                if b_use_allowed_area && !p_allowed_area.is_match(loc.country.as_str()) {
                    continue;
                }
                _node.area = loc.country.clone();
                tokio::select!{
                    _ = e_exit.cancelled() => {
                        debug!("{worker_name} got exit flag");
                        break;
                    },
                    rslt = area_tx.send(_node) => {
                        if let Err(e) = rslt {
                            debug!("{worker_name} putting data got {e}");
                            break;
                        }
                    }
                }
            }
        }
    }

    info!("{worker_name} done.");
}

async fn uniq(
    area_rx: Receiver<Node>,
    uniq_tx: Sender<Node>,
    e_clear: Arc<Notify>,
    e_exit: CancellationToken,
    b_stop: Arc<RwLock<bool>>,
) -> () {
    let worker_name = "[uniq]";
    let mut st = HashSet::new();
    let mut exit_flag = false;
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            _ = e_clear.notified() =>{
                st.clear();
            },
            Ok(_node) = area_rx.recv() =>{
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
                if st.insert(_hash_value) {
                    tokio::select!{
                        _ = e_exit.cancelled() => {
                            debug!("{worker_name} got exit flag");
                            break;
                        },
                        rslt = uniq_tx.send(_node) => {
                            if let Err(e) = rslt {
                                debug!("{worker_name} putting data got {e}");
                            }
                        }
                    }
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

async fn filter_bad_ip(
    uniq_rx: Receiver<Node>,
    good_tx: async_priority_channel::Sender<Node, u32>,
    e_exit: CancellationToken,
    b_stop: Arc<RwLock<bool>>,
) -> () {
    let worker_name = "[filter]";
    let mut tasks = FuturesUnordered::new(); // 控制同时存在的协程数量
    const MAX_CONCURRENT_TASKS: usize = 200;

    let mut exit_flag = false;
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok(_node) = uniq_rx.recv() =>{
                {
                    let b = b_stop.read().await;
                    if *b{
                        trace!("{worker_name} b_stop, not process node");
                        continue;
                    }
                }

                let tx = good_tx.clone();
                let cur_b_stop = b_stop.clone();
                let cur_e_exit = e_exit.clone();
                let task = tokio::spawn(async move{
                    let addr = SocketAddr::new(_node.real_ip.parse().unwrap(), _node.port as u16);
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
                            tokio::select! {
                                _ = cur_e_exit.cancelled() => {
                                    debug!("{worker_name} got exit flag");
                                },
                                rslt = tx.send(_node, 999999u32 - _latency as u32) => {
                                    if let Err(e) = rslt {
                                        debug!("{worker_name} putting data got {e}");
                                    }
                                }
                            }
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
                    let _ = tasks.next().await;
                    // trace!("{} wait done for max_concurrent_task", worker_name);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }

        }
    }

    // 等待所有任务退出
    while let Some(rslt) = tasks.next().await {
        if let Err(e) = rslt {
            warn!("{worker_name} join task got {e}");
        }
    }

    info!("{worker_name} done.");
}

async fn measure_node(
    good_rx: async_priority_channel::Receiver<Node, u32>,
    candidate_tx: async_priority_channel::Sender<Node, u32>,
    e_candidate_change: Arc<Notify>,
    e_exit: CancellationToken,
    conf: Arc<Value>,
    b_stop_process: Arc<RwLock<bool>>,
) -> () {
    let worker_name = "[measure]";
    let start_port = conf["port_range"][0].as_u64().unwrap_or_default();
    let end_port = conf["port_range"][1].as_u64().unwrap_or_default();
    let max_concurrent = end_port - start_port + 1;
    let (port_tx, mut port_rx) = mpsc::channel(max_concurrent as usize);
    for _i in start_port..=end_port {
        let _ = port_tx.send(_i).await;
    }
    // set default headers
    let mut headers = header::HeaderMap::new();
    if let Value::Object(_m) = &conf["headers"] {
        for (_k, _v) in _m {
            let _h_k = HeaderName::from_bytes(_k.as_str().as_bytes()).unwrap_or_else(|e| {
                error!("{worker_name} convert key {} failed! {e}", _k.as_str());
                HeaderName::from_static("")
            });
            let _h_v = HeaderValue::from_str(_v.as_str().unwrap_or_default()).unwrap_or_else(|e| {
                error!(
                    "{worker_name} convert value {} failed! {e}",
                    _v.as_str().unwrap()
                );
                HeaderValue::from_static("")
            });
            headers.insert(_h_k, _h_v);
        }
    } else {
        error!("{worker_name} default headers set failed!");
    }

    let mut exit_flag = false;
    let hm_rate: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                exit_flag = true;
            },
            Ok((_node, _)) = good_rx.recv() =>{
                {
                    let b_stop = b_stop_process.read().await;
                    if *b_stop {
                        trace!("{worker_name} b_stop, not process good_rx");
                        continue;
                    }
                }
                let tx = candidate_tx.clone();
                let cur_conf = conf.clone();

                let cur_port: u64;
                tokio::select! {
                    _ = e_exit.cancelled() => {
                        debug!("{worker_name} got exit flag");
                        break;
                    },
                    rslt = port_rx.recv() => {
                        cur_port = match rslt {
                            Some(o) => {o},
                            None => {
                                error!("{worker_name} rx got None !!! channel closed?");
                                break;
                            }
                        };
                    }
                }
                let cur_port_tx = port_tx.clone();
                let cur_good_rx = good_rx.clone();
                let cur_e_candidate_change = e_candidate_change.clone();
                let cur_headers = headers.clone();
                let cur_worker_name = format!("{}-{}", worker_name, cur_port);
                let cur_b_stop = b_stop_process.clone();
                let cur_e_exit = e_exit.clone();
                let cur_hm_rate = Arc::clone(&hm_rate);
                tokio::spawn(async move{
                    let (mut child, file_path) = match launcher::launcher_proxy(cur_worker_name.as_str(), &_node, &cur_conf, cur_port, true).await{
                        Some((a,b))=>{(a,b)},
                        None=>{
                            if log::log_enabled!(log::Level::Trace) {
                                warn!("{cur_worker_name} launcher failed!!! {_node}");
                            }
                            let _ = cur_port_tx.send(cur_port).await;
                            return
                        }
                    };

                    // check again, debug_only
                    if child.id().is_none() {
                        warn!("{cur_worker_name} proxy process exited?");
                    }

                    let speed_limit: u32 = cur_conf["measure_speed_limit"].as_u64().unwrap_or(2000).try_into().unwrap();
                    let cur_proxy = format!("http://127.0.0.1:{cur_port}");
                    trace!("{cur_worker_name} cur_proxy={cur_proxy} for {_node}");
                    let client = reqwest::Client::builder()
                        .proxy(reqwest::Proxy::all(cur_proxy).unwrap())
                        .default_headers(cur_headers)
                        .connect_timeout(Duration::from_millis((speed_limit as f64 * 0.8).round() as u64))
                        .timeout(Duration::from_millis((speed_limit as f64 * 0.8).round() as u64))
                        .build().unwrap_or_else(|e|{error!("client create failed! {e}"); reqwest::Client::builder().build().unwrap()});

                    let min_succ = 2;
                    let (nr_succ, min_latency) = tools::check_proxy(cur_worker_name.as_str(), &_node, &client, "https://www.youtube.com/@wangzhian/streams", 3, min_succ, false, cur_e_exit.clone()).await;
                    let mut b_stop_put = false;
                    {
                        let b_stop = cur_b_stop.read().await;
                        if *b_stop {
                            b_stop_put = true;
                            trace!("{cur_worker_name} b_stop, not put to candidate");
                        }
                    }

                    if !b_stop_put && nr_succ >= min_succ{
                        if min_latency < speed_limit{
                            info!("~~~~~~~~~~~~~~~~ {cur_worker_name} succ {min_latency}ms {} {_node} ~~~~~~~~~~~~~~~", _node.area);

                            // 评分部分
                            let mut sort_items: Vec<(String, u32)>;
                            // 跳过代理部分只取实际的url
                            let _l_pos: Vec<_> = _node.source.match_indices("https://").collect();
                            let _source = if _l_pos.len() > 1 {
                                String::from(_node.source.get(_l_pos[1].0..).unwrap())
                            } else {
                                _node.source.clone()
                            };
                            trace!("{worker_name} rate source {_source}");
                            {
                                // 对应的数据来源评分+1
                                let mut hm_guard = cur_hm_rate.lock().unwrap();
                                *hm_guard.entry(_source).or_insert(0) += 1;
                                sort_items = hm_guard.clone().into_iter().collect();
                            }
                            // 输出当前评分情况
                            let mut output = String::new();
                            sort_items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                            for (url, rate) in sort_items.iter() {
                                writeln!(output, "\trate: {} {}", rate, url).unwrap();
                            }
                            debug!("{worker_name}\n{output}");

                            tokio::select!{
                                _ = cur_e_exit.cancelled() => {
                                    debug!("{cur_worker_name} got exit flag");
                                    return;
                                },
                                rslt = tx.send(_node, 999999u32 - min_latency) => {
                                    if let Err(e) = rslt {
                                        debug!("{cur_worker_name} putting data got {e}");
                                    }else{
                                        cur_e_candidate_change.notify_waiters();
                                        info!("{cur_worker_name} candidate now {}", tx.len());
                                        if tx.is_full(){
                                            info!("{worker_name} candidate queue is full, stop processing.");
                                            let mut b = cur_b_stop.write().await;
                                            *b = true;
                                            while cur_good_rx.try_recv().is_ok(){};
                                        }
                                    }
                            }
                            };
                        }else{
                            debug!("~~~~~ {cur_worker_name} succ but slow {min_latency}ms {} {_node} ~~~~~", _node.area);
                            trace!("{cur_worker_name} drop slow proxy {min_latency}ms {_node}");
                        }
                        // match timeout(Duration::from_millis(20), cur_e_exit.cancelled()).await {
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

    let mut sort_items: Vec<(String, u32)>;
    {
        let hm_guard = hm_rate.lock().unwrap();
        sort_items = hm_guard.clone().into_iter().collect();
    }
    if !sort_items.is_empty() {
        let mut output = String::new();
        sort_items.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        for (url, rate) in sort_items.iter() {
            writeln!(output, "\trate: {} {}", rate, url).unwrap();
        }
        debug!("{worker_name}\n{output}");
    }

    info!("{worker_name} done.");
}

async fn service(
    conf: Arc<Value>,
    candidate_rx: async_priority_channel::Receiver<Node, u32>,
    e_candidate_change: Arc<Notify>,
    e_ip_changed: Arc<Notify>,
    e_exit: CancellationToken,
    measure_mode: bool,
) {
    let worker_name = "[service]";
    let mut pid: u32;
    let mut exit_flag = false;
    let mut cur_ip = tools::get_lan_ip();
    let mut mconf = Value::clone(&conf);
    let pick_new_interval_seconds = conf["pick_new_interval_seconds"].as_u64().unwrap_or(86400);
    // set default headers
    let mut headers = header::HeaderMap::new();
    if let Value::Object(_m) = conf["headers"].clone() {
        for (_k, _v) in _m {
            let _h_k = HeaderName::from_bytes(_k.as_bytes()).unwrap_or_else(|e| {
                error!("{worker_name} convert key {} failed! {e}", _k.as_str());
                HeaderName::from_static("")
            });
            let _h_v = HeaderValue::from_str(_v.as_str().unwrap_or_default()).unwrap_or_else(|e| {
                error!(
                    "{worker_name} convert value {} failed! {e}",
                    _v.as_str().unwrap()
                );
                HeaderValue::from_static("")
            });
            headers.insert(_h_k, _h_v);
        }
    } else {
        error!("{worker_name} default headers set failed!");
    }
    let port: u64 = conf["proxy_port"]
        .as_number()
        .map_or_else(|| 0, |o| o.as_u64().unwrap_or_default());
    // set proxy
    let mut proxy = reqwest::Proxy::all(format!(
        "http://{}:{port}",
        conf["proxy_host"].as_str().unwrap()
    ))
    .unwrap()
    .basic_auth(
        conf["proxy_user"].as_str().unwrap(),
        conf["proxy_pass"].as_str().unwrap(),
    );
    debug!("{worker_name} proxy={proxy:?}");
    while !exit_flag {
        tokio::select! {
            _ = e_exit.cancelled() =>{
                debug!("{worker_name} got exit flag");
                break;
            },
            Ok((_node, mut _prio)) = candidate_rx.recv() =>{  // 取一个备用服务节点信息
                _prio = 999999u32 - _prio;
                debug!("{} pick {}ms {} {}", worker_name, _prio, _node.area, _node);
                e_candidate_change.notify_waiters();
                if measure_mode{
                    info!("{worker_name} in measure_mode, not launch service, sleep 120 sec ...");
                    tokio::select!{
                        _ = e_exit.cancelled() =>{
                            debug!("{worker_name} got exit flag");
                            break;
                        },
                        _ = tokio::time::sleep(Duration::from_secs(120)) =>{}
                    }

                }else{
                    let ip = tools::get_lan_ip();  // 取最新ip
                    // 根据最新ip更新配置项
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
                        proxy = reqwest::Proxy::all(format!("http://{}:{port}", mconf["proxy_host"].as_str().unwrap())).unwrap().basic_auth(mconf["proxy_user"].as_str().unwrap(), mconf["proxy_pass"].as_str().unwrap());
                    }
                    // 启动服务
                    let (mut child, file_path) = match launcher::launcher_proxy(worker_name, &_node, &mconf, port, false).await{
                        Some((a,b))=>{(a,b)},
                        None=>{
                            error!("{worker_name} service launch proxy failed!!! {_node}");
                            continue;
                        }
                    };
                    pid = child.id().unwrap();
                    info!("{worker_name} service launched, {} {_node} {}, pid {pid}, conf {file_path}", _node.area, _node.alias);
                    // 做初次服务心跳测试
                    let client = reqwest::Client::builder()
                        .proxy(proxy.clone())
                        .default_headers(headers.clone())
                        .connect_timeout(Duration::from_secs(5))
                        .timeout(Duration::from_secs(5))
                        .build().unwrap_or_else(|e|{error!("client create failed! {e}"); reqwest::Client::builder().build().unwrap()});

                    let mut show_raw = false; // 控制服务启动成功后首次打印相关信息
                    let mut start_time = SystemTime::now();

                    // 间隔一段时间进行一次心跳测试，失败则清理进程和配置文件，跳出循环
                    loop {
                        let (nr_maxtry, min_succ) = (3, 2);
                        let (nr_succ, min_latency) = tools::check_proxy(worker_name, &_node, &client, "https://www.youtube.com/@wangzhian/streams", nr_maxtry, min_succ, false, e_exit.clone()).await;

                        if nr_succ >= min_succ && min_latency < 2500 { // 过关
                            if !show_raw {
                                info!("{worker_name} node raw: {}  source: {}", _node.raw, _node.source);
                                show_raw = true;
                                start_time = SystemTime::now();
                            }
                            print!("[{pid}_{min_latency}]");
                            let _ = io::stdout().flush();
                        } else { // 不行
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
                            if timeout(Duration::from_millis(10), e_exit.cancelled()).await.is_ok() {
                                warn!("{worker_name} got exit flag!");
                                exit_flag = true;
                            }
                            break;
                        }

                        // 等待下一次检查，退出事件/服务被kill/到达服务切换时间 做相应处理
                        tokio::select!{
                            _ = e_exit.cancelled() => {
                                exit_flag = true;
                                info!("{worker_name} got exit flag");

                                if let Err(e) = child.kill().await{
                                    error!("{worker_name} proxy {pid} kill failed! {e}");
                                }
                                if !file_path.is_empty(){
                                    if let Err(e) = tokio::fs::remove_file(file_path).await{
                                        error!("{worker_name} file remove failed! {e}");
                                    };
                                }
                                break;
                            },
                            _ = child.wait() => {
                                info!("{worker_name} service killed ?");
                                if !file_path.is_empty(){
                                    if let Err(e) = tokio::fs::remove_file(file_path).await{
                                        error!("{worker_name} file remove failed! {e}");
                                    };
                                }

                                if timeout(Duration::from_secs(1), e_exit.cancelled()).await.is_ok() {
                                    warn!("{worker_name} got exit flag?");
                                    exit_flag = true;
                                }
                                break;
                            },
                            _ = tokio::time::sleep(Duration::from_secs(120)) => {
                                if let Ok(duration) = SystemTime::now().duration_since(start_time) {
                                    if duration.as_secs() >= pick_new_interval_seconds {
                                        // 正常运行一段时间后，主动切换为新proxy
                                        info!("{worker_name} proxy already run {} seconds, switch to new one ...", duration.as_secs());
                                        if let Err(e) = child.kill().await{
                                            warn!("{worker_name} proxy {pid} kill failed! {e}");
                                        }
                                        if !file_path.is_empty(){
                                            if let Err(e) = tokio::fs::remove_file(file_path).await{
                                                error!("{worker_name} file remove failed! {e}");
                                            };
                                        }
                                        break;
                                    }
                                }

                            },
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

async fn dispatch(
    conf: Arc<Value>,
    url_rx: Receiver<String>,
    url_tx: Sender<String>,
    data_rx: Receiver<(String, String)>,
    data_tx: Sender<(String, String)>,
    node_rx: Receiver<Node>,
    uniq_rx: Receiver<Node>,
    good_rx: async_priority_channel::Receiver<Node, u32>,
    candidate_rx: async_priority_channel::Receiver<Node, u32>,
    e_candidate_change: Arc<Notify>,
    e_clear: Arc<Notify>,
    e_exit: CancellationToken,
    b_stop_process: Arc<RwLock<bool>>,
    measure_mode: bool,
    pick_one: bool,
) {
    let worker_name = "[dispatch]";

    info!("{}\n v2ray path: {}\n conf_basepath: {}\n proxy_host: {}\n pac_server_port: {}\n pick_new_interval_seconds: {}\n measure_speed_limit: {}\n proxy_http_port: {}\n proxy_socks_port: {}\n proxy_http_port_noauth: {}\n proxy_socks_port_noauth: {}\n proxy_output_file: {}\n measure_mode: {}\n pick_one: {}",
        worker_name, conf["v2ray_path"],
        conf["conf_basepath"],
        conf["proxy_host"],
        conf["pac_server_port"],
        conf["pick_new_interval_seconds"].as_u64().unwrap_or(86400),
        conf["measure_speed_limit"].as_u64().unwrap_or(2000),
        conf["inboundsSetting"][0]["port"],
        conf["inboundsSetting"][1]["port"],
        conf["inboundsSetting"][2]["port"],
        conf["inboundsSetting"][3]["port"],
        conf["proxy_output_file"],
        measure_mode,
        pick_one);

    let min_candidate: u64 = conf["candidate_min"].as_u64().unwrap();
    let max_candidate: u64 = conf["candidate_max"].as_u64().unwrap();
    let mut time_add = SystemTime::now()
        .checked_sub(Duration::from_secs(3600))
        .unwrap_or(UNIX_EPOCH);
    let mut exit_flag = false;
    let mut pick_index: usize = usize::MAX;
    let mut st_recent: HashSet<String> = HashSet::new(); // 记录近期放入过url_rx队列的数据源
    let mut nr_recent = 0; // 每次批量入url_rx队列时递增，用于判断何时清空st_recent
    while !exit_flag {
        print!(".");
        let _ = io::stdout().flush();

        let recent_put = SystemTime::now()
            .duration_since(time_add)
            .unwrap_or_default()
            .as_secs()
            < 180;
        let remain = candidate_rx.len();
        if remain <= min_candidate && !recent_put {
            info!("{worker_name} TMPDBG {remain}<={min_candidate} && not recent put");
            if url_rx.is_empty()
                && data_rx.is_empty()
                && node_rx.is_empty()
                && uniq_rx.is_empty()
                && good_rx.is_empty()
            {
                debug!("{worker_name} TMPDBG all queue is empty");
                {
                    let mut b_stop = b_stop_process.write().await;
                    *b_stop = false;
                }
                info!("{worker_name} few candidate {remain}, get more ...");
                e_clear.notify_waiters();
                let today = chrono::Local::now().date_naive();
                let s_today = today.format("%Y%m%d").to_string(); // 对应 TODAY
                let (s_year, s_month) =
                    (format!("{}", today.year()), format!("{:02}", today.month())); // 对应 YEAR MONTH
                let yesterday = today
                    .checked_sub_days(chrono::Days::new(1))
                    .unwrap_or_default();
                let s_yesterday = yesterday.format("%Y%m%d").to_string(); // 对应 YESTERDAY
                let before_yesterday = yesterday
                    .checked_sub_days(chrono::Days::new(1))
                    .unwrap_or_default();
                let s_before_yesterday = before_yesterday.format("%Y%m%d").to_string(); // 对应 BEFORE_YESTERDAY

                // 从配置文件加载url
                let mut urls: Vec<String> = conf["url_source"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| {
                        let s = v.to_string();
                        s.trim_matches('"').to_string() // 去掉两边的双引号
                    })
                    .filter(|x| !x.trim().is_empty())
                    .collect();
                debug!("{worker_name} load {} url from conf file", urls.len());

                let mut rplc = HashMap::new();
                rplc.insert("{TODAY}", s_today.as_str());
                rplc.insert("{YESTERDAY}", s_yesterday.as_str());
                rplc.insert("{BEFORE_YESTERDAY}", s_before_yesterday.as_str());
                rplc.insert("{YEAR}", s_year.as_str());
                rplc.insert("{MONTH}", s_month.as_str());
                tools::replace_in_place(&mut urls, &rplc);

                // 对github的访问通过随机方式分配代理
                let git_proxies: Vec<&str> = conf["git_proxies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap_or(""))
                    .collect();
                for _line in urls.iter_mut() {
                    if let Some(_proxy) = git_proxies.choose(&mut thread_rng()) {
                        *_line = _line.replace("{PROXY}", _proxy);
                    } else {
                        warn!("get random proxy failed!");
                        break;
                    }
                }

                let mut st: JoinSet<Option<(String, Vec<String>)>> = JoinSet::new();
                if st_recent.insert(String::from("mibei")) {
                    st.spawn(proto::get_mibei_url(worker_name, "mibei"));
                } else {
                    debug!("{worker_name} mibei skip processing this loop.");
                }
                if st_recent.insert(String::from("xrayclashx")) {
                    st.spawn(proto::get_v2rayclashx_url(worker_name, "xrayclashx"));
                } else {
                    debug!("{worker_name} xrayclashx skip processing this loop.");
                }

                let l_githubreadme: Vec<[&str; 4]> = vec![
                    // ["shareclash", "https://{PROXY}/https://raw.githubusercontent.com/shareclash/shareclash.github.io/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["v2rayclashnode", "https://{PROXY}/https://raw.githubusercontent.com/v2rayclashnode/v2rayclashnode.github.io/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["freev2rayclash", "https://{PROXY}/https://raw.githubusercontent.com/freev2rayclash/freev2rayclash.github.io/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["freev2ray", "https://{PROXY}/https://raw.githubusercontent.com/free-v2ray/free-v2ray.github.io/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["windowsv2ray", "https://{PROXY}/https://raw.githubusercontent.com/windowsv2ray/windowsv2ray.github.io/refs/heads/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["clashfreev2ray", "https://{PROXY}/https://raw.githubusercontent.com/clashfreev2ray/clashfreev2ray.github.io/refs/heads/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["v2rayclashx", "https://{PROXY}/https://raw.githubusercontent.com/v2rayclashx/v2rayclashx.github.io/refs/heads/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                    // ["v2rayclashnodes", "https://{PROXY}/https://raw.githubusercontent.com/v2rayclashnodes/v2rayclashnodes.github.io/refs/heads/main/README.md", "V2ray节点订阅链接", r"(?im)^-\s+(https://.+?\.txt)"],
                ];
                if !l_githubreadme.is_empty() {
                    for [_git_name, _url_tpl, _begin_str, _regex_str] in l_githubreadme {
                        let mut _url: String;
                        if let Some(_proxy) = git_proxies.choose(&mut thread_rng()) {
                            _url = _url_tpl.replace("{PROXY}", _proxy);
                        } else {
                            warn!("get random proxy failed!");
                            break;
                        }

                        if pick_one {
                            let (_, one_urls) = proto::get_githubreadme_url(
                                worker_name,
                                _git_name,
                                &_url,
                                _begin_str,
                                _regex_str,
                            )
                            .await
                            .unwrap_or_default();
                            if !one_urls.is_empty() {
                                for url in one_urls {
                                    urls.push(url);
                                }
                                debug!("{worker_name} {_git_name} url(s) added to urls.");
                            }
                        } else {
                            st.spawn(async move {
                                proto::get_githubreadme_url(
                                    worker_name,
                                    _git_name,
                                    &_url,
                                    _begin_str,
                                    _regex_str,
                                )
                                .await
                            });
                        }
                    }
                    info!("{worker_name} TMPDBG all githubreadme url(s) added.");
                }

                while let Some(rslt) = st.join_next().await {
                    if let Ok(Some((_source, _urls))) = rslt {
                        info!(
                            "{worker_name} {_source} {} url(s) added to urls.",
                            _urls.len()
                        );
                        if !_urls.is_empty() {
                            for _url in _urls {
                                urls.push(_url);
                            }
                        }
                    }
                }

                // 打乱顺序
                if !pick_one {
                    urls = tokio::task::block_in_place(|| {
                        let mut url_dup = urls.clone();
                        let mut rng = thread_rng();
                        url_dup.shuffle(&mut rng);
                        url_dup
                    });
                }
                debug!("{worker_name} TMPDBG shuffle url done.");

                // tolinkshare/sharkdoor data
                if !pick_one
                    && st_recent.insert(String::from("tolinkshare"))
                    && st_recent.insert(String::from("vpnnet"))
                {
                    let data_tx_c = data_tx.clone();
                    tokio::spawn(async move {
                        let _d1 = proto::get_tolinkshare_data("test").await;
                        let _d2 = proto::get_vpnnet_data("test").await;
                        for _x in [_d1, _d2].into_iter().flatten() {
                            if let Err(e) = data_tx_c.try_send(_x) {
                                error!("{worker_name} put data failed!!! {e}");
                            }
                        }

                        // 获取几个节点
                        let l_node = proto::get_sharkdoor_url(worker_name)
                            .await
                            .unwrap_or_default();
                        if !l_node.is_empty() {
                            for node in l_node {
                                if let Err(e) = data_tx_c.try_send(node) {
                                    error!("{worker_name} put data failed!!! {e}");
                                }
                            }
                            debug!("{worker_name} sharkdoor data added to data_tx.");
                        }
                    });

                    info!("{worker_name} TMPDBG tolinkshare/sharkdoor data added.");
                } else {
                    if st_recent.contains("tolinkshare") || st_recent.contains("vpnnet") {
                        debug!("{worker_name} tolinkshare/sharkdoor skip processing this loop.");
                    } else {
                        warn!("{worker_name} in pick_one mode, tolinkshare and sharkdoor skip processing !!!");
                    }
                }

                // info!("{worker_name} TMPDBG regular urls len {}", urls.len());
                if pick_one && pick_index == usize::MAX {
                    for (_i, _url) in urls.iter().enumerate() {
                        println!("{_i}: {_url}");
                    }
                    let mut s = String::new();
                    pick_index = loop {
                        s.clear();
                        print!("select by index(0-{}):", urls.len() - 1);
                        let _ = stdout().flush();
                        match stdin().read_line(&mut s) {
                            Ok(_) => {
                                if let Ok(n) = s.trim().to_string().parse() {
                                    if n < urls.len() {
                                        break n;
                                    }
                                    println!("out of index!");
                                } else {
                                    println!("bad input!");
                                }
                            }
                            Err(e) => {
                                error!("{worker_name} got error {e}");
                            }
                        }
                    };
                }

                if pick_index != usize::MAX {
                    info!(
                        "{worker_name} the index {pick_index} is used: {}",
                        urls[pick_index]
                    );
                    if let Err(e) = url_tx.try_send(urls[pick_index].clone()) {
                        error!("{worker_name} put url failed!!! {e}");
                    }
                } else {
                    if nr_recent > 5 {
                        st_recent.clear();
                        debug!("{worker_name} st_recent cleared");
                        nr_recent = 0;
                    }
                    debug!("{worker_name} sending {} urls to url_tx ...", urls.len());
                    for (idx, url) in urls.into_iter().enumerate() {
                        if st_recent.insert(url.clone()) {
                            // 最近没有处理过的才处理，避免短期内处理重复数据源
                            if let Err(e) = url_tx.try_send(url) {
                                error!("{worker_name} put url break at idx {idx}, cause: {e}");
                                break;
                            }
                        }
                    }
                    nr_recent += 1;
                    info!("{worker_name} urls sent done.");
                }

                time_add = SystemTime::now();
            } else {
                let b_stop = b_stop_process.read().await;
                info!(
                    "{worker_name} in processing, url:{} data:{} node:{} uniq:{} good:{} b_stop:{}",
                    url_rx.len(),
                    data_rx.len(),
                    node_rx.len(),
                    uniq_rx.len(),
                    good_rx.len(),
                    *b_stop
                );
            }
        } else {
            if remain >= max_candidate {
                info!("{worker_name} TMPDBG {remain}>={max_candidate}");
                if measure_mode {
                    debug!(
                        "{worker_name} in measure_node not stop measure node, now {}",
                        candidate_rx.len()
                    );
                } else {
                    {
                        let mut b_stop = b_stop_process.write().await;
                        if !*b_stop {
                            info!("{worker_name} remain reach max_candidate, stop processing");
                            *b_stop = true;
                        }
                    }
                    if !url_rx.is_empty()
                        || !data_rx.is_empty()
                        || !node_rx.is_empty()
                        || !uniq_rx.is_empty()
                        || !good_rx.is_empty()
                    {
                        info!("{worker_name} candidate remain {remain}, clear other queues");
                        while url_rx.try_recv().is_ok() {}
                        while data_rx.try_recv().is_ok() {}
                        while node_rx.try_recv().is_ok() {}
                        while uniq_rx.try_recv().is_ok() {}
                        while good_rx.try_recv().is_ok() {}
                        info!("{worker_name} queues cleared.");
                    } else {
                        print!("={}=", candidate_rx.len());
                        let _ = io::stdout().flush();
                    }
                }
            } else {
                // info!("{worker_name} candidate now {}", candidate_rx.len());
                print!("_{}_", candidate_rx.len());
                let _ = io::stdout().flush();
            }
        }

        tokio::select! {
            _ = e_exit.cancelled() =>{
                info!("{worker_name} got exit flag");
                exit_flag = true;
            },
            _ = e_candidate_change.notified() =>{
                info!("{worker_name} candidate changed to {}", candidate_rx.len());
            },
            _ = tokio::time::sleep(Duration::from_secs(if recent_put {90} else {45}))=> {
                // debug!("{worker_name} Timed out");
                print!("~");
                let _ = io::stdout().flush();
            },
            else =>{
                warn!("{worker_name} should not be here !!!");
            }
        }
    }

    info!("{worker_name} done.");
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, futures::channel::mpsc::Receiver<Event>)>
{
    let (mut tx, rx) = channel(1);
    let watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            futures::executor::block_on(async {
                trace!("got event: {res:?}");
                if let Ok(o) = res {
                    #[cfg(windows)]
                    if o.kind == notify::EventKind::Modify(notify::event::ModifyKind::Any)
                        || o.kind == notify::EventKind::Create(notify::event::CreateKind::Any)
                    {
                        tx.send(o).await.unwrap();
                    }

                    #[cfg(unix)]
                    if o.kind
                        == notify::event::EventKind::Modify(notify::event::ModifyKind::Data(
                            notify::event::DataChange::Any,
                        ))
                    {
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

async fn pac_server(conf: Arc<Value>, e_ip_changed: Arc<Notify>, e_exit: CancellationToken) {
    let worker_name = "[pac]";
    let mut proxy_host = conf["proxy_host"].as_str().unwrap_or_default().to_string(); // 只作为初始的ip使用，后续如果ip变更，会动态获取新ip
    let proxy_port: u64 = conf["proxy_port"].as_u64().unwrap_or_default() + 1;
    let mut s_addr = format!("{}:{}", proxy_host, conf["pac_server_port"]);
    let mut addr = s_addr.to_socket_addrs().unwrap().next().unwrap();
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap(); // 创建底层 socket
    socket.set_reuse_address(true).unwrap(); // 设置 SO_REUSEADDR (允许重用地址/端口)
    socket.bind(&addr.into()).unwrap(); // 绑定地址
    socket.listen(64).unwrap(); // 设置为监听模式, 设置全连接队列最大容量(可能用不到)
    let file_path = conf["pac_server_file"].as_str().unwrap_or_default();
    let mut file_content = String::new();
    if let Some(mut s) = tools::read_file(file_path) {
        trace!("{worker_name} file read from {file_path}");
        s = s
            .replace("{proxy_host}", proxy_host.as_str())
            .replace("{proxy_port}", proxy_port.to_string().as_str());
        trace!("{worker_name} {file_path}:\n{s}");
        file_content = s;
    } else {
        error!("{worker_name} file reload failed!!!");
    }

    let (mut watcher, mut rx) = async_watcher().unwrap();
    let watch_dir = Path::new(file_path).parent().unwrap();
    watcher
        .watch(watch_dir, RecursiveMode::NonRecursive)
        .unwrap();
    let mut last_reload = time::Instant::now();

    let mut listener = TcpListener::from_std(socket.into()).unwrap();

    let mut exit_flag = false;
    info!("{worker_name} pac server started at {s_addr}");
    while !exit_flag {
        let cur_file_content = file_content.clone();
        tokio::select! {
            _ = e_exit.cancelled() =>{
                info!("{worker_name} got exit flag");
                exit_flag = true;
            },
            _ = e_ip_changed.notified() => {
                proxy_host = tools::get_lan_ip();
                s_addr = format!("{}:{}", proxy_host, conf["pac_server_port"]);
                addr = s_addr.to_socket_addrs().unwrap().next().unwrap();
                listener = TcpListener::bind(addr).await.unwrap();
                info!("{worker_name} notified ip changed, pac server listened on {s_addr}");
                // TODO ip改变，则pac js里面的ip做同样的改变，目前没想好ip为0.0.0.0也就是断网的时候做什么处理
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
                trace!("{worker_name} detected {:?} {:?}", e.kind, e.paths);
                if !e.paths.is_empty() && e.paths[0].as_path() == Path::new(file_path) && last_reload.elapsed() > time::Duration::from_secs(2) {
                    let mut try_time = 1;
                    let max_try = 3;
                    while try_time <= max_try {
                        if let Some(mut s) = tools::read_file(file_path){
                            s = s.replace("{proxy_host}", proxy_host.as_str()).replace("{proxy_port}", proxy_port.to_string().as_str());
                            file_content = s;
                            if file_content.is_empty() {
                                debug!("try {try_time}/{max_try} got 0 byte from {file_path}");
                            } else {
                                info!("try {try_time}/{max_try} {worker_name} file reloaded. {file_path} {} byte(s)", file_content.len());
                                last_reload = time::Instant::now();
                                break;
                            }
                        } else {
                            error!("try {try_time}/{max_try} {worker_name} file reload failed!!!");
                        }
                        try_time += 1;
                        tokio::time::sleep(Duration::from_millis(500)).await;
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
                    if let Some(_line) = s.lines().next() {  // only check the first line
                        if _line.starts_with("GET ") {
                            let parts: Vec<&str> = _line.split_whitespace().collect();
                            query_path = String::from(parts[1]);
                            if query_path != "/" {  // 不是 '/' 就返回空内容
                                debug!("{worker_name} {} 200 OK to {new_addr:?} 0 bytes", parts[1]);
                                stream.write_all("HTTP/1.1 200 OK\r\nContent_type: text/plain\r\nContent-Length: 0\r\n\r\n".as_bytes()).await.unwrap();
                                stream.flush().await.unwrap();
                                return;
                            }
                        }
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
