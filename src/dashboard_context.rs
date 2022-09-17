//! 仪表板模板的上下文。

use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, Utc};
use serde::Serialize;

use crate::{
    stats::{GeneralStats, MountStats, NetworkStats},
    stats_history::StatsHistory,
};

const CPU_PER_LOGICAL_CPU_LINE_COLOR_LIGHT_MODE: &str = "#00000044"; // gray
const CPU_PER_LOGICAL_CPU_LINE_COLOR_DARK_MODE: &str = "#ffffff44"; // gray
const CPU_AGGREGATE_LINE_COLOR: &str = "#ffcc00"; // yellow
const CPU_AGGREGATE_FILL_COLOR: &str = "#ffcc0099"; // yellow

const TEMPERATURE_LINE_COLOR: &str = "#990000"; // red
const TEMPERATURE_FILL_COLOR: &str = "#99000099"; // red

const MEM_LINE_COLOR: &str = "#0055ff"; // blue
const MEM_FILL_COLOR: &str = "#0055ff99"; // blue

const SENT_LINE_COLOR: &str = "#44eeaa"; // blue-green
const SENT_FILL_COLOR: &str = "#44eeaa99"; // blue-green
const RECEIVED_LINE_COLOR: &str = "#44dd22"; // green
const RECEIVED_FILL_COLOR: &str = "#44dd2299"; // green

const SEND_ERRORS_LINE_COLOR: &str = "#ff8800"; // yellow-orange
const SEND_ERRORS_FILL_COLOR: &str = "#ff880099"; // yellow-orange
const RECEIVE_ERRORS_LINE_COLOR: &str = "#ff4400"; // orange
const RECEIVE_ERRORS_FILL_COLOR: &str = "#ff440099"; // orange

const TCP_LINE_COLOR: &str = "#44eedd"; // teal
const TCP_FILL_COLOR: &str = "#44eedd99"; // teal
const UDP_LINE_COLOR: &str = "#44bbdd"; // light blue
const UDP_FILL_COLOR: &str = "#44bbdd99"; // light blue

const LOAD_AVERAGE_1_LINE_COLOR: &str = "#ff00ff"; // pink
const LOAD_AVERAGE_1_FILL_COLOR: &str = "#ff00ff99"; // pink
const LOAD_AVERAGE_5_LINE_COLOR: &str = "#bb00ff"; // purple
const LOAD_AVERAGE_5_FILL_COLOR: &str = "#bb00ff99"; // purple
const LOAD_AVERAGE_15_LINE_COLOR: &str = "#7700ff"; // dark purple
const LOAD_AVERAGE_15_FILL_COLOR: &str = "#7700ff99"; // dark purple

/// 仪表板模板的上下文。
#[derive(Serialize)]
pub struct DashboardContext {
    title: String,
    dark_mode: bool,
    charts: Vec<ChartContext>,
    sections: Vec<DashboardSectionContext>,
    last_update_time: String,
}

/// 仪表板中单个图表的上下文。
#[derive(Serialize)]
struct ChartContext {
    /// 此图表的 ID。必须是独一无二的。
    id: String,
    /// 此图表的标题。
    title: String,
    /// 此图表上显示的数据集。
    datasets: Vec<DatasetContext>,
    /// X 轴的标签。
    x_label: String,
    /// Y 轴的标签。
    y_label: String,
    /// X 轴上标记的名称。
    x_values: Vec<String>,
    /// 此图表预期的最低 Y 值。
    min_y: f32,
    /// 此图表预期的最高可能 Y 值。
    max_y: f32,
    /// 要在图表旁边显示的第一行文本。
    accompanying_text_1: String,
    /// 在图表旁边显示的第二行文本。
    accompanying_text_2: String,
}

/// 图表中单个数据集的上下文。
#[derive(Serialize)]
struct DatasetContext {
    /// 此数据集的名称。
    name: String,
    /// 用于线条的颜色代码。
    line_color_code: String,
    /// 用于线下区域的颜色代码。仅当 `fill` 为 `true` 时才相关。
    fill_color_code: String,
    /// 此数据集中的值。
    values: Vec<f32>,
    /// 是否填充线下区域。
    fill: bool,
}

/// 仪表板部分的上下文。
#[derive(Serialize)]
struct DashboardSectionContext {
    /// 名称
    name: String,
    /// 统计数据
    stats: Vec<String>,
    /// 小节
    subsections: Vec<DashboardSubsectionContext>,
}

/// 仪表板子部分的上下文
#[derive(Serialize)]
struct DashboardSubsectionContext {
    /// 名称
    name: String,
    /// 统计数据
    stats: Vec<String>,
}

impl DashboardContext {
    /// 从提供的统计历史记录中构建一个 `DashboardContext`。
    ///
    /// # 参数
    /// * `stats_history` - 用于填充上下文的统计历史记录。
    /// * `dark_mode` - 是否启用暗模式。
    pub fn from_history(stats_history: &StatsHistory, dark_mode: bool) -> DashboardContext {
        let title = "仪表盘".to_string();

        let mut sections = Vec::new();
        let most_recent_stats = match stats_history.get_most_recent_stats() {
            Some(x) => x,
            None => {
                return DashboardContext {
                    title,
                    dark_mode,
                    charts: Vec::new(),
                    sections: vec![DashboardSectionContext {
                        name: "暂无数据".to_string(),
                        stats: Vec::new(),
                        subsections: Vec::new(),
                    }],
                    last_update_time: "N/A".to_string(),
                }
            }
        };

        if let Some(x) = build_general_section(&most_recent_stats.general) {
            sections.push(x);
        }
        if let Some(x) = build_network_section(&most_recent_stats.network) {
            sections.push(x);
        }
        if let Some(x) = &most_recent_stats.filesystems {
            sections.push(build_filesystems_section(x));
        }

        let mut charts = Vec::new();
        charts.extend(build_cpu_charts(stats_history, dark_mode));
        charts.push(build_memory_chart(stats_history));
        charts.push(build_load_average_chart(stats_history));
        charts.extend(build_network_charts(stats_history));

        DashboardContext {
            title,
            dark_mode,
            charts,
            sections,
            last_update_time: most_recent_stats
                .collection_time
                .to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

/// 创建一般小节
///
/// # 参数
/// * `stats` - 系统信息
fn build_general_section(stats: &GeneralStats) -> Option<DashboardSectionContext> {
    let mut stat_strings = Vec::new();
    if let Some(x) = stats.uptime_seconds {
        stat_strings.push(format!("正常运行时间: {} 秒", x))
    };
    if let Some(x) = stats.boot_timestamp {
        let parsed_time = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(x, 0), Utc);
        stat_strings.push(format!(
            "开机时间: {}",
            parsed_time.with_timezone(&Local).to_rfc3339()
        ))
    }

    if stat_strings.is_empty() {
        None
    } else {
        Some(DashboardSectionContext {
            name: "系统信息".to_string(),
            stats: stat_strings,
            subsections: Vec::new(),
        })
    }
}

/// 创建网络小节
///
/// # 参数
/// * `network_stats` - 网络统计信息
fn build_network_section(network_stats: &NetworkStats) -> Option<DashboardSectionContext> {
    let mut subsections = Vec::new();
    match &network_stats.sockets {
        Some(socket_stats) => subsections.push(DashboardSubsectionContext {
            name: "Sockets".to_string(),
            stats: vec![
                format!(
                    "TCP: {} in use total, {} IPv6, {} orphaned",
                    socket_stats.tcp_in_use, socket_stats.tcp6_in_use, socket_stats.tcp_orphaned
                ),
                format!(
                    "UDP: {} in use total, {} IPv6",
                    socket_stats.udp_in_use, socket_stats.udp6_in_use
                ),
            ],
        }),
        None => (),
    }

    match &network_stats.interfaces {
        Some(x) => {
            for interface in x {
                subsections.push(DashboardSubsectionContext {
                    name: interface.name.clone(),
                    stats: vec![
                        format!("IP addresses: {}", interface.addresses.join(", ")),
                        format!(
                            "Sent: {} packets, {} MB, {} errors",
                            interface.sent_packets, interface.sent_mb, interface.send_errors
                        ),
                        format!(
                            "Received: {} packets, {} MB, {} errors",
                            interface.received_packets,
                            interface.received_mb,
                            interface.receive_errors
                        ),
                    ],
                })
            }
        }
        None => (),
    }

    if subsections.is_empty() {
        None
    } else {
        Some(DashboardSectionContext {
            name: "Network".to_string(),
            stats: Vec::new(),
            subsections,
        })
    }
}

/// 创建文件系统小节
///
/// # 参数
/// * `mount_stats` - 文件系统信息
fn build_filesystems_section(mount_stats: &[MountStats]) -> DashboardSectionContext {
    let mut total_used_mb = 0;
    let mut total_total_mb = 0;
    let mut subsections = Vec::new();
    for mount in mount_stats {
        total_used_mb += mount.used_mb;
        total_total_mb += mount.total_mb;
        let used_pct = ((mount.used_mb as f64) / (mount.total_mb as f64)) * 100.0;
        subsections.push(DashboardSubsectionContext {
            name: mount.mounted_on.clone(),
            stats: vec![
                format!("类型: {}", mount.fs_type),
                format!("挂载点: {}", mount.mounted_from),
                format!(
                    "使用量: {} / {} MB ({:.2}%)",
                    mount.used_mb, mount.total_mb, used_pct
                ),
            ],
        });
    }

    let total_used_pct = ((total_used_mb as f64) / (total_total_mb as f64)) * 100.0;
    DashboardSectionContext {
        name: "文件系统".to_string(),
        stats: vec![format!(
            "总使用量: {} / {} MB ({:.2}%)",
            total_used_mb, total_total_mb, total_used_pct
        )],
        subsections,
    }
}

/// 创建CPU图表
///
/// # 参数
/// * `stats_history` - 历史统计信息
fn build_cpu_charts(stats_history: &StatsHistory, dark_mode: bool) -> Vec<ChartContext> {
    let mut charts = Vec::new();
    let mut cpu_datasets = Vec::new();
    let mut aggregate_values = Vec::new();
    let mut per_logical_cpu_values = Vec::new();
    let mut temp_values = Vec::new();
    let mut x_values = Vec::new();
    let empty_vec = Vec::new();
    for stats in stats_history.into_iter() {
        aggregate_values.push(stats.cpu.aggregate_load_percent.unwrap_or(0.0));
        per_logical_cpu_values.push(
            stats
                .cpu
                .per_logical_cpu_load_percent
                .as_ref()
                .unwrap_or(&empty_vec),
        );
        temp_values.push(stats.cpu.temp_celsius.unwrap_or(0.0));
        x_values.push(format_time(stats.collection_time));
    }

    let usage_accompanying_text = format!("{:.2}%", aggregate_values.last().unwrap_or(&0.0));

    cpu_datasets.push(DatasetContext {
        name: "总计".to_string(),
        line_color_code: CPU_AGGREGATE_LINE_COLOR.to_string(),
        fill_color_code: CPU_AGGREGATE_FILL_COLOR.to_string(),
        values: aggregate_values,
        fill: true,
    });

    // TODO 必须有更好的方法来做到这一点
    let num_logical_cpus = match per_logical_cpu_values.first() {
        Some(x) => x.len(),
        None => 0,
    };
    let mut per_logical_cpu_values_flipped: Vec<Vec<f32>> = Vec::new();
    for _ in 0..num_logical_cpus {
        per_logical_cpu_values_flipped.push(Vec::new());
    }
    for vec in per_logical_cpu_values {
        for (i, x) in vec.iter().enumerate() {
            per_logical_cpu_values_flipped[i].push(*x);
        }
    }

    let per_logical_cpu_line_color = if dark_mode {
        CPU_PER_LOGICAL_CPU_LINE_COLOR_DARK_MODE
    } else {
        CPU_PER_LOGICAL_CPU_LINE_COLOR_LIGHT_MODE
    };
    for (i, values) in per_logical_cpu_values_flipped.into_iter().enumerate() {
        cpu_datasets.push(DatasetContext {
            name: format!("CPU {}", i),
            line_color_code: per_logical_cpu_line_color.to_string(),
            fill_color_code: "".to_string(),
            values,
            fill: false,
        });
    }

    charts.push(ChartContext {
        id: "cpu-usage-chart".to_string(),
        title: "CPU使用率".to_string(),
        datasets: cpu_datasets,
        x_label: "时间".to_string(),
        y_label: "使用率 (%)".to_string(),
        x_values: x_values.clone(),
        min_y: 0.0,
        max_y: 100.0,
        accompanying_text_1: usage_accompanying_text,
        accompanying_text_2: "".to_string(),
    });

    let temp_accompanying_text = format!("{:.2}°C", temp_values.last().unwrap_or(&0.0));
    charts.push(ChartContext {
        id: "cpu-temp-chart".to_string(),
        title: "温度".to_string(),
        datasets: vec![DatasetContext {
            name: "摄氏度".to_string(),
            line_color_code: TEMPERATURE_LINE_COLOR.to_string(),
            fill_color_code: TEMPERATURE_FILL_COLOR.to_string(),
            values: temp_values,
            fill: true,
        }],
        x_label: "时间".to_string(),
        y_label: "温度 (C)".to_string(),
        x_values,
        min_y: 0.0,
        max_y: 85.0,
        accompanying_text_1: temp_accompanying_text,
        accompanying_text_2: "".to_string(),
    });

    charts
}

/// 创建存储图表
///
/// # 参数
/// * `stats_history` - 历史统计信息
fn build_memory_chart(stats_history: &StatsHistory) -> ChartContext {
    let mut memory_values = Vec::new();
    let mut memory_total_mb = 0;
    let mut x_values = Vec::new();
    for stats in stats_history.into_iter() {
        match &stats.memory {
            Some(x) => {
                if x.total_mb > memory_total_mb {
                    memory_total_mb = x.total_mb;
                }
                memory_values.push(x.used_mb as f32)
            }
            None => memory_values.push(0.0),
        }
        x_values.push(format_time(stats.collection_time));
    }

    let (accompanying_text_1, accompanying_text_2) = {
        match stats_history.get_most_recent_stats() {
            Some(x) => match &x.memory {
                Some(mem) => {
                    let used_pct = ((mem.used_mb as f64) / (mem.total_mb as f64)) * 100.0;
                    (
                        format!("{} / {} MB", mem.used_mb, mem.total_mb),
                        format!("{:.2}%", used_pct),
                    )
                }
                None => ("-- / -- MB".to_string(), "--%".to_string()),
            },
            None => ("-- / -- MB".to_string(), "--%".to_string()),
        }
    };

    ChartContext {
        id: "ram-chart".to_string(),
        title: "内存使用量".to_string(),
        datasets: vec![DatasetContext {
            name: "已用内存".to_string(),
            line_color_code: MEM_LINE_COLOR.to_string(),
            fill_color_code: MEM_FILL_COLOR.to_string(),
            values: memory_values,
            fill: true,
        }],
        x_label: "时间".to_string(),
        y_label: "使用量 (MB)".to_string(),
        x_values,
        min_y: 0.0,
        max_y: memory_total_mb as f32,
        accompanying_text_1,
        accompanying_text_2,
    }
}

/// 创建负载图表
///
/// # 参数
/// * `stats_history` - 历史统计信息
fn build_load_average_chart(stats_history: &StatsHistory) -> ChartContext {
    let mut one_min_values = Vec::new();
    let mut five_min_values = Vec::new();
    let mut fifteen_min_values = Vec::new();
    let mut x_values = Vec::new();
    for stats in stats_history.into_iter() {
        match &stats.general.load_averages {
            Some(x) => {
                one_min_values.push(x.one_minute);
                five_min_values.push(x.five_minutes);
                fifteen_min_values.push(x.fifteen_minutes);
            }
            None => {
                one_min_values.push(0.0);
                five_min_values.push(0.0);
                fifteen_min_values.push(0.0);
            }
        }

        x_values.push(format_time(stats.collection_time));
    }

    let accompanying_text = format!(
        "1: {:.2}, 5: {:.2}, 15: {:.2}",
        one_min_values.last().unwrap_or(&0.0),
        five_min_values.last().unwrap_or(&0.0),
        fifteen_min_values.last().unwrap_or(&0.0)
    );
    let datasets = vec![
        DatasetContext {
            name: "1 分钟".to_string(),
            line_color_code: LOAD_AVERAGE_1_LINE_COLOR.to_string(),
            fill_color_code: LOAD_AVERAGE_1_FILL_COLOR.to_string(),
            values: one_min_values,
            fill: false,
        },
        DatasetContext {
            name: "5 分钟".to_string(),
            line_color_code: LOAD_AVERAGE_5_LINE_COLOR.to_string(),
            fill_color_code: LOAD_AVERAGE_5_FILL_COLOR.to_string(),
            values: five_min_values,
            fill: false,
        },
        DatasetContext {
            name: "15 分钟".to_string(),
            line_color_code: LOAD_AVERAGE_15_LINE_COLOR.to_string(),
            fill_color_code: LOAD_AVERAGE_15_FILL_COLOR.to_string(),
            values: fifteen_min_values,
            fill: false,
        },
    ];

    ChartContext {
        id: "load-average-chart".to_string(),
        title: "平均负载".to_string(),
        datasets,
        x_label: "时间".to_string(),
        y_label: "平均负载".to_string(),
        x_values,
        min_y: 0.0,
        max_y: 0.0,
        accompanying_text_1: accompanying_text,
        accompanying_text_2: "".to_string(),
    }
}

/// 创建网络图表
///
/// # 参数
/// * `stats_history` - 历史统计信息
fn build_network_charts(stats_history: &StatsHistory) -> Vec<ChartContext> {
    let mut sent_mb_values = Vec::new();
    let mut received_mb_values = Vec::new();
    let mut send_errors_values = Vec::new();
    let mut receive_errors_values = Vec::new();
    let mut tcp_sockets_values = Vec::new();
    let mut udp_sockets_values = Vec::new();
    let mut x_values = Vec::new();
    for stats in stats_history.into_iter() {
        match &stats.network.interfaces {
            Some(x) => {
                let mut total_sent_mb = 0.0;
                let mut total_received_mb = 0.0;
                let mut total_send_errors = 0.0;
                let mut total_receive_errors = 0.0;
                for interface_stats in x {
                    total_sent_mb += interface_stats.sent_mb as f32;
                    total_received_mb += interface_stats.received_mb as f32;
                    total_send_errors += interface_stats.send_errors as f32;
                    total_receive_errors += interface_stats.receive_errors as f32;
                }

                sent_mb_values.push(total_sent_mb);
                received_mb_values.push(total_received_mb);
                send_errors_values.push(total_send_errors);
                receive_errors_values.push(total_receive_errors);
            }
            None => {
                sent_mb_values.push(0.0);
                received_mb_values.push(0.0);
                send_errors_values.push(0.0);
                receive_errors_values.push(0.0);
            }
        }

        match &stats.network.sockets {
            Some(x) => {
                tcp_sockets_values.push(x.tcp_in_use as f32);
                udp_sockets_values.push(x.udp_in_use as f32);
            }
            None => {
                tcp_sockets_values.push(0.0);
                udp_sockets_values.push(0.0);
            }
        }

        x_values.push(format_time(stats.collection_time));
    }

    let mut charts = Vec::new();

    let usage_accompanying_text = format!(
        "{} MB sent, {} MB received",
        sent_mb_values.last().unwrap_or(&0.0),
        received_mb_values.last().unwrap_or(&0.0)
    );
    let usage_datasets = vec![
        DatasetContext {
            name: "发送".to_string(),
            line_color_code: SENT_LINE_COLOR.to_string(),
            fill_color_code: SENT_FILL_COLOR.to_string(),
            values: sent_mb_values,
            fill: false,
        },
        DatasetContext {
            name: "接收".to_string(),
            line_color_code: RECEIVED_LINE_COLOR.to_string(),
            fill_color_code: RECEIVED_FILL_COLOR.to_string(),
            values: received_mb_values,
            fill: false,
        },
    ];

    charts.push(ChartContext {
        id: "network-usage-chart".to_string(),
        title: "累积网络使用量".to_string(),
        datasets: usage_datasets,
        x_label: "时间".to_string(),
        y_label: "总计 (MB)".to_string(),
        x_values: x_values.clone(),
        min_y: 0.0,
        max_y: 0.0,
        accompanying_text_1: usage_accompanying_text,
        accompanying_text_2: "".to_string(),
    });

    let errors_accompanying_text = format!(
        "{} 已发送, {} 已接收",
        send_errors_values.last().unwrap_or(&0.0),
        receive_errors_values.last().unwrap_or(&0.0)
    );
    let errors_datasets = vec![
        DatasetContext {
            name: "发送".to_string(),
            line_color_code: SEND_ERRORS_LINE_COLOR.to_string(),
            fill_color_code: SEND_ERRORS_FILL_COLOR.to_string(),
            values: send_errors_values,
            fill: false,
        },
        DatasetContext {
            name: "Receive".to_string(),
            line_color_code: RECEIVE_ERRORS_LINE_COLOR.to_string(),
            fill_color_code: RECEIVE_ERRORS_FILL_COLOR.to_string(),
            values: receive_errors_values,
            fill: false,
        },
    ];

    charts.push(ChartContext {
        id: "network-errors-chart".to_string(),
        title: "累积网络错误".to_string(),
        datasets: errors_datasets,
        x_label: "时间".to_string(),
        y_label: "总错误".to_string(),
        x_values: x_values.clone(),
        min_y: 0.0,
        max_y: 0.0,
        accompanying_text_1: errors_accompanying_text,
        accompanying_text_2: "".to_string(),
    });

    let sockets_accompanying_text = format!(
        "{} TCP, {} UDP",
        tcp_sockets_values.last().unwrap_or(&0.0),
        udp_sockets_values.last().unwrap_or(&0.0)
    );
    let sockets_datasets = vec![
        DatasetContext {
            name: "TCP".to_string(),
            line_color_code: TCP_LINE_COLOR.to_string(),
            fill_color_code: TCP_FILL_COLOR.to_string(),
            values: tcp_sockets_values,
            fill: false,
        },
        DatasetContext {
            name: "UDP".to_string(),
            line_color_code: UDP_LINE_COLOR.to_string(),
            fill_color_code: UDP_FILL_COLOR.to_string(),
            values: udp_sockets_values,
            fill: false,
        },
    ];

    charts.push(ChartContext {
        id: "sockets-chart".to_string(),
        title: "套接字使用量".to_string(),
        datasets: sockets_datasets,
        x_label: "时间".to_string(),
        y_label: "使用量".to_string(),
        x_values,
        min_y: 0.0,
        max_y: 0.0,
        accompanying_text_1: sockets_accompanying_text,
        accompanying_text_2: "".to_string(),
    });

    charts
}

/// 格式化时间
///
/// # 参数
/// * `time` - 本地时间
fn format_time(time: DateTime<Local>) -> String {
    time.format("%I:%M:%S %p").to_string()
}
