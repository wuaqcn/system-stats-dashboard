//! 系统统计信息的集合

use std::{io::Error, thread};

use chrono::{DateTime, Local};
use serde::Deserialize;
use serde::Serialize;
use systemstat::{
    saturating_sub_bytes, ByteSize, Duration, IpAddr, NetworkAddrs, Platform, System,
};

// 每MB的字节数
const BYTES_PER_MB: u64 = 1_000_000;

/// 所有系统统计信息
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AllStats {
    /// 一般系统统计
    pub general: GeneralStats,
    /// CPU统计
    pub cpu: CpuStats,
    /// 内存统计
    pub memory: Option<MemoryStats>,
    /// 每个已挂载文件系统的统计信息
    pub filesystems: Option<Vec<MountStats>>,
    /// 网络统计
    pub network: NetworkStats,
    /// 收集统计数据的时间
    pub collection_time: DateTime<Local>,
}

impl AllStats {
    /// 获取所提供系统的所有统计信息。
    ///
    /// # 参数
    /// * `sys` - 指定需要获取信息的系统
    /// * `cpu_sample_duration` - 采样 CPU 负载所需的时间。请注意，此函数将在返回之前在此期间阻塞它所在的线程。
    pub fn from(sys: &System, cpu_sample_duration: Duration) -> AllStats {
        AllStats {
            general: GeneralStats::from(&sys),
            cpu: CpuStats::from(&sys, cpu_sample_duration),
            memory: MemoryStats::from(&sys),
            filesystems: MountStats::from(&sys),
            network: NetworkStats::from(&sys),
            collection_time: Local::now(),
        }
    }
}

/// 一般系统统计
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GeneralStats {
    /// 系统运行的秒数
    pub uptime_seconds: Option<u64>,
    /// 自 UNIX 纪元以来的启动时间（以秒为单位）
    pub boot_timestamp: Option<i64>,
    /// 系统的平均负载
    pub load_averages: Option<LoadAverages>,
}

/// 平均负载
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoadAverages {
    /// 最近1分钟的平均负载
    pub one_minute: f32,
    /// 最近5分钟的平均负载
    pub five_minutes: f32,
    /// 最近15分钟的平均负载
    pub fifteen_minutes: f32,
}

impl GeneralStats {
    /// 获取所提供系统的一般统计信息。
    pub fn from(sys: &System) -> GeneralStats {
        let uptime_seconds = match sys.uptime() {
            Ok(x) => Some(x.as_secs()),
            Err(e) => {
                log("获取系统运行时间时出错: ", e);
                None
            }
        };

        let boot_timestamp = match sys.boot_time() {
            Ok(boot_time) => Some(boot_time.unix_timestamp()),
            Err(e) => {
                log("获取启动时间时出错: ", e);
                None
            }
        };

        let load_averages = match sys.load_average() {
            Ok(x) => Some(LoadAverages {
                one_minute: x.one,
                five_minutes: x.five,
                fifteen_minutes: x.fifteen,
            }),
            Err(e) => {
                log("获取平均负载时出错: ", e);
                None
            }
        };

        GeneralStats {
            uptime_seconds,
            boot_timestamp,
            load_averages,
        }
    }
}

/// CPU统计
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CpuStats {
    /// 每个逻辑 CPU 的负载百分比
    pub per_logical_cpu_load_percent: Option<Vec<f32>>,
    /// CPU整体负载百分比
    pub aggregate_load_percent: Option<f32>,
    /// CPU 的温度，以摄氏度为单位
    pub temp_celsius: Option<f32>,
}

impl CpuStats {
    /// 获取所提供系统的 CPU 统计信息。
    ///
    /// # 参数
    /// * `sys` - 指定需要获取信息的系统
    /// * `sample_duration` - 采样 CPU 负载所需的时间。请注意，此函数将在返回之前在此期间阻塞它所在的线程。
    pub fn from(sys: &System, sample_duration: Duration) -> CpuStats {
        let cpu_load = sys.cpu_load();
        let cpu_load_aggregate = sys.cpu_load_aggregate();
        thread::sleep(sample_duration);
        let per_logical_cpu_load_percent = match cpu_load {
            Ok(x) => match x.done() {
                Ok(cpus) => Some(cpus.iter().map(|cpu| (1.0 - cpu.idle) * 100.0).collect()),
                Err(e) => {
                    log("获取每个逻辑 CPU 负载时​​出错: ", e);
                    None
                }
            },
            Err(e) => {
                log("获取每个逻辑 CPU 负载时​​出错: ", e);
                None
            }
        };

        let aggregate_load_percent = match cpu_load_aggregate {
            Ok(x) => match x.done() {
                Ok(cpu) => Some((1.0 - cpu.idle) * 100.0),
                Err(e) => {
                    log("获取总 CPU 负载时​​出错: ", e);
                    None
                }
            },
            Err(e) => {
                log("获取总 CPU 负载时​​出错: ", e);
                None
            }
        };

        let temp_celsius = match sys.cpu_temp() {
            Ok(x) => Some(x),
            Err(e) => {
                log("获取 CPU 温度时出错: ", e);
                None
            }
        };

        CpuStats {
            per_logical_cpu_load_percent,
            aggregate_load_percent,
            temp_celsius,
        }
    }
}

/// 内存统计
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    /// 使用的内存，以MB为单位
    pub used_mb: u64,
    /// 总内存兆字节，以MB为单位
    pub total_mb: u64,
}

impl MemoryStats {
    /// 获取所提供系统的内存统计信息。如果发生错误，则返回“None”。
    pub fn from(sys: &System) -> Option<MemoryStats> {
        match sys.memory() {
            Ok(mem) => {
                let used_mem = saturating_sub_bytes(mem.total, mem.free);
                Some(MemoryStats {
                    used_mb: bytes_to_mb(used_mem),
                    total_mb: bytes_to_mb(mem.total),
                })
            }
            Err(e) => {
                log("Error getting memory usage: ", e);
                None
            }
        }
    }
}

/// 已挂载文件系统的统计信息
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MountStats {
    /// 文件系统类型（NTFS、ext3 等）
    pub fs_type: String,
    /// 此挂载对应的设备名称
    pub mounted_from: String,
    /// 此挂载对应的根路径
    pub mounted_on: String,
    /// 此挂载使用的空间（以兆字节为单位）
    pub used_mb: u64,
    /// 此挂载的总空间（以 MB 为单位）
    pub total_mb: u64,
}

impl MountStats {
    /// 获取所提供系统的挂载统计信息列表。仅包含总空间超过 0 字节的挂载。如果发生错误，则返回“None”。
    pub fn from(sys: &System) -> Option<Vec<MountStats>> {
        match sys.mounts() {
            Ok(mounts) => Some(
                mounts
                    .into_iter()
                    .filter_map(|mount| {
                        if mount.total.as_u64() == 0 {
                            None
                        } else {
                            let used = saturating_sub_bytes(mount.total, mount.avail);
                            Some(MountStats {
                                fs_type: mount.fs_type,
                                mounted_from: mount.fs_mounted_from,
                                mounted_on: mount.fs_mounted_on,
                                used_mb: bytes_to_mb(used),
                                total_mb: bytes_to_mb(mount.total),
                            })
                        }
                    })
                    .collect(),
            ),
            Err(e) => {
                log("获取挂载信息时出错: ", e);
                None
            }
        }
    }
}

/// 网络统计
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    /// 网络接口的统计信息
    pub interfaces: Option<Vec<NetworkInterfaceStats>>,
    /// 套接字的统计信息
    pub sockets: Option<SocketStats>,
}

impl NetworkStats {
    /// 获取所提供系统的网络统计信息。
    pub fn from(sys: &System) -> NetworkStats {
        NetworkStats {
            interfaces: NetworkInterfaceStats::from(sys),
            sockets: SocketStats::from(sys),
        }
    }
}

/// 网络接口的统计信息
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterfaceStats {
    /// 接口名称
    pub name: String,
    /// 与此接口关联的 IP 地址
    pub addresses: Vec<String>,
    /// 通过此接口发送的总兆字节
    pub sent_mb: u64,
    /// 通过此接口接收的总兆字节
    pub received_mb: u64,
    /// 通过此接口发送的数据包总数
    pub sent_packets: u64,
    /// 通过此接口接收的数据包总数
    pub received_packets: u64,
    /// 通过该接口发送数据时发生的错误总数
    pub send_errors: u64,
    /// 通过该接口接收数据时发生的错误总数
    pub receive_errors: u64,
}

impl NetworkInterfaceStats {
    /// 获取所提供系统的网络接口统计信息列表。如果发生错误，则返回“None”。
    pub fn from(sys: &System) -> Option<Vec<NetworkInterfaceStats>> {
        match sys.networks() {
            Ok(interfaces) => Some(
                interfaces
                    .into_iter()
                    .filter_map(|(_, interface)| match sys.network_stats(&interface.name) {
                        Ok(stats) => {
                            let addresses = interface
                                .addrs
                                .into_iter()
                                .filter_map(address_to_string)
                                .collect();
                            Some(NetworkInterfaceStats {
                                name: interface.name,
                                addresses,
                                sent_mb: bytes_to_mb(stats.tx_bytes),
                                received_mb: bytes_to_mb(stats.rx_bytes),
                                sent_packets: stats.tx_packets,
                                received_packets: stats.rx_packets,
                                send_errors: stats.tx_errors,
                                receive_errors: stats.rx_errors,
                            })
                        }
                        Err(e) => {
                            log(
                                &format!("获取接口统计信息时出错 {}: ", interface.name),
                                e,
                            );
                            None
                        }
                    })
                    .collect(),
            ),
            Err(e) => {
                log("获取接口统计信息时出错: ", e);
                None
            }
        }
    }
}

/// 套接字的统计信息
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SocketStats {
    /// 正在使用的 TCP 套接字数
    pub tcp_in_use: usize,
    /// 孤立 TCP 套接字的数量
    pub tcp_orphaned: usize,
    /// 正在使用的 UDP 套接字数
    pub udp_in_use: usize,
    /// 正在使用的 IPv6 TCP 套接字数
    pub tcp6_in_use: usize,
    /// 正在使用的 IPv6 UDP 套接字数
    pub udp6_in_use: usize,
}

impl SocketStats {
    /// 获取所提供系统的套接字统计信息。如果发生错误，则返回“None”。
    pub fn from(sys: &System) -> Option<SocketStats> {
        match sys.socket_stats() {
            Ok(stats) => Some(SocketStats {
                tcp_in_use: stats.tcp_sockets_in_use,
                tcp_orphaned: stats.tcp_sockets_orphaned,
                udp_in_use: stats.udp_sockets_in_use,
                tcp6_in_use: stats.tcp6_sockets_in_use,
                udp6_in_use: stats.udp6_sockets_in_use,
            }),
            Err(e) => {
                log("获取套接字统计信息时出错: ", e);
                None
            }
        }
    }
}

/// 记录错误消息。如果错误是针对不受支持的统计信息，以调试级别记录。否则以错误级别记录。
fn log(message: &str, e: Error) {
    if e.to_string() == "Not supported" {
        debug!("{}{}", message, e);
    } else {
        error!("{}{}", message, e)
    }
}

/// 获取由提供的 `ByteSize` 表示的兆字节数。
fn bytes_to_mb(byte_size: ByteSize) -> u64 {
    byte_size.as_u64() / BYTES_PER_MB
}

/// 获取 `NetworkAddrs` 的字符串表示形式。如果地址不是 IPv4 或 IPv6，则返回“None”。
fn address_to_string(address: NetworkAddrs) -> Option<String> {
    match address.addr {
        IpAddr::V4(x) => Some(x.to_string()),
        IpAddr::V6(x) => Some(x.to_string()),
        _ => None,
    }
}
