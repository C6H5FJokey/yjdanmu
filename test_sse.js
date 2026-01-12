// 简单的测试脚本，用于验证SSE服务器功能
const { spawn } = require('child_process');

console.log('启动Tauri应用进行功能测试...');

// 启动Tauri应用
const tauriProcess = spawn('cargo', ['tauri', 'dev'], {
  cwd: 'c:\\Users\\46744\\Documents\\code\\yjdanmu\\tauri-app\\src-tauri',
  shell: true
});

tauriProcess.stdout.on('data', (data) => {
  console.log(`Tauri输出: ${data}`);
  
  // 检查SSE服务器是否已启动
  if (data.includes('SSE服务器启动在 http://127.0.0.1:8081')) {
    console.log('SSE服务器已成功启动！');
    
    // 等待几秒让服务器完全准备好
    setTimeout(() => {
      testSendDanmu();
    }, 3000);
  }
});

tauriProcess.stderr.on('data', (data) => {
  console.error(`Tauri错误: ${data}`);
});

tauriProcess.on('close', (code) => {
  console.log(`Tauri进程已退出，退出码: ${code}`);
});

// 测试发送弹幕功能
async function testSendDanmu() {
  console.log('测试发送弹幕功能...');
  
  try {
    const response = await fetch('http://127.0.0.1:8081/api/send-danmu', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify({
        type: 'danmu',
        text: '测试弹幕 - 功能正常！',
        fontSize: 28,
        color: '#00ff00',
        strokeColor: '#000000',
        strokeWidth: 2,
        typingSpeed: 50,
        displayDuration: 5000
      })
    });
    
    if (response.ok) {
      console.log('弹幕发送成功！');
      console.log('响应:', await response.json());
    } else {
      console.error('弹幕发送失败:', response.status, response.statusText);
    }
  } catch (error) {
    console.error('发送弹幕时发生错误:', error);
  }
}