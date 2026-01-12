const express = require('express');
const cors = require('cors');
const path = require('path');
const fs = require('fs');

const app = express();
const portIndex = process.argv.indexOf('--port');
const PORT = parseInt(process.argv[portIndex + 1]) || 8180;

// 中间件
app.use(cors());
app.use(express.json());

// 检查是否为开发模式
const isDevelopment = process.argv.includes('--dev');

// 存储所有SSE连接
let sseConnections = [];

// SSE连接端点
app.get('/api/sse', (req, res) => {
    // 设置SSE响应头
    res.setHeader('Content-Type', 'text/event-stream');
    res.setHeader('Cache-Control', 'no-cache');
    res.setHeader('Connection', 'keep-alive');
    res.setHeader('Access-Control-Allow-Origin', '*');
    
    // 生成连接ID
    const connectionId = Date.now().toString();
    
    // 创建连接对象
    const connection = {
        id: connectionId,
        response: res,
        lastActivity: Date.now()
    };
    
    sseConnections.push(connection);
    console.log(`SSE连接建立: ${connectionId}, 当前连接数: ${sseConnections.length}`);
    
    // 发送连接确认消息
    res.write(`data: {"type":"connected","id":"${connectionId}"}\n\n`);
    
    // 如果是开发模式，启动测试弹幕
    if (isDevelopment) {
        const testInterval = setInterval(() => {
            if (sseConnections.some(conn => conn.id === connectionId)) {
                const testDanmu = {
                    type: 'danmu',
                    text: `开发测试弹幕 ${Math.floor(Math.random() * 1000)}`,
                    user: '开发测试用户',
                    color: ['#ff6b6b', '#4ecdc4', '#45b7d1', '#96ceb4', '#feca57'][Math.floor(Math.random() * 5)],
                    size: 24 + Math.random() * 16,
                    time: Date.now(),
                    timestamp: new Date().toLocaleTimeString()
                };
                
                res.write(`data: ${JSON.stringify(testDanmu)}\n\n`);
            } else {
                clearInterval(testInterval);
            }
        }, 3000);
    }
    
    // 心跳检测
    const heartbeatInterval = setInterval(() => {
        if (res.writableEnded) {
            clearInterval(heartbeatInterval);
            return;
        }
        res.write(`data: {"type":"heartbeat","timestamp":${Date.now()}}\n\n`);
    }, 30000);
    
    // 连接关闭处理
    req.on('close', () => {
        console.log(`SSE连接关闭: ${connectionId}`);
        sseConnections = sseConnections.filter(conn => conn.id !== connectionId);
        clearInterval(heartbeatInterval);
    });
    
    req.on('error', (err) => {
        console.error(`SSE连接错误: ${connectionId}`, err);
        sseConnections = sseConnections.filter(conn => conn.id !== connectionId);
        clearInterval(heartbeatInterval);
    });
});

// 发送SSE消息的函数
function sendSSEMessage(data) {
    const message = `data: ${JSON.stringify(data)}\n\n`;
    
    sseConnections = sseConnections.filter(connection => {
        if (connection.response.writableEnded) {
            console.log(`连接已结束，移除: ${connection.id}`);
            return false;
        }
        
        try {
            connection.response.write(message);
            connection.lastActivity = Date.now();
            return true;
        } catch (error) {
            console.error(`发送SSE消息失败: ${connection.id}`, error);
            return false;
        }
    });
}

// 接收外部发送的弹幕
app.post('/api/send-danmu', (req, res) => {
    const danmuData = req.body;
    
    if (!danmuData.text) {
        return res.status(400).json({ error: '缺少text字段' });
    }
    
    // 设置默认值
    const danmu = {
        type: 'danmu',
        text: danmuData.text,
        user: danmuData.user || '匿名用户',
        color: danmuData.color || '#ffffff',
        size: danmuData.size || 24,
        time: Date.now(),
        timestamp: new Date().toLocaleTimeString(),
        ...danmuData
    };
    
    sendSSEMessage(danmu);
    res.json({ success: true, message: '弹幕发送成功' });
});

// 获取连接状态
app.get('/api/status', (req, res) => {
    res.json({
        connections: sseConnections.length,
        timestamp: Date.now(),
        development: isDevelopment
    });
});

// 默认路由返回index.html
// 在服务端渲染index.html时替换SSE URL
app.get('/', (req, res) => {
    let html = fs.readFileSync(path.join(__dirname, 'index.html'), 'utf8');
    html = html.replace(
        'SSE_URL_PLACEHOLDER',
        `http://localhost:${PORT}/api/sse`
    );
    res.send(html);
});

// 提供静态文件
app.use(express.static(path.join(__dirname)));

// 启动服务器
const server = app.listen(PORT, () => {
    console.log(`服务器运行在 http://localhost:${PORT}`);
    console.log(`SSE端点: http://localhost:${PORT}/api/sse`);
    console.log(`开发模式: ${isDevelopment ? '开启' : '关闭'}`);
});

// 定期清理无效连接
// setInterval(() => {
//     const now = Date.now();
//     const timeout = 5 * 60 * 1000; // 5分钟超时
    
//     sseConnections = sseConnections.filter(connection => {
//         if (now - connection.lastActivity > timeout) {
//             console.log(`清理超时连接: ${connection.id}`);
//             return false;
//         }
//         return true;
//     });
// }, 60000);

module.exports = { app, server, sendSSEMessage };