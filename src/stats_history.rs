//! 统计历史

use systemstat::System;
use thread::JoinHandle;

use crate::stats::*;
use std::{
    fs::{create_dir_all, File},
    io::{BufRead, BufReader, Write},
};
use std::{
    fs::{rename, OpenOptions},
    io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

const CURRENT_HISTORY_FILE_NAME: &str = "current_stats.txt";
const OLD_HISTORY_FILE_NAME: &str = "old_stats.txt";

/// 定期更新统计历史记录
pub struct UpdatingStatsHistory {
    /// 处理更新统计信息的线程
    _update_thread: JoinHandle<()>,
    /// 统计历史
    pub stats_history: Arc<Mutex<StatsHistory>>,
}

/// 统计历史持久化的配置
#[derive(Clone)]
pub enum HistoryPersistenceConfig {
    /// 禁用持久化
    Disabled,
    /// 启用持久化
    Enabled {
        /// 将统计历史记录保存到的目录
        dir: PathBuf,
        /// 允许保存的统计历史目录增长到的最大大小，以字节为单位
        size_limit: u64,
    },
}

impl UpdatingStatsHistory {
    /// 创建一个`UpdatingStatsHistory`。
    ///
    /// # 参数
    /// * `system` - 待收集统计信息的系统。
    /// * `cpu_sample_duration` - 采样 CPU 负载所需的时间。必须小于`update_frequency`。
    /// * `update_frequency` - 应该多久收集一次新的统计数据。必须大于 `cpu_sample_duration`。
    /// * `history_size` - 保留在历史记录中的最大条目数。
    /// * `consolidation_limit` - 在合并统计数据并将其添加到历史记录之前收集统计数据的次数。
    /// * `persistence_config` - 将历史记录保存到磁盘的配置。
    pub fn new(
        system: System,
        cpu_sample_duration: Duration,
        update_frequency: Duration,
        history_size: NonZeroUsize,
        consolidation_limit: NonZeroUsize,
        persistence_config: HistoryPersistenceConfig,
    ) -> UpdatingStatsHistory {
        //TODO instead of maintaining this list, keep a single moving average?
        let mut recent_stats = Vec::with_capacity(consolidation_limit.get());
        let shared_stats_history = Arc::new(Mutex::new(StatsHistory::new(history_size)));
        let update_thread_stats_history = Arc::clone(&shared_stats_history);
        let update_thread = thread::spawn(move || loop {
            let new_stats = AllStats::from(&system, cpu_sample_duration);
            recent_stats.push(new_stats.clone());

            if recent_stats.len() >= consolidation_limit.get() {
                let consolidated_stats = consolidate_all_stats(recent_stats);
                if let HistoryPersistenceConfig::Enabled { dir, size_limit } = &persistence_config {
                    if let Err(e) = persist_stats(&consolidated_stats, dir, *size_limit) {
                        //TODO use actual logging once https://github.com/SergioBenitez/Rocket/issues/21 is done
                        println!("将统计信息持久保存到 {:?}: {}", dir, e);
                    }
                }

                {
                    let mut history = update_thread_stats_history.lock().unwrap();
                    history.update_most_recent_stats(consolidated_stats);
                    history.push(new_stats);
                }
                recent_stats = Vec::with_capacity(consolidation_limit.get());
            } else {
                let mut history = update_thread_stats_history.lock().unwrap();
                history.update_most_recent_stats(new_stats);
            }

            thread::sleep(update_frequency - cpu_sample_duration);
        });

        UpdatingStatsHistory {
            _update_thread: update_thread,
            stats_history: shared_stats_history,
        }
    }
}

/// 合并所有统计数据
///
/// # 参数
/// * `stats_list` - 待合并的统计数据列表
fn consolidate_all_stats(mut stats_list: Vec<AllStats>) -> AllStats {
    if stats_list.is_empty() {
        panic!("stats_list 不能为空")
    }

    // 首先处理需要计算平均值的统计数据
    let mut average_one_min_load_average = 0.0;
    let mut average_five_min_load_average = 0.0;
    let mut average_fifteen_min_load_average = 0.0;

    let mut average_per_logical_cpu_loads = Vec::new();
    let mut average_aggregate_cpu_load = 0.0;
    let mut average_temp = 0.0;

    let mut average_mem_used = 0.0;
    let mut max_total_mem = 0;

    let mut average_tcp_used = 0.0;
    let mut average_tcp_orphaned = 0.0;
    let mut average_udp_used = 0.0;
    let mut average_tcp6_used = 0.0;
    let mut average_udp6_used = 0.0;

    for (i, all_stats) in stats_list.iter().enumerate() {
        // 更新平均负载
        if let Some(load_averages) = &all_stats.general.load_averages {
            average_one_min_load_average =
                average_one_min_load_average.updated_average(load_averages.one_minute, i + 1);
            average_five_min_load_average =
                average_five_min_load_average.updated_average(load_averages.five_minutes, i + 1);
            average_fifteen_min_load_average = average_fifteen_min_load_average
                .updated_average(load_averages.fifteen_minutes, i + 1);
        }

        // 更新每个CPU的平均负载
        if let Some(loads) = &all_stats.cpu.per_logical_cpu_load_percent {
            average_per_logical_cpu_loads.update_averages(loads, i + 1);
        }

        // 更新CPU整体负载
        if let Some(aggregate) = &all_stats.cpu.aggregate_load_percent {
            average_aggregate_cpu_load =
                average_aggregate_cpu_load.updated_average(*aggregate, i + 1);
        }

        // 更新每个CPU的平均温度
        if let Some(temp) = &all_stats.cpu.temp_celsius {
            average_temp = average_temp.updated_average(*temp, i + 1);
        }

        // 更新内存使用情况
        if let Some(memory_stats) = &all_stats.memory {
            average_mem_used = average_mem_used.updated_average(memory_stats.used_mb as f32, i + 1);
            if memory_stats.total_mb > max_total_mem {
                max_total_mem = memory_stats.total_mb;
            }
        }

        // 更新网络使用信息
        if let Some(socket_stats) = &all_stats.network.sockets {
            average_tcp_used =
                average_tcp_used.updated_average(socket_stats.tcp_in_use as f32, i + 1);
            average_tcp_orphaned =
                average_tcp_orphaned.updated_average(socket_stats.tcp_orphaned as f32, i + 1);
            average_udp_used =
                average_udp_used.updated_average(socket_stats.udp_in_use as f32, i + 1);
            average_tcp6_used =
                average_tcp6_used.updated_average(socket_stats.tcp6_in_use as f32, i + 1);
            average_udp6_used =
                average_udp6_used.updated_average(socket_stats.udp6_in_use as f32, i + 1);
        }
    }

    // 更新系统信息
    let last_stats = stats_list.pop().unwrap(); // 这不应该panic，因为如果 stats_list 为空，我们将无法到达这里
    let general = GeneralStats {
        uptime_seconds: last_stats.general.uptime_seconds,
        boot_timestamp: last_stats.general.boot_timestamp,
        load_averages: Some(LoadAverages {
            one_minute: average_one_min_load_average,
            five_minutes: average_five_min_load_average,
            fifteen_minutes: average_fifteen_min_load_average,
        }),
    };

    // 更新文件系统信息
    let filesystems = last_stats.filesystems;

    // 更新网络接口信息
    let network = NetworkStats {
        interfaces: last_stats.network.interfaces,
        sockets: Some(SocketStats {
            tcp_in_use: average_tcp_used.round() as usize,
            tcp_orphaned: average_tcp_orphaned.round() as usize,
            udp_in_use: average_udp_used.round() as usize,
            tcp6_in_use: average_tcp6_used.round() as usize,
            udp6_in_use: average_udp6_used.round() as usize,
        }),
    };

    let collection_time = last_stats.collection_time;

    AllStats {
        general,
        cpu: CpuStats {
            per_logical_cpu_load_percent: Some(average_per_logical_cpu_loads),
            aggregate_load_percent: Some(average_aggregate_cpu_load),
            temp_celsius: Some(average_temp),
        },
        memory: Some(MemoryStats {
            used_mb: average_mem_used.round() as u64,
            total_mb: max_total_mem,
        }),
        filesystems,
        network,
        collection_time,
    }
}

/// 持久化统计数据
///
/// # 参数
/// * `stats` - 统计信息。
/// * `dir` - 要保存到的目录。
/// * `dir_size_limit_bytes` - 文件大小限制，以比特为单位。
fn persist_stats(stats: &AllStats, dir: &Path, dir_size_limit_bytes: u64) -> io::Result<()> {
    if !dir.exists() {
        create_dir_all(dir)?;
    }

    let current_stats_path = dir.join(CURRENT_HISTORY_FILE_NAME);
    let old_stats_path = dir.join(OLD_HISTORY_FILE_NAME);

    // 将大小限制除以 2，因为这会在 2 个文件之间交换
    if current_stats_path.exists()
        && current_stats_path.metadata()?.len() >= (dir_size_limit_bytes / 2)
    {
        rename(&current_stats_path, &old_stats_path)?;
    }

    let mut current_stats_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(current_stats_path)?;
    writeln!(current_stats_file, "{}", serde_json::to_string(stats)?)?;

    Ok(())
}

trait MovingAverage<T> {
    /// 加入新值来更新平均值。
    ///
    /// # 参数
    /// * `new_value` - 添加到平均值的新值。
    /// * `n` - 数据集中值的数量（包括新值）。
    ///
    /// 返回更新的平均值。
    fn updated_average(self, new_value: T, n: usize) -> T;
}

impl MovingAverage<f32> for f32 {
    fn updated_average(self, new_value: f32, n: usize) -> f32 {
        self + ((new_value - self) / n as f32)
    }
}

trait MovingAverageCollection<T> {
    /// 使用一组新值来更新平均值。
    ///
    /// # 参数
    /// * `new_values` - 添加到平均值的新值。如果大于 `self`，`self` 将用零填充以匹配其大小。
    /// * `n` - 数据集中值集的数量（包括新值）。
    fn update_averages(&mut self, new_values: &[T], n: usize);
}

impl MovingAverageCollection<f32> for Vec<f32> {
    fn update_averages(&mut self, new_values: &[f32], n: usize) {
        while self.len() < new_values.len() {
            self.push(0.0);
        }

        for (i, new_value) in new_values.iter().enumerate() {
            self[i] = self[i] + ((new_value - self[i]) / n as f32)
        }
    }
}

/// 系统统计数据的滚动历史。随着新统计数据的添加，如果历史记录已满，最旧的统计数据将被替换。
pub struct StatsHistory {
    /// 统计信息列表的最大大小
    max_size: NonZeroUsize,
    /// 统计数据列表
    stats: Vec<AllStats>,
    /// 最近添加的统计信息的索引
    most_recent_index: usize,
}

impl StatsHistory {
    /// 创建一个 `StatsHistory`。
    ///
    /// # 参数
    /// * `max_size` - 此历史记录中要保存的最大条目数。
    pub fn new(max_size: NonZeroUsize) -> StatsHistory {
        StatsHistory {
            max_size,
            stats: Vec::with_capacity(max_size.get()),
            most_recent_index: 0,
        }
    }

    /// 从提供的目录加载统计历史记录。
    ///
    /// # 参数
    /// * `dir` - 在其中查找持久统计历史文件的目录。
    pub fn load_from(dir: &Path) -> io::Result<StatsHistory> {
        let mut stats = Vec::new();

        let old_stats_path = dir.join(OLD_HISTORY_FILE_NAME);
        let current_stats_path = dir.join(CURRENT_HISTORY_FILE_NAME);

        add_stats_from_file(old_stats_path, &mut stats)?;
        add_stats_from_file(current_stats_path, &mut stats)?;

        match NonZeroUsize::new(stats.len()) {
            Some(size) => Ok(StatsHistory {
                max_size: size,
                stats,
                most_recent_index: size.get() - 1,
            }),
            None => Ok(StatsHistory::new(NonZeroUsize::new(1).unwrap())),
        }
    }

    /// 将统计数据添加到历史记录。
    ///
    /// # 参数
    /// * `new_stats` - 要添加的统计信息。
    fn push(&mut self, new_stats: AllStats) {
        if self.stats.len() == self.max_size.get() {
            // 列表已满，因此我们需要替换现有条目
            self.most_recent_index = self.get_next_index();
            self.update_most_recent_stats(new_stats);
        } else {
            // 列表还没有满，所以我们可以在末尾添加一个新条目
            self.stats.push(new_stats);
            self.most_recent_index = self.stats.len() - 1;
        }
    }

    /// 用提供的统计信息替换最近添加的统计信息。
    ///
    /// # 参数
    /// * `new_stats` - 用于替换最新统计信息的统计信息。
    fn update_most_recent_stats(&mut self, new_stats: AllStats) {
        if self.stats.is_empty() {
            self.push(new_stats);
        } else {
            self.stats[self.most_recent_index] = new_stats;
        }
    }

    /// 从历史记录中获取最近添加的统计信息。如果历史记录为空，则返回“None”。
    pub fn get_most_recent_stats(&self) -> Option<&AllStats> {
        if self.stats.is_empty() {
            None
        } else {
            Some(&self.stats[self.most_recent_index])
        }
    }

    fn get_next_index(&self) -> usize {
        index_after(self.most_recent_index, self.max_size)
    }
}

/// 从提供的路径（如果存在）的文件中添加统计信息到提供的统计信息列表
fn add_stats_from_file(path: PathBuf, stats: &mut Vec<AllStats>) -> io::Result<()> {
    if path.exists() {
        let file = File::open(path)?;
        for line in BufReader::new(file).lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            stats.push(serde_json::from_str(trimmed)?);
        }
    }

    Ok(())
}

/// 在提供的索引之后查找索引，如果达到最大索引则循环。
fn index_after(i: usize, max_size: NonZeroUsize) -> usize {
    (i + 1) % max_size.get()
}

impl<'a> IntoIterator for &'a StatsHistory {
    type Item = &'a AllStats;
    type IntoIter = StatsHistoryIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let starting_index = if self.stats.len() == self.max_size.get() {
            // 数组已满，因此最旧的统计信息位于下一个索引中。（因为它循环）
            self.get_next_index()
        } else {
            // 数组未满，因此最旧的统计信息位于数组的开头。
            0
        };

        StatsHistoryIterator {
            stats_history: self,
            index: starting_index,
            done: false,
        }
    }
}

pub struct StatsHistoryIterator<'a> {
    stats_history: &'a StatsHistory,
    index: usize,
    done: bool,
}

impl<'a> Iterator for StatsHistoryIterator<'a> {
    type Item = &'a AllStats;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let result = &self.stats_history.stats[self.index];
        if self.index == self.stats_history.most_recent_index {
            self.done = true;
        } else {
            self.index = index_after(self.index, self.stats_history.max_size);
        }
        Some(result)
    }
}
