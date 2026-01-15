<template>
  <div class="control-panel">
    <el-card class="control-card">
      <template #header>
        <div class="card-header">
          <span>弹幕控制面板</span>
        </div>
      </template>
      
      <!-- 服务状态 -->
      <div class="status-section">
        <el-tag type="success" class="status-tag">
          服务运行正常
        </el-tag>
        <el-button 
          @click="checkStatus" 
          type="primary"
        >
          检查服务状态
        </el-button>
        <el-button 
          @click="openPreviewInBrowser" 
          type="primary"
        >
          打开预览页面
        </el-button>
        <el-button 
          @click="toggleAlwaysOnTop" 
          :type="isAlwaysOnTop ? 'success' : 'default'"
        >
          {{ isAlwaysOnTop ? '取消置顶' : '窗口置顶' }}
        </el-button>
      </div>
      
      <!-- 发送弹幕区域 -->
      <div class="send-section">
        <h3>发送弹幕</h3>
        <el-input 
          v-model="danmuText" 
          placeholder="输入弹幕内容" 
          maxlength="100"
          show-word-limit
          class="danmu-input"
          @keyup.enter="sendDanmu"
        />
        <el-button @click="sendDanmu" type="primary" class="send-btn">发送</el-button>
      </div>
      
      <!-- 样式设置区域 -->
      <div class="style-section">
        <h3>样式设置</h3>
        <el-form :model="styleSettings" label-width="120px">
          <el-form-item label="字体大小">
            <el-input-number 
              v-model="styleSettings.fontSize" 
              :min="10" 
              :max="100" 
              :step="2"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="字体颜色">
            <el-color-picker v-model="styleSettings.color" />
          </el-form-item>
          
          <el-form-item label="描边颜色">
            <el-color-picker v-model="styleSettings.strokeColor" />
          </el-form-item>
          
          <el-form-item label="描边宽度">
            <el-input-number 
              v-model="styleSettings.strokeWidth" 
              :min="0" 
              :max="10" 
              :step="1"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="打字速度(ms)">
            <el-input-number 
              v-model="styleSettings.typingSpeed" 
              :min="10" 
              :max="500" 
              :step="10"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="显示时长(ms)">
            <el-input-number 
              v-model="styleSettings.displayDuration" 
              :min="1000" 
              :max="10000" 
              :step="500"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="消失时长(ms)">
            <el-input-number 
              v-model="styleSettings.fadeDuration" 
              :min="100" 
              :max="10000" 
              :step="100"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="抖动幅度">
            <el-input-number 
              v-model="styleSettings.shakeAmplitude" 
              :min="0" 
              :max="10" 
              :step="0.5"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="随机倾斜角度">
            <el-input-number 
              v-model="styleSettings.randomTilt" 
              :min="0" 
              :max="45" 
              :step="1"
              controls-position="right"
            />
          </el-form-item>
        </el-form>
        
        <el-button @click="applyStyleSettings" type="primary">应用样式设置</el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, reactive } from 'vue'
import { ElMessage } from 'element-plus'

// 动态导入Tauri API（仅在Tauri环境中可用）
const tauriAPI = typeof window !== 'undefined' && window.__TAURI_INTERNALS__ ? window.__TAURI_INTERNALS__.invoke : null;

const danmuText = ref('')
const isAlwaysOnTop = ref(false)

// 样式设置
const styleSettings = reactive({
  fontSize: 32,
  color: '#ffffff',
  strokeColor: '#000000',
  strokeWidth: 2,
  typingSpeed: 100,
  displayDuration: 3000,
  fadeDuration: 1000,
  shakeAmplitude: 2,
  randomTilt: 10
})

const normalizePreviewHost = (bindAddr) => {
  if (!bindAddr) return null
  // 后端可能返回 0.0.0.0:PORT，这个地址不能直接在浏览器中访问
  if (String(bindAddr).startsWith('0.0.0.0:')) {
    return `127.0.0.1:${String(bindAddr).split(':')[1]}`
  }
  return String(bindAddr)
}

const getPreviewUrl = async () => {
  // 优先从后端读取当前 runtime bind addr + token
  if (tauriAPI) {
    const resp = await tauriAPI('get_general_settings')
    const bind = normalizePreviewHost(resp.runtimeBindAddr)
    const token = resp.settings?.sseToken || ''
    const base = bind ? `http://${bind}` : 'http://127.0.0.1:8081'
    const url = new URL('/preview.html', base)
    if (token) url.searchParams.set('token', token)
    return url.toString()
  }

  // 非 Tauri：允许用 localStorage 覆盖（方便 dev）
  const base = localStorage.getItem('yjdanmu.devSseBase') || 'http://127.0.0.1:8081'
  const token = localStorage.getItem('yjdanmu.sseToken') || ''
  const url = new URL('/preview.html', base)
  if (token) url.searchParams.set('token', token)
  return url.toString()
}

const getSendDanmuUrl = async () => {
  if (tauriAPI) {
    const resp = await tauriAPI('get_general_settings')
    const bind = normalizePreviewHost(resp.runtimeBindAddr)
    const token = resp.settings?.sseToken || ''
    const base = bind ? `http://${bind}` : 'http://127.0.0.1:8081'
    const url = new URL('/api/send-danmu', base)
    if (token) url.searchParams.set('token', token)
    return url.toString()
  }
  const base = localStorage.getItem('yjdanmu.devSseBase') || 'http://127.0.0.1:8081'
  const token = localStorage.getItem('yjdanmu.sseToken') || ''
  const url = new URL('/api/send-danmu', base)
  if (token) url.searchParams.set('token', token)
  return url.toString()
}

// 检查服务状态
const checkStatus = async () => {
  try {
    if (tauriAPI) {
      const result = await tauriAPI('get_status')
      console.log('服务状态:', result)
      ElMessage.success('服务状态正常')
    } else {
      ElMessage.info('Tauri环境不可用，跳过状态检查')
    }
  } catch (error) {
    console.error('获取状态失败:', error)
    ElMessage.error('服务状态异常')
  }
}

// 发送弹幕
const sendDanmu = async () => {
  if (!danmuText.value.trim()) {
    ElMessage.warning('请输入弹幕内容')
    return
  }
  
  const customData = {
    user: '匿名用户',
    ...styleSettings
  }
  
  try {
    if (tauriAPI) {
      const result = await tauriAPI('send_danmu', { 
        text: danmuText.value,
        customData: customData
      })
      console.log('弹幕发送结果:', result)
    } else {
      // 在非Tauri环境下，直接发送到SSE端点
      const url = await getSendDanmuUrl()
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          type: 'danmu',
          text: danmuText.value,
          ...customData
        })
      });
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      console.log('弹幕发送结果:', await response.json())
    }
    danmuText.value = ''
    ElMessage.success('弹幕发送成功')
  } catch (error) {
    console.error('发送弹幕失败:', error)
    ElMessage.error('发送弹幕失败')
  }
}

// 打开预览页面在外部浏览器
const openPreviewInBrowser = async () => {
  try {
    const url = await getPreviewUrl()
    if (tauriAPI) {
      const result = await tauriAPI('open_in_browser', { url })
      console.log('预览页面打开结果:', result)
      ElMessage.success('预览页面已在浏览器中打开')
    } else {
      // 在非Tauri环境下，使用window.open
      window.open(url, '_blank')
      ElMessage.info('预览页面已打开')
    }
  } catch (error) {
    console.error('打开预览页面失败:', error)
    ElMessage.error('打开预览页面失败')
  }
}

// 应用样式设置
const applyStyleSettings = async () => {
  try {
    if (tauriAPI) {
      const result = await tauriAPI('send_config', { config: styleSettings })
      console.log('样式设置应用结果:', result)
    } else {
      // 在非Tauri环境下，直接发送配置到SSE端点
      const response = await fetch('http://localhost:8081/api/config', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify({
          type: 'config',
          config: styleSettings
        })
      });
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      console.log('样式设置应用结果:', await response.json())
    }
    ElMessage.success('样式设置已应用')
  } catch (error) {
    console.error('应用样式设置失败:', error)
    ElMessage.error('应用样式设置失败')
  }
}

// 切换窗口置顶
const toggleAlwaysOnTop = async () => {
  try {
    if (tauriAPI) {
      const result = await tauriAPI('toggle_always_on_top')
      isAlwaysOnTop.value = result
      ElMessage.success(result ? '窗口已置顶' : '已取消置顶')
    } else {
      ElMessage.info('此功能仅在Tauri环境中可用')
    }
  } catch (error) {
    console.error('切换置顶状态失败:', error)
    ElMessage.error('切换置顶状态失败')
  }
}

</script>

<style scoped>

.control-card {
  min-height: 100%;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.status-section {
  margin-bottom: 20px;
  display: flex;
  align-items: center;
  gap: 10px;
}

.status-tag {
  min-width: 80px;
  text-align: center;
}

.send-section, .style-section {
  margin-bottom: 30px;
  padding-bottom: 20px;
  border-bottom: 1px solid #eee;
}

.send-section h3, .style-section h3 {
  margin-top: 0;
  margin-bottom: 15px;
}

.danmu-input {
  width: 70%;
  margin-right: 10px;
}

.send-btn {
  vertical-align: top;
}

.style-section .el-form {
  margin-top: 15px;
}

.style-buttons {
  display: flex;
  gap: 10px;
}
</style>