<template>
  <div class="websocket-settings">
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <span>WebSocket连接设置</span>
        </div>
      </template>
      
      <!-- 连接状态 -->
      <div class="status-section">
        <el-tag :type="connectionStatusType" class="status-tag">
          {{ connectionStatusText }}
        </el-tag>
      </div>
      
      <!-- 连接设置 -->
      <div class="connection-section">
        <h3>连接配置</h3>
        <el-form :model="connectionConfig" label-width="120px">
          <el-form-item label="连接类型">
            <el-radio-group v-model="connectionConfig.roomKeyType">
              <el-radio label="RoomId">房间ID</el-radio>
              <el-radio label="AuthCode">用户码</el-radio>
            </el-radio-group>
          </el-form-item>
          <el-form-item :label="connectionConfig.roomKeyType === 'RoomId' ? '房间ID' : '用户码'">
            <el-input 
              v-model="connectionConfig.roomKey" 
              placeholder="请输入房间ID或用户码"
            />
          </el-form-item>

          <!-- OpenLive 凭据（仅用户码模式需要） -->
          <template v-if="connectionConfig.roomKeyType === 'AuthCode'">
            <el-form-item label="OpenLive App ID">
              <el-input
                v-model="connectionConfig.openLiveAppId"
                placeholder="可选：不填则后端读取环境变量 BILI_OPEN_LIVE_APP_ID"
              />
            </el-form-item>
            <el-form-item label="Access Key ID">
              <el-input
                v-model="connectionConfig.openLiveAccessKeyId"
                placeholder="可选：不填则后端读取环境变量 BILI_OPEN_LIVE_ACCESS_KEY_ID"
              />
            </el-form-item>
            <el-form-item label="Access Key Secret">
              <el-input
                v-model="connectionConfig.openLiveAccessKeySecret"
                type="password"
                show-password
                placeholder="可选：不填则后端读取环境变量 BILI_OPEN_LIVE_ACCESS_KEY_SECRET"
              />
            </el-form-item>
          </template>
          
          <el-form-item label="重连间隔(ms)">
            <el-input-number 
              v-model="connectionConfig.reconnectInterval" 
              :min="1000" 
              :max="60000" 
              :step="1000"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="最大重连次数">
            <el-input-number 
              v-model="connectionConfig.maxReconnectAttempts" 
              :min="0" 
              :max="100" 
              :step="1"
              controls-position="right"
            />
          </el-form-item>
        </el-form>
      </div>
      
      <!-- 操作按钮 -->
      <div class="action-section">
        <el-button 
          @click="connectWebSocket" 
          type="primary"
          :disabled="!canUseTauri || !canConnect"
        >
          连接
        </el-button>
        <el-button 
          @click="disconnectWebSocket" 
          type="danger"
          :disabled="!canUseTauri || !canDisconnect"
        >
          断开连接
        </el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue'
import { ElMessage } from 'element-plus'
import { listen } from '@tauri-apps/api/event'

const canUseTauri = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__

const Phase = Object.freeze({
  IDLE: 'idle',
  CONNECTING: 'connecting',
  CONNECTED: 'connected',
  DISCONNECTING: 'disconnecting',
  RECONNECTING: 'reconnecting',
  ERROR: 'error'
})

const phase = ref(Phase.IDLE)
const connectionStatus = ref('未连接')
let unlisten = null

const connectionConfig = reactive({
  roomKey: '',
  roomKeyType: "RoomId",
  reconnectInterval: 3000,
  maxReconnectAttempts: 5,
  openLiveAppId: '',
  openLiveAccessKeyId: '',
  openLiveAccessKeySecret: ''
})

const connectionStatusType = computed(() => {
  switch (phase.value) {
    case Phase.CONNECTED:
      return 'success'
    case Phase.RECONNECTING:
      return 'warning'
    case Phase.ERROR:
      return 'danger'
    case Phase.CONNECTING:
    case Phase.DISCONNECTING:
      return 'info'
    default:
      return 'info'
  }
})

const connectionStatusText = computed(() => {
  return connectionStatus.value
})

const canConnect = computed(() => {
  return phase.value === Phase.IDLE || phase.value === Phase.ERROR
})

const canDisconnect = computed(() => {
  return (
    phase.value === Phase.CONNECTING ||
    phase.value === Phase.CONNECTED ||
    phase.value === Phase.RECONNECTING
  )
})

const connectWebSocket = async () => {
  try {
    if (!canUseTauri) {
      ElMessage.info('此功能仅在Tauri环境中可用')
      return
    }
    if (!connectionConfig.roomKey.trim()) {
      ElMessage.warning('请输入房间ID或用户码')
      return
    }

    phase.value = Phase.CONNECTING
    connectionStatus.value = '连接中...'

    const tauriAPI = window.__TAURI_INTERNALS__.invoke
    const result = await tauriAPI('connect_websocket', { 
        roomKey: connectionConfig.roomKey,
        roomKeyType: connectionConfig.roomKeyType,
        reconnectInterval: connectionConfig.reconnectInterval,
        maxReconnectAttempts: connectionConfig.maxReconnectAttempts,
        openLiveAppId: connectionConfig.openLiveAppId ? Number(connectionConfig.openLiveAppId) : null,
        openLiveAccessKeyId: connectionConfig.openLiveAccessKeyId || null,
        openLiveAccessKeySecret: connectionConfig.openLiveAccessKeySecret || null
      })
    console.log('WebSocket连接结果:', result)
  } catch (error) {
    console.error('WebSocket连接失败:', error)
    phase.value = Phase.ERROR
    connectionStatus.value = `连接失败: ${error}`
    ElMessage.error(`连接失败: ${error}`)
  }
}

const disconnectWebSocket = async () => {
  try {
    if (!canUseTauri) {
      ElMessage.info('此功能仅在Tauri环境中可用')
      return
    }

    phase.value = Phase.DISCONNECTING
    connectionStatus.value = '断开中...'

    const tauriAPI = window.__TAURI_INTERNALS__.invoke
    const result = await tauriAPI('disconnect_websocket')
    console.log('WebSocket断开结果:', result)

    // 后端也会 emit websocket-status=disconnected，这里先乐观更新，避免 UI 卡住
    phase.value = Phase.IDLE
    connectionStatus.value = '未连接'
    ElMessage.success('WebSocket已断开')
  } catch (error) {
    console.error('WebSocket断开失败:', error)
    phase.value = Phase.ERROR
    connectionStatus.value = `断开失败: ${error}`
    ElMessage.error('断开失败')
  }
}

onMounted(async () => {
  if (!canUseTauri) return

  // 读取通用设置作为默认重连参数（避免每次手动改）
  try {
    const tauriAPI = window.__TAURI_INTERNALS__.invoke
    const resp = await tauriAPI('get_general_settings')
    const s = resp.settings
    if (s && !connectionConfig.roomKey) {
      if (typeof s.defaultReconnectInterval === 'number') {
        connectionConfig.reconnectInterval = s.defaultReconnectInterval
      }
      if (typeof s.defaultMaxReconnectAttempts === 'number') {
        connectionConfig.maxReconnectAttempts = s.defaultMaxReconnectAttempts
      }
    }
  } catch (e) {
    // 忽略：不影响 WS 功能
    console.debug('读取通用设置失败（可忽略）:', e)
  }

  try {
    unlisten = await listen('websocket-status', (event) => {
      const { status, message } = event.payload
      console.log('WebSocket状态:', status, message)
      
      switch (status) {
        case 'connected':
          phase.value = Phase.CONNECTED
          connectionStatus.value = message
          ElMessage.success(message)
          break
        case 'disconnected':
          phase.value = Phase.IDLE
          connectionStatus.value = message
          ElMessage.warning(message)
          break
        case 'reconnecting':
          phase.value = Phase.RECONNECTING
          connectionStatus.value = message
          ElMessage.info(message)
          break
        case 'connecting':
          phase.value = Phase.CONNECTING
          connectionStatus.value = message
          break
        case 'error':
          phase.value = Phase.ERROR
          connectionStatus.value = message
          ElMessage.error(message)
          break
      }
    })
  } catch (error) {
    console.error('监听WebSocket状态失败:', error)
  }
})

onUnmounted(() => {
  if (unlisten) {
    unlisten()
  }
})
</script>

<style scoped>
.websocket-settings {
  padding: 20px;
  height: 100%;
  overflow-y: auto;
}

.settings-card {
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

.connection-section {
  margin-bottom: 30px;
  padding-bottom: 20px;
  border-bottom: 1px solid #eee;
}

.connection-section h3 {
  margin-top: 0;
  margin-bottom: 15px;
}

.action-section {
  display: flex;
  gap: 10px;
}
</style>