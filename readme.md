# 高级字幕系统

## 项目简介

这是一个基于 SSE (Server-Sent Events) 协议的高级字幕系统，支持丰富的视觉效果和实时弹幕推送。系统采用前后端分离架构，后端提供 SSE 推送服务，前端实现动态字幕渲染。

## 功能特性

### 字幕效果
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

### 开发模式
- 参数配置面板
- 实时预览效果
- 开发/生产环境切换

## 技术架构

### 前端
- HTML5 + CSS3 + JavaScript
- SSE 客户端连接
- 动画效果使用 CSS3 实现
- 响应式布局

### 后端
- Node.js + Express
- SSE 服务器实现
- CORS 支持
- 实时消息推送

## 安装部署

### 环境要求
- Node.js >= 14.0.0
- npm

### 安装步骤

1. 克隆项目
```bash
git clone git@github.com:C6H5FJokey/yjdanmu.git
cd yjdanmu
```

2. 安装依赖
```bash
npm install
```

3. 启动服务
```bash
# 生产模式
npm start

# 开发模式
npm run dev

# 指定端口(windows上请务必使用cmd而非pwsh，会吞参数且原因不明)
npm start -- --port 8180
```
或者直接双击`start.bat`
## API 接口

### SSE 连接
- **端点**: `/api/sse`
- **方法**: GET
- **功能**: 建立 SSE 连接，接收实时字幕推送

### 发送弹幕
- **端点**: `/api/send-danmu`
- **方法**: POST
- **Content-Type**: application/json
- **功能**: 发送自定义弹幕到前端

**请求体示例**:
```json
{
    "type": "danmu",
    "text": "测试弹幕",
    "color": "#ff6b6b",
    "size": 28,
    "strokeColor": "#000000",
    "strokeWidth": 2,
    "typingSpeed": 100,
    "displayDuration": 3000,
    "fadeDuration": 1000,
    "shakeAmplitude": 2,
    "randomTilt": 10
}
```

### 状态查询
- **端点**: `/api/status`
- **方法**: GET
- **功能**: 查询服务器连接状态

## 使用说明

### 基本使用
1. 启动服务后访问 `http://localhost:8180`
2. 在控制面板中点击"连接"建立 SSE 连接
3. 通过 API 或开发面板发送弹幕

### 开发模式
- 启动开发模式: `npm run dev`
- 或访问 `http://localhost:8180?dev=true`
- 使用参数配置面板调整字幕效果

### 环境变量
- `PORT`: 指定服务端口 (默认: 8080)
- `NODE_ENV=development`: 启用开发模式

## 配置参数

### 字幕参数
- `text`: 弹幕文本内容
- `color`: 字体颜色
- `size`: 字体大小
- `strokeColor`: 描边颜色
- `strokeWidth`: 描边宽度
- `typingSpeed`: 打字速度 (毫秒)
- `displayDuration`: 显示时长 (毫秒)
- `fadeDuration`: 淡出时长 (毫秒)
- `shakeAmplitude`: 抖动幅度
- `randomTilt`: 随机倾斜角度
- `shrinkScale`: 淡出时缩放比例

## 项目结构

```
advanced-subtitle-system/
├── server.js           # 后端服务
├── index.html          # 前端页面
├── package.json        # 项目配置
├── README.md          # 项目说明
└── start.bat          # 启动脚本
```

## 扩展功能

### 自动重连
SSE 连接具备自动重连机制，确保连接稳定性。

## 许可证

[MIT License](LICENSE)