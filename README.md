# miniftp
## Demo
[![Watch the video](Watch the video)](https://user-images.githubusercontent.com/37993728/181192629-e53c2053-1c60-4415-8727-382ae99c94af.mp4)

## 介绍

一个 Rust 实现的 Linux Light FTP Server

## 支持的功能

- 大部分的 FTP 命令
- 支持主被动传输模式
- 支持用户自定义配置信息
- 支持指定被动模式下数据端口的范围，考虑到了主机配置有防火墙的情况
- 支持文件上传/下载的断点续传
- 支持限速功能，防止服务过多占用带宽资源
- 限流，防 DDOS 攻击
- 空闲连接自动剔除

## 快速运行此代码
## 可执行程序
下载 release 中的可执行程序包，解压，   运行

### 源码编译

#### 环境依赖
1. Linux 主机
2. Cargo 编译器


#### 编译步骤

1. 在当前目录下，执行 `cargo build --release`，此时在`./target/release/` 生产可执行文件 `miniftp`
2. 按照需求设置配置文件 `config.yaml` 中的选项
3. 运行程序 `sudo ./target/release/miniftp -c config.yaml`
4. 若主机打开的有防火墙，请确保配置文件用到的端已在防火墙中打开
5. 客户端连接到此服务器，默认的端口是 8089
6. 示例：
 - 服务器端启动服务
 ```bash
 cargo build --release
 sudo ./target/release/miniftp -c config.yaml
 ```
 - 本地 FTP 客户端连接到服务器 若没有安装 ftp 客户端，可运行 sudo apt install -y ftp 安装ftp客户端
 ```bash
 > ftp
 > open 127.0.0.1 8089
 ```
 
