use std::fs::File;
use memmap::Mmap;
use log::trace;

#[allow(dead_code)]
pub struct QQWry {
    pub file: String,
    m: Mmap,
    m_idx: usize,
    first_index_offset: u32,
    total_index_records: u32,
}

fn tou32(buf: &[u8]) -> u32{
    let b1:u32 = buf[0] as u32 & 0x0000FF;
    let b2:u32 = buf[1] as u32 & 0x0000FF;
    let b3:u32 = buf[2] as u32 & 0x0000FF;
    if buf.len() == 4 {
        let b4:u32 = buf[3] as u32 & 0x0000FF;
        b4 << 24 | b3 << 16 | b2 << 8 | b1
    }else{
        b3 << 16 | b2 << 8 | b1
    }
}

pub fn ip_to_u32(ip:&str) -> u32{
    let mut buf = vec![];
    // for item in ip.split(".") {
    //     let no = item.parse::<u8>().unwrap();
    //     buf.push(no);
    // }
    // buf.reverse();
    for item in ip.rsplit(".") {
        match item.parse::<u8>(){
            Ok(no) => {
                buf.push(no);
            }
            Err(_) => {
                return 0;
            }
        }
    }
    return tou32(&buf[..]);
}

pub fn u32_to_ip(ip:u32) -> String{
    format!("{}.{}.{}.{}", ip >> 24 & 0xFF, ip >> 16 & 0xFF, ip >> 8 & 0xFF, ip & 0xFF)
}

#[derive(Debug)]
pub struct IPLocation {
    pub index_offset: u32,
    pub record_offset: u32,
    pub start_ip: u32,
    pub end_ip: u32,
    pub country: String,
    pub area: String
}

impl QQWry {
    pub fn from(file:  String) -> QQWry {
        let f = File::open(&file).unwrap();
        let m = unsafe{Mmap::map(&f).unwrap()};
        let mut m_idx = 0;
        let first_index_offset = tou32(&m[m_idx..m_idx + 4]);
        m_idx += 4;
        let last_index_offset = tou32(&m[m_idx..m_idx + 4]);
        m_idx += 4;
        let total_index_records = (last_index_offset - first_index_offset) / 7 + 1;
        trace!("qqwry file loaded. {file} first {first_index_offset} last {last_index_offset} , total {total_index_records} records");
        QQWry {
            file,
            m,
            m_idx,
            first_index_offset,
            total_index_records,
        }
    }

    fn read_as_string(&mut self) -> String{
        let mut idx_1 = self.m_idx;
        while self.m[idx_1] as u8 != ('\0' as u8) {
            idx_1 += 1;
        }
        let s = textcode::gb2312::decode_to_string(&self.m[self.m_idx..idx_1]);
        self.m_idx = idx_1 + 1;
        s
    }

    fn read_as_area(&mut self) -> String{
        let b1 = &self.m[self.m_idx];
        self.m_idx += 1;
        if *b1 == 0x01 || *b1 == 0x02 {
            let data_offset = tou32(&self.m[self.m_idx..self.m_idx + 3]);
            self.m_idx += 3;
            if data_offset == 0 {
                return String::from("unknown.area");
            }
            self.m_idx = data_offset as usize;
            return self.read_as_string();
        }else{
            self.m_idx -= 1;
            return self.read_as_string();
        }
    }

    fn read_location(&mut self, record_offset: &u32, location: &mut IPLocation){
        location.record_offset = *record_offset;
        self.m_idx = *record_offset as usize;
        location.end_ip = tou32(&self.m[self.m_idx..self.m_idx + 4]);
        self.m_idx += 4;

        let b1 = &self.m[self.m_idx];
        self.m_idx += 1;
        if *b1 == 0x01 || *b1 == 0x02{
            let data_offset = tou32(&self.m[self.m_idx..self.m_idx + 3]);
            self.m_idx = data_offset as usize;
            if *b1 == 0x01 {
                let b1 = &self.m[self.m_idx];
                self.m_idx += 1;
                let area_offset: u64;
                if *b1 == 0x02 {
                    area_offset = (data_offset + 4) as u64;
                    let data_offset = tou32(&self.m[self.m_idx..self.m_idx + 3]);
                    self.m_idx = data_offset as usize;
                    location.country = self.read_as_string();
                }else{
                    self.m_idx -= 1;
                    location.country = self.read_as_string();
                    area_offset = self.m_idx as u64;
                }
                self.m_idx = area_offset as usize;
                location.area = self.read_as_area();
            } else {
                location.country = self.read_as_string();
                location.area = self.read_as_area();
            }

        }else{
            self.m_idx -= 1;
            location.country = self.read_as_string();
            location.area = self.read_as_area();
        }
    }

    /**
    * 读取IP信息
    */
    pub async fn read_ip_location(&mut self, s_ip: &str) -> IPLocation {
        let ip = ip_to_u32(s_ip);
        let mut result = IPLocation {
            index_offset: 0,
            start_ip: 0,
            end_ip: 0,
            record_offset: 0,
            country: String::new(),
            area: String::new()
        };
        if ip == 0{
            trace!("ip {s_ip} convert failed!!! maybe v6?");
            return result;
        }

        let mut i: u32 = 0;
        let mut right_edge = false;
        loop {
            let index_offset = self.first_index_offset + i * 7;
            self.m_idx = index_offset as usize;
            let  start_ip = tou32(&self.m[self.m_idx..self.m_idx + 4]);
            self.m_idx += 4;
            let b3 = &self.m[self.m_idx..self.m_idx + 3];
            self.m_idx += 3;

            if ip == start_ip {
                self.read_location(&tou32(b3), &mut result);
                result.index_offset = index_offset;
                result.start_ip = start_ip;
                break;
            }
            if ip < start_ip {
                i -= 1;
                right_edge = true;
                continue;
            }
            if right_edge {
                self.read_location(&tou32(b3), &mut result);
                if ip<=result.end_ip {
                    result.index_offset = index_offset;
                    result.start_ip = start_ip;
                }
                break;
            }
            i += 1;
            if i == self.total_index_records {
                self.read_location(&tou32(b3), &mut result);
                if ip<=result.end_ip {
                    result.index_offset = index_offset;
                    result.start_ip = start_ip;
                }
                break;
            }
        }
        result
    }
}
