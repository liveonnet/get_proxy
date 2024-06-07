use std::time;
use std::time::Duration;
use std::fs::File;
use std::fs::copy;
use std::io::Write;
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;
use tokio::io::BufWriter;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::process::Child;
use crate::node::{self, Node};
use crate::tools;
#[allow(unused_imports)]
use log::{trace, debug, info, warn, error};
use serde_json::Value;
use regex::Regex;
#[cfg(unix)]
use libc;

// https://learn.microsoft.com/zh-cn/windows/win32/procthread/process-creation-flags?redirectedfrom=MSDN
#[cfg(windows)]
#[allow(dead_code)]
static CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

pub async fn launcher_proxy(worker_name: &str, node: &Node, conf: &Value, port: u64, measure_mode: bool) -> Option<(Child, String)>{
    if let Some(setting) = node::create_proxy_conf(worker_name, node, conf, port, measure_mode){
        let v2ray_path = conf["v2ray_path"].as_str().unwrap();
        let wait_millisecs = conf["test_wait_milliseconds"].as_u64().unwrap_or_else(||1000);
        // port不等于配置里的端口号，说明是在做测试
        if port != conf["proxy_port"].as_number().map_or_else(||0, |o|o.as_u64().unwrap_or_default()){
            let mut cmd = Command::new(v2ray_path);

            #[cfg(windows)]
            let cmd_x = cmd.arg("run")
                .creation_flags(CREATE_NEW_PROCESS_GROUP)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());

            #[cfg(unix)]
            let mut cmd_x = cmd.arg("run")
                .process_group(0)
                .stdin(std::process::Stdio::piped())
                // .stdout(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());

            #[cfg(unix)]
            unsafe {
                cmd_x = cmd_x.pre_exec(||{
                    libc::signal(libc::SIGPIPE, libc::SIG_IGN);
                    Ok(())
                });
            }

            match cmd_x.spawn(){
                Ok(mut child)=>{
                    if let Some(h_stdin) = child.stdin.take(){
                        let mut stdin = BufWriter::new(h_stdin);
                        let _ = stdin.write_all(setting.as_bytes()).await;
                        let _ = stdin.flush().await;

                        #[cfg(windows)]
                        drop(stdin);

                        // #[cfg(unix)]
                        // {
                        //     let h = stdin.into_inner();
                        //     child.stdin = Some(h);
                        // }

                        if wait_start(worker_name, &node, &mut child, None, wait_millisecs, false).await {
                            return Some((child, String::new()));
                        }
                        return None;
                    }else{
                        error!("{worker_name} get stdin failed!");
                    }

                },
                Err(err)=>{
                    error!("{worker_name} spawn {v2ray_path} failed! {err}");
                    return None;
                }
            }
        }else{  // 正式使用节点
            let filepath = PathBuf::from(format!("{}{}_{}.json", conf["conf_basepath"].as_str().unwrap_or_default(), node.proto, node.ip.replace(":", "_")));  // ipv6中的冒号替换为下划线
            if let Ok(mut f) = File::create(filepath.as_path()){
                let _ = f.write_all(setting.as_bytes()).unwrap_or_default();
                let _ = f.sync_data();
                debug!("{worker_name} conf file saved. {filepath:?}");
            }else{
                error!("{worker_name} conf file create failed! {filepath:?}");
                return None;
            }

            let mut cmd = Command::new(v2ray_path);

            #[cfg(windows)]
            let cmd_x = cmd.arg("run")
                .arg("-config")
                .arg(filepath.as_path().to_str().unwrap_or_default().to_string())
                .creation_flags(CREATE_NEW_PROCESS_GROUP)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());

            #[cfg(unix)]
            let cmd_x = cmd.arg("run")
                .process_group(0)
                .arg("-config")
                .arg(filepath.as_path().to_str().unwrap_or_default().to_string())
                // .stdout(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null());
            #[cfg(unix)]
            unsafe {
                cmd_x.pre_exec(||{
                    libc::signal(libc::SIGPIPE, libc::SIG_IGN);
                    Ok(())
                });
            }

            match cmd_x.spawn(){
                Ok(mut child)=>{
                    if wait_start(worker_name, &node, &mut child, Some(String::from(filepath.as_os_str().to_str().unwrap())), 5000, true).await {
                        return Some((child, String::from(filepath.as_os_str().to_str().unwrap())));
                    }
                    return None;
                },
                Err(err)=>{
                    error!("{worker_name} spawn {v2ray_path} with conf {filepath:?} failed! {err}");
                }
            }
        }
    } 
    None
}

// 通过读取v2ray输出内容判断是否启动成功，成功返回true，超时/出错返回false
async fn wait_start(worker_name: &str, node: &Node, child: &mut Child, file_path: Option<String>, max_wait: u64, real_use: bool) -> bool {
    let mut ret = true;

    let pid = child.id().unwrap_or_default();
    if let Some(h_stdout) = child.stdout.take() {
        let re = Regex::new(r"\[Warning\] V2Ray .+? started").unwrap();
        let mut stdout = BufReader::new(h_stdout);
        let mut line = String::new();
        let now = time::Instant::now();
        loop {
            line.clear();
            match timeout(Duration::from_millis(max_wait), stdout.read_line(&mut line)).await {
                Err(_) => {
                    trace!("{worker_name} {node} pid {pid} read stdout timeout! {}ms", now.elapsed().as_millis());
                    // ret = false;
                    break;
                },
                Ok(Err(err)) => {
                    warn!("{worker_name} {node} pid {pid} read stdout error: {err}");
                    ret = false;
                    break;
                },
                Ok(Ok(0)) => {
                    if let Some(ref cf) = file_path {
                        let to_file = format!("{cf}.eof");
                        match copy(cf, to_file.as_str()) {
                            Ok(_) => { warn!("{worker_name} {node} pid {pid} read stdout reached EOF, conf file copied to {to_file}");},
                            Err(e) => { warn!("{worker_name} {node} conf file copy got error!!! {e}");}
                        }
                    } else {
                        warn!("{worker_name} {node} pid {pid} read stdout reached EOF");
                    }
                    ret = false;
                    break;
                },
                Ok(Ok(_)) => {
                    // debug!("{worker_name} {node} {pid} read from stdout got {size_got} bytes: {}", line.trim_end());
                    match re.find(&line){
                        Some(_) => {
                            if real_use {
                                info!("{worker_name} {node} pid {pid} started in {}ms", now.elapsed().as_millis());
                            }else {
                                // info!("{worker_name} {node} pid {pid} started in {}ms", now.elapsed().as_millis());
                            }
                            break;
                        },
                        None => {
                            trace!("{worker_name} {node} pid {pid} got stdout: {}", line.trim_end());
                            continue;
                        }
                    };
                }
            }
        }

        #[cfg(windows)]
        drop(stdout);

        #[cfg(unix)]
        {
            let h = stdout.into_inner();
            child.stdout = Some(h);
        }

    }else {
        error!("{worker_name} {node} pid {pid} get stdout failed!");
        ret = false;
    }

    if !ret {
        if let Err(e) = child.kill().await{
            error!("{worker_name} {node} proxy {pid} kill failed! {e}");
        }
        if let Some(filepath) = file_path {
            if let Err(e) = tokio::fs::remove_file(filepath.as_str()).await{
                error!("{worker_name} file {filepath:?} remove failed! {e}");
            };
        }
    }else {
        // 检查下是否退出了
        // check again, debug only
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(filepath) = file_path {
                    match tools::read_file(filepath.as_str()) {
                        Some(s) => {
                            info!("{worker_name} {node} pid {pid} exited unexpectly with {status}, log content:\n{s}");
                        },
                        None => {}
                    }
                } else {
                    info!("{worker_name} {node} pid {pid} exited unexpectly with {status}");
                }
            },
            Ok(None) => {
                // debug!("{worker_name} process exit status is not available now, still running");
            },
            Err(e) => {
                warn!("{worker_name} {node} error try_wait process {pid}!!! {e}");
            }
        }
    }

    ret
}