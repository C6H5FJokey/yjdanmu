# 月计式弹幕机

## 项目简介

这是一个基于 Tauri 框架开发的桌面弹幕接收器，支持从 Bilibili 直播间获取弹幕并以字幕形式展示。系统采用现代化的前后端架构，前端使用 Vue.js + Element Plus，后端使用 Rust 实现 WebSocket 连接和 SSE 服务器。

## 功能特性

### 弹幕接收
- **Bilibili 弹幕监听**: 连接 Bilibili 直播间，实时接收弹幕消息
- **WebSocket 连接**: 稳定的 WebSocket 连接机制
- **消息过滤**: 支持过滤系统消息和重复消息

### 字幕展示
- **打字机效果**: 逐字显示，从左到右输出
- **抖动动画**: 字符轻微抖动，支持定格动画效果
- **淡出消失**: 字幕在指定时间后淡出并缩小
- **随机倾斜**: 字幕可随机倾斜指定角度
- **位置控制**: 字幕在屏幕范围内随机定位

### 参数配置
- 字体大小、颜色、描边可调
- 打字速度可调
- 显示时长可调
- 抖动幅度可调
- 缩放比例可调
- 随机倾斜角度可调

### 系统集成
- 桌面应用界面
- 系统托盘集成
- 本地化配置保存

## 技术架构

### 前端
- Vue.js 3 + Element Plus
- Vite 构建工具
- Tauri 提供的系统 API
- 实时动态字幕渲染

### 后端
- Rust + Tauri 框架
- WebSocket 客户端连接 Bilibili
- SSE 服务器推送消息
- 异步并发处理

## 安装部署

### 环境要求
- Node.js >= 18.0.0
- Rust >= 1.77.2
- npm 或 yarn

### 安装步骤

1. 克隆项目
```bash
git clone git@github.com:C6H5FJokey/yjdanmu.git
cd yjdanmu
```

2. 安装前端依赖
```bash
cd src
npm install
cd ..
```

3. 安装 Tauri CLI
```bash
cargo install tauri-cli
```

4. 启动开发模式
```bash
# 在项目根目录
npm run tauri-dev
```

## 使用说明

### 开发模式
1. 启动开发服务器: `npm run tauri-dev`
2. 在配置面板中输入 Bilibili 直播间房间号
3. 点击"连接"按钮开始接收弹幕
4. 弹幕将以动态字幕形式展示

### 生产构建
```bash
# 构建桌面应用
npm run tauri-build
```

## 项目结构

```
yjdanmu/
├── src/                  # 前端代码 (Vue.js)
│   ├── components/       # Vue 组件
│   ├── App.vue          # 主应用组件
│   ├── main.js          # 前端入口
│   └── package.json     # 前端依赖
├── src-tauri/            # 后端代码 (Rust)
│   ├── src/             # Rust 源码
│   │   ├── main.rs      # 主程序入口
│   │   ├── lib.rs       # Tauri 命令定义
│   │   ├── sse_server.rs # SSE 服务器
│   │   └── bili_websocket_client.rs # B站WebSocket客户端
│   ├── Cargo.toml       # Rust 依赖
│   ├── tauri.conf.json  # Tauri 配置
│   └── capabilities/    # Tauri 权限配置
├── package.json         # 根目录配置
└── README.md            # 项目说明
```

## 开发指南

### 添加新功能
1. 在 `src-tauri/src/lib.rs` 中定义 Tauri 命令
2. 在前端组件中调用对应命令
3. 确保遵守类型安全和异步处理规范

### 调试技巧
- 使用 `npm run tauri-dev` 进行热重载开发
- 前端控制台输出可在 Tauri 应用中查看
- Rust 错误日志可在终端中查看

## 许可证

[MIT License](LICENSE)