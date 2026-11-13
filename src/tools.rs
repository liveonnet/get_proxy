use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use chromiumoxide::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, CaptureScreenshotParams, Viewport,
};
use log::{debug, error, info, trace, warn};
use reqwest::Client;
use serde_json::map::Entry;
use serde_json::Value;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
#[allow(unused_imports)]
use tokio::process::Command;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;
// #[allow(unused_imports)]
// use toml::Value as tvalue;
use crate::node::Node;
// use chromiumoxide::browser;
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use regex::Regex;
use std::collections::HashMap;

#[allow(unused_macros)]
macro_rules! assert_types {
    ($($var:ident : $ty:ty),*) => { $(let _: & $ty = & $var;)* }
}

// 获取内网地址
pub fn get_lan_ip() -> String {
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
pub async fn b64d(s: &str, url: &str, show: bool) -> Option<String> {
    let s = s.trim_end_matches('\n').trim_end_matches('\r');
    let mut result = None;
    for i in 0..4 {
        let a_s = [s, &"=".repeat(i)].concat();
        match STANDARD.decode(a_s) {
            Ok(bytes) => {
                result = Some(String::from_utf8(bytes).ok()?);
                break;
            }
            Err(e) => {
                if show {
                    if s.len() >= 500 {
                        trace!(
                            "decode failed {e}, padding {i}, s[0..200] = {}, url = {url}",
                            &s[0..100]
                        );
                    } else {
                        trace!("decode failed {e}, padding {i}, s = {s}, url = {url}");
                    }
                }
            }
        }
    }
    if result.is_none() && show {
        if s.len() >= 500 {
            trace!("64decode failed s[0..200] = {} url = {url}", &s[0..100]);
        } else {
            trace!("64decode failed s = {s} url = {url}");
        }
    }
    result
}

// 去掉windows统一路径前面的\?\
pub fn remove_extended_prefix(pathbuf: &Path) -> PathBuf {
    if let Some(prefix) = pathbuf.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        PathBuf::from(prefix)
    } else {
        pathbuf.to_path_buf()
    }
}

#[allow(dead_code)]
// 读取文件内容，返回String
pub fn read_file(file_path: &str) -> Option<String> {
    let mut path = PathBuf::new();
    path.push(file_path);
    if !path.is_absolute() {
        path = path.canonicalize().unwrap_or_else(|e| {
            debug!("file not found! {e}");
            PathBuf::new()
        });
        debug!("now path is {path:?}");
    }
    if path.as_os_str().is_empty() {
        return None;
    }
    let mut s = String::new();
    let Ok(mut f) = File::open(path.as_path()) else {
        error!("fail to load file {path:?}");
        return None;
    };
    f.read_to_string(&mut s).unwrap_or_default();
    debug!(
        "read {} bytes from file {:?}",
        s.len(),
        remove_extended_prefix(&path)
    );

    Some(s)
}

// 调整别名，转为utf8，去掉空格和制表符
pub fn adjust_alias(s: String) -> String {
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
            }
            Entry::Occupied(mut entry) => {
                let existing_value = entry.get_mut();
                match (existing_value.clone(), value2.clone()) {
                    (Value::Object(_), Value::Object(_)) => {
                        merge_json_objects(existing_value, value2);
                    }
                    (_, _) => {
                        entry.insert(value2.clone());
                    }
                }
            }
        };
    }
}

// 尝试访问代理，返回成功次数和最小延时
pub async fn check_proxy(
    worker_name: &str,
    node: &Node,
    client: &Client,
    url: &str,
    max_retry: u32,
    min_succ: u32,
    show_fail: bool,
    e_exit: CancellationToken,
) -> (u32, u32) {
    let mut nr_succ = 0;
    let mut min_latency = 999999u128;
    for _nr in 1..=max_retry {
        let req = client.head(url).build().unwrap();
        let start = SystemTime::now();
        tokio::select! {
            _ = e_exit.cancelled() => {
                info!("{worker_name} got exit flag while checking proxy!");
                return (0, 999999u32);
            },
            rslt = client.execute(req) => {
                match rslt {
                    Ok(_) => {
                        let latency = SystemTime::now()
                            .duration_since(start)
                            .unwrap_or_default()
                            .as_millis();
                        if latency < min_latency {
                            min_latency = latency;
                        }
                        nr_succ += 1;
                        if nr_succ >= min_succ {
                            // 已达标
                            break;
                        } else if nr_succ + (max_retry - _nr) < min_succ {
                            // 即使后面都成功也达不到min_succ则退出
                            break;
                        }
                    }
                    Err(e) => {
                        if show_fail {
                            warn!("{worker_name} failed {node} {e}");
                        }

                        // 提前退出的情况：被主动拒绝10061，tls握手出错，解码出错；这几种情况再试也不会成功
                        if e.is_connect() && e.is_request() {
                            if let Some(x) = e.source() {
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
                        } else if e.is_decode() {
                            trace!("{worker_name} q failed {node} {e}");
                            break;
                        }

                        if nr_succ + (max_retry - _nr) < min_succ {
                            trace!("{worker_name} failed {node} {e}");
                            break;
                        }
                        if timeout(Duration::from_millis(100), e_exit.cancelled())
                            .await
                            .is_ok()
                        {
                            debug!("{worker_name} got exit flag");
                            break;
                        }
                    }
                }
            }
        }
    }

    // if min_latency != 999999{
    //     debug!("{} check {} return nr_succ {} min_latency {}", worker_name, node, nr_succ, min_latency as u32);
    // }
    (nr_succ, min_latency as u32)
}

#[allow(dead_code)]
// #[cfg(unix)]
// 获取linux下的发行版信息
pub async fn get_distributor_id() -> String {
    let re = Regex::new(r"(?mR)^ID=(\w+)$").unwrap();
    let mut s = String::new();
    match Command::new("/bin/cat")
        .arg("/etc/os-release")
        .output()
        .await
    {
        Ok(output) => {
            let cmd_out = String::from_utf8(output.stdout)
                .unwrap_or_else(|_| String::from("no stdout content"));
            match re.captures(&cmd_out) {
                Some(m) => {
                    s = String::from(&m[1]);
                }
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
        }
        Err(err) => {
            warn!("cmd exec failed!!! {err}");
        }
    }

    s
}

// 获取动态渲染后的页面 HTML
///
/// # 参数
/// * `url`: 要抓取的目标 URL
/// * `proxy`: 可选的代理服务器地址，格式如 "http://host:port" 或 "socks5://host:port"
///   注意：如果代理需要认证，格式为 "http://user:pass@host:port"
/// * `launch_timeout`: 浏览器启动和连接的超时时间
/// * `request_timeout`: 页面导航和渲染的最大等待时间
///
/// # 返回
/// 渲染完成后的 HTML 字符串
pub async fn fetch_dynamic_page(
    url: &str,
    proxy: Option<String>,
    launch_timeout: Duration,
    request_timeout: Duration,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // 1. 构建浏览器启动参数
    let mut config_builder = BrowserConfig::builder().no_sandbox(); // 通常在容器或自动化环境中需要

    // 2. 如果提供了代理，添加到启动参数中
    if let Some(proxy_server) = proxy {
        // Chromium 接受 --proxy-server 参数
        config_builder = config_builder.args(vec![format!("--proxy-server={}", proxy_server)]);
    }

    config_builder = config_builder.request_timeout(request_timeout);
    config_builder = config_builder.arg("--blink-settings=imagesEnabled=false"); // 不加载图片

    let config = config_builder.build()?;

    // 3. 启动浏览器，应用连接超时
    let (mut browser, mut handler) = timeout(launch_timeout, Browser::launch(config))
        .await
        .map_err(|_| "Browser launch timed out")??;
    // info!("browser launched.");

    // 4. 在后台任务中处理浏览器事件流
    let browser_handle = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                warn!("browser handler error: {:?}", event.err());
                break;
            }
        }
    });

    // 5. 创建新页面
    let page = browser.new_page(url).await?;
    page.wait_for_navigation().await?;
    if let Ok(Some(title)) = page.get_title().await {
        debug!("page loaded succ. title: {title} url: {url}");
    }
    let html = page.content().await?;
    trace!("browser got {} bytes from page {url}.", html.len());

    if false {
        // 屏幕截图，未处理懒加载和基于滚动的动态加载
        let width: f64 = page
            .evaluate("document.documentElement.scrollWidth")
            .await?
            .into_value()?;
        let height: f64 = page
            .evaluate("document.documentElement.scrollHeight")
            .await?
            .into_value()?;
        let _ = page
            .save_screenshot(
                CaptureScreenshotParams::builder()
                    .format(CaptureScreenshotFormat::Png)
                    .capture_beyond_viewport(true)
                    .clip(Viewport {
                        x: 0.0,
                        y: 0.0,
                        width,
                        height,
                        scale: 1.0,
                    })
                    .build(),
                "d:/tmp.png",
            )
            .await;
    }

    // 6. 导航到目标 URL，并应用加载超时
    // let navigation_result = timeout(request_timeout, page.goto(url)).await;

    // let html = match navigation_result {
    // let html = match page {
    //     Ok(Ok(_)) => {
    //         // 导航成功

    //         // 7. 等待动态内容渲染
    //         // 简单等待一段时间让 JS 执行，实际项目中可根据特定元素存在与否来判断
    //         tokio::time::sleep(Duration::from_secs(3)).await;

    //         // 8. 获取渲染后的 HTML
    //         page.content().await?
    //     }
    //     Ok(Err(e)) => {
    //         // 导航失败
    //         page.close().await.ok();
    //         browser.close().await.ok();
    //         browser_handle.abort();
    //         return Err(Box::new(e));
    //     }
    //     Err(_) => {
    //         // 加载超时
    //         page.close().await.ok();
    //         browser.close().await.ok();
    //         browser_handle.abort();
    //         return Err("Page load timed out".into());
    //     }
    // };

    // 9. 清理资源
    page.close().await.ok();
    browser.close().await.ok();
    browser_handle.abort();

    Ok(html)
}

// 对l_string里面的每个字符串，根据rplc的对应关系进行关键字原地替换
pub fn replace_in_place(l_string: &mut [String], rplc: &HashMap<&str, &str>) {
    if rplc.is_empty() {
        return;
    }

    // 构建正则表达式，匹配所有需要替换的键  注意：如果键中包含正则特殊字符，需要转义
    let patterns: Vec<String> = rplc.keys().map(|k| regex::escape(k)).collect();
    // 使用 alternation (|) 组合所有键，例如: (Hello|World|Rust)
    let combined_pattern = patterns.join("|");
    let re = Regex::new(&combined_pattern).expect("Invalid regex");

    for _s in l_string.iter_mut() {
        let original = _s.as_str();
        let mut result = String::with_capacity(original.len());
        let mut last_end = 0;

        // 手动迭代所有匹配项，这样可以完全控制生命周期和内存分配
        for caps in re.captures_iter(original) {
            if let Some(m) = caps.get(0) {
                let matched_str = m.as_str();
                let start = m.start();
                let end = m.end();

                // 添加未匹配的部分
                result.push_str(&original[last_end..start]);

                // 查找替换值
                if let Some(&replacement) = rplc.get(matched_str) {
                    result.push_str(replacement);
                    trace!("{matched_str} -> {replacement}");
                } else {
                    // 如果没有替换值，保留原样
                    result.push_str(matched_str);
                }

                last_end = end;
            }
        }

        // 添加最后一部分未匹配的内容
        if last_end < original.len() {
            result.push_str(&original[last_end..]);
        }

        // 如果结果与原串不同，则更新
        if result != original {
            trace!("{original} -> {result}");
            *_s = result;
        }
    }
}
