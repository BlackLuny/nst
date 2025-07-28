# Network Stability Test (NST)

一个用于测试SOCKS5代理网络稳定性的Rust CLI工具。

## 功能特性

### 核心测试模块

1. **TCP连接稳定性测试**
   - 长连接维持和心跳检测
   - 连接断开检测和重连机制  
   - 延迟和丢包统计
   - 连接稳定性评分

2. **持续小流量带宽测试**
   - 模拟真实应用场景的数据传输
   - 上传/下载速度监控
   - 吞吐量稳定性分析
   - 数据完整性验证

3. **连接性能测试**
   - 批量并发连接建立测试
   - SOCKS5握手性能测试
   - 连接建立成功率统计
   - 多级并发压力测试

4. **DNS解析稳定性测试**
   - 通过代理进行DNS查询测试
   - 多域名解析时间监控
   - DNS缓存行为分析
   - 跨域名一致性检测

5. **网络抖动测试**
   - RTT（往返时间）变化监测
   - 网络抖动和丢包检测
   - 网络质量综合评分
   - 多目标一致性分析

## 安装

### 从源码编译

```bash
# 克隆仓库
git clone <repository-url>
cd network_stable_test

# 编译项目
cargo build --release

# 二进制文件位于
./target/release/nst
```

### 依赖要求

- Rust 1.70+
- Linux/macOS系统

## 使用方法

### 基本命令

```bash
# 查看帮助
nst --help

# TCP连接稳定性测试
nst tcp-stability -p 127.0.0.1:1080 -t 8.8.8.8:53 -i 30 -d 300

# 带宽测试
nst bandwidth -p 127.0.0.1:1080 -t httpbin.org:80 -s 1024 -d 60

# 连接性能测试
nst connection-perf -p 127.0.0.1:1080 -t 8.8.8.8:53 -c 10 -n 100

# 运行所有测试
nst all -p 127.0.0.1:1080
```

### 命令参数说明

#### TCP稳定性测试 (`tcp-stability`)
- `-p, --proxy`: SOCKS5代理地址 (默认: 127.0.0.1:1080)
- `-t, --target`: 目标服务器地址 (默认: 8.8.8.8:53)
- `-i, --interval`: 心跳间隔(秒) (默认: 30)
- `-d, --duration`: 测试持续时间(秒) (默认: 300)

#### 带宽测试 (`bandwidth`)
- `-p, --proxy`: SOCKS5代理地址 (默认: 127.0.0.1:1080)
- `-t, --target`: 目标服务器地址 (默认: httpbin.org:80)
- `-s, --size`: 数据块大小(字节) (默认: 1024)
- `-d, --duration`: 测试持续时间(秒) (默认: 60)

#### 连接性能测试 (`connection-perf`)
- `-p, --proxy`: SOCKS5代理地址 (默认: 127.0.0.1:1080)
- `-t, --target`: 目标服务器地址 (默认: 8.8.8.8:53)
- `-c, --concurrent`: 并发连接数 (默认: 10)
- `-n, --total`: 总连接数 (默认: 100)

### 全局选项
- `-c, --config`: 指定配置文件路径
- `-v, --verbose`: 启用详细日志输出

## 配置文件

支持JSON格式的配置文件：

```json
{
  "proxy": {
    "host": "127.0.0.1",
    "port": 1080,
    "username": null,
    "password": null,
    "timeout_ms": 5000
  },
  "tests": {
    "tcp_stability": {
      "heartbeat_interval_ms": 30000,
      "test_duration_sec": 300,
      "max_retries": 3,
      "targets": ["8.8.8.8:53", "1.1.1.1:53"]
    },
    "bandwidth": {
      "chunk_size": 1024,
      "test_duration_sec": 60,
      "targets": ["httpbin.org:80"],
      "upload_test": true,
      "download_test": true
    },
    "connection_perf": {
      "concurrent_connections": 10,
      "total_connections": 100,
      "connection_timeout_ms": 5000,
      "targets": ["8.8.8.8:53"]
    },
    "dns_stability": {
      "domains": ["google.com", "github.com", "cloudflare.com"],
      "query_interval_ms": 1000,
      "test_duration_sec": 60
    },
    "network_jitter": {
      "ping_interval_ms": 1000,
      "test_duration_sec": 60,
      "targets": ["8.8.8.8:53", "1.1.1.1:53"]
    }
  },
  "reporting": {
    "output_format": "Json",
    "output_file": null,
    "real_time_metrics": true,
    "detailed_logs": false
  }
}
```

## 输出示例

### TCP稳定性测试结果
```
=== TCP Stability Test Results ===
Test Duration: 300s
Heartbeat Interval: 30s

Connection Statistics:
  Total Heartbeats: 10
  Successful: 9 (90.00%)
  Failed: 1 (10.00%)
  Reconnections: 1

Latency Statistics:
  Average RTT: 45ms
  Min RTT: 23ms
  Max RTT: 125ms

Connection Stability:
  Total Downtime: 2.5s
  Connection Drops: 1
  Uptime: 99.17%

Overall Stability Score: 88.5/100
```

### 带宽测试结果
```
=== Bandwidth Test Results ===
Test Duration: 60s
Chunk Size: 1024 bytes

Data Transfer Statistics:
  Total Bytes Sent: 1048576 (1.00 MB)
  Total Bytes Received: 2097152 (2.00 MB)

Speed Statistics:
  Average Upload Speed: 17.48 KB/s (0.14 Mbps)
  Average Download Speed: 34.95 KB/s (0.28 Mbps)
  Upload Speed Range: 12.50 - 22.75 KB/s
  Download Speed Range: 25.60 - 45.30 KB/s

Connection Quality:
  Connection Interruptions: 0
  Data Integrity Errors: 0
  Error Rate: 0.00%
  Bandwidth Stability Score: 92.3/100
```

## 评分系统

工具提供0-100分的综合评分：

- **90-100分**: 优秀 (Excellent) - 网络极其稳定
- **80-89分**: 良好 (Good) - 网络稳定，偶有小问题
- **70-79分**: 一般 (Fair) - 网络基本稳定，有明显问题
- **60-69分**: 较差 (Poor) - 网络不稳定，问题较多
- **0-59分**: 很差 (Very Poor) - 网络极不稳定

各项测试权重：
- TCP稳定性: 25%
- 带宽测试: 20%
- 连接性能: 20%
- DNS稳定性: 15%
- 网络抖动: 20%

## 技术实现

### 核心技术栈
- **Rust**: 系统编程语言，保证性能和安全
- **Tokio**: 异步运行时，支持高并发测试
- **Clap**: 命令行参数解析
- **Serde**: 数据序列化和配置管理
- **Chrono**: 时间处理和统计

### SOCKS5实现
工具内置了完整的SOCKS5客户端实现：
- 支持无认证和用户名/密码认证
- 支持IPv4/IPv6和域名解析
- 完整的错误处理和超时控制
- 连接复用和状态管理

### 异步架构
- 所有网络操作都是异步的
- 支持高并发连接测试
- 精确的时间测量和统计
- 资源高效利用

## 使用场景

1. **SOCKS5代理质量评估**
   - 评估代理服务商的服务质量
   - 对比不同代理的性能表现
   - 监控代理服务的稳定性变化

2. **网络环境诊断**
   - 诊断网络连接问题
   - 识别网络瓶颈和不稳定因素
   - 验证网络配置的有效性

3. **自动化监控**
   - 集成到监控系统中
   - 定期检测代理服务状态
   - 性能趋势分析和报警

4. **性能基准测试**
   - 建立网络性能基准
   - 测试不同配置的性能差异
   - 优化网络参数设置

## 故障排除

### 常见问题

1. **连接超时**
   - 检查SOCKS5代理地址和端口
   - 验证代理服务是否正常运行
   - 检查防火墙设置

2. **认证失败**
   - 确认用户名和密码正确
   - 检查代理是否需要认证
   - 验证认证方式配置

3. **目标不可达**
   - 确认目标服务器地址正确
   - 检查目标服务是否可用
   - 验证代理是否支持目标协议

### 调试模式

使用 `-v` 参数启用详细日志：
```bash
nst tcp-stability -v -p 127.0.0.1:1080
```

## 贡献

欢迎提交Issue和Pull Request！

## 开源协议

MIT License