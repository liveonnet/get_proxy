var domains = {
    "google.com": 1,
    "youtube.com": 1,
    "github.com": 1,
    "gstatic.com": 1,
    "googleapis.com": 1,
    "phncdn.com": 1,
    "ytimg.com": 1,
    "googlevideo.com": 1,
    "objects.githubusercontent.com": 1,
    "ggpht.com": 1,
    "v2ray.com": 1,
    "pornhub.com": 1,
    "xvideos.com": 1,
    "img.blr844.com": 1,
    "imgpile.com": 1,
    "contents.fc2.com": 1,
    "ccimg.xyz": 1,
    "97p.org": 1,
    "imgbox.xyz": 1,
    "pic.dmoe.in": 1,
    "pics.dmm.co.jp": 1,
    "sshare666.com": 1,
    "i.97p.org": 1,
    "bic2303d.click": 1,
    "qpic.ws": 1,
    "youporn.com": 1,
    "redtube.com": 1,
    "pic.dmoe.in": 1,
    "tube8.com": 1,
    "githubassets.com": 1,
    "imgbox.xyzm": 1,
    "gateway.ipfsscan.io": 1
};

var proxy = "PROXY {proxy_host}:{proxy_port}; DIRECT;";

var direct = "DIRECT;";

function FindProxyForURL(url, host) {
    var lastPos;
    do {
        if (domains.hasOwnProperty(host)) {
            return proxy;
        }
        lastPos = host.indexOf('.') + 1;
        host = host.slice(lastPos);
    } while (lastPos >= 1);
    return direct;
}
