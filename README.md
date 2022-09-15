# system-stats-dashboard
提供用于查看系统统计信息的简单仪表板，以及用于以编程方式检索所述统计信息的 API。

有 3 个级别的统计信息：“当前”、“最近”和“持久”。当前和最近的统计信息都保存在内存中，持久化的统计信息保存到磁盘。最近的统计数据是持久统计数据的子集。

默认：
* 当前统计数据每 3 秒更新一次。
* 每分钟，当前统计数据的最后一分钟都会合并并作为单个条目添加到最近和持久统计数据列表中。
* 180 个条目（3 小时）保存在最近列表中，2 MB（~2000 个条目，~33 小时）保存在持久列表中。
* 持久化的统计信息存储在 `./stats_history` 中。

# 运行
* 从以下位置下载适用于您平台的版本 [发布页面](https://github.com/wuaqcn/system-stats-dashboard/releases) 并将其解压缩到您喜欢的目录。
* 运行可执行文件 `system-stats-dashboard`。
* 打开 `localhost:8001/dashboard` 查看统计数据。

# 配置
配置选项位于 `Rocket.toml`。
|配置名|默认值|描述|
|----|-------------|-----------|
|address|`"0.0.0.0"`|运行服务器的地址|
|port|`8001`|运行服务器的端口|
|recent_history_size|`180`|最近历史记录中要保留的条目数|
|consolidation_limit|`20`|在合并它们并将条目写入最近和持久的统计信息之前要收集的条目数|
|update_frequency_seconds|`3`|每个统计信息收集之间等待的秒数|
|persist_history|`true`|是否将统计信息保存到磁盘。如果设置为 `false`，则忽略下面的所有配置选项|
|history_files_directory|`"./stats_history"`|将统计信息保存到的目录|
|history_files_max_size_bytes|`2_000_000`|允许`history_files_directory`增长到的最大大小（以字节为单位）|

# 接口

## 仪表板

### `/dashboard`
显示当前统计信息，以及一些最近统计信息的图表。默认为暗模式；为浅色模式添加 `?dark=false`。

![dark_dashboard](https://user-images.githubusercontent.com/48834501/111235475-b7458880-85be-11eb-90a0-0c5d3de4d49b.png)

### `/dashboard/history`
与 `/dashboard` 相同，包含持久化统计信息。

## API

### GET `/stats`
返回所有最近收集的统计信息。

<details>
<summary>示例响应</summary>
```json
{
  "general": {
    "uptimeSeconds": 5239,
    "bootTimestamp": 1615846969,
    "loadAverages": {
      "oneMinute": 0.0,
      "fiveMinutes": 0.01,
      "fifteenMinutes": 0.0
    }
  },
  "cpu": {
    "perLogicalCpuLoadPercent": [
      0.0,
      0.0,
      0.0,
      0.0
    ],
    "aggregateLoadPercent": 0.2450943,
    "tempCelsius": 50.464
  },
  "memory": {
    "usedMb": 52,
    "totalMb": 969
  },
  "filesystems": [
    {
      "fsType": "ext4",
      "mountedFrom": "/dev/root",
      "mountedOn": "/",
      "usedMb": 8208,
      "totalMb": 62699
    }
  ],
  "network": {
    "interfaces": [
      {
        "name": "wlan0",
        "addresses": [
          "192.168.1.100"
        ],
        "sentMb": 1,
        "receivedMb": 1,
        "sentPackets": 4391,
        "receivedPackets": 7024,
        "sendErrors": 0,
        "receiveErrors": 0
      }
    ],
    "sockets": {
      "tcpInUse": 5,
      "tcpOrphaned": 0,
      "udpInUse": 4,
      "tcp6InUse": 4,
      "udp6InUse": 3
    }
  },
  "collectionTime": "2021-03-15T18:50:07.721739139-05:00"
}
```
</details>

### GET `/stats/general`
返回最近收集的一般统计信息。

示例响应：
```json
{
  "uptimeSeconds": 5239,
  "bootTimestamp": 1615846969,
  "loadAverages": {
    "oneMinute": 0.0,
    "fiveMinutes": 0.01,
    "fifteenMinutes": 0.0
  }
}
```

### GET `/stats/cpu`
返回最近收集的与 CPU 相关的统计信息。

示例响应：
```json
{
  "perLogicalCpuLoadPercent": [
    0.0,
    0.0,
    0.0,
    0.0
  ],
  "aggregateLoadPercent": 0.2450943,
  "tempCelsius": 50.464
}
```

### GET `/stats/memory`
返回最近收集的与内存相关的统计信息。

示例响应：
```json
{
  "usedMb": 52,
  "totalMb": 969
}
```

### GET `/stats/filesystems`
返回最近收集的与文件系统相关的统计信息。

示例响应：
```json
[
  {
    "fsType": "ext4",
    "mountedFrom": "/dev/root",
    "mountedOn": "/",
    "usedMb": 8208,
    "totalMb": 62699
  }
]
```

### GET `/stats/network`
返回最近收集的与网络相关的统计信息。

示例响应：
```json
{
  "interfaces": [
    {
      "name": "wlan0",
      "addresses": [
        "192.168.1.100"
      ],
      "sentMb": 1,
      "receivedMb": 1,
      "sentPackets": 4391,
      "receivedPackets": 7024,
      "sendErrors": 0,
      "receiveErrors": 0
    }
  ],
  "sockets": {
    "tcpInUse": 5,
    "tcpOrphaned": 0,
    "udpInUse": 4,
    "tcp6InUse": 4,
    "udp6InUse": 3
  }
}
```

# 可能添加的功能
* 启动时从磁盘加载保存的历史记录
* 如果某些统计数据在一定时间内高于/低于某些值，则发送电子邮件

## 从 Windows 构建 Raspberry Pi
1. 从 https://gnutoolchains.com/raspberry/ 获取链接器
2. 添加 target: `rustup target add armv7-unknown-linux-gnueabihf`
3. 构建: `cargo build --release --target=armv7-unknown-linux-gnueabihf`