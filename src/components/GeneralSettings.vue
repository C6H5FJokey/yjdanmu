<template>
  <div class="general-settings">
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <span>通用设置</span>
          <el-tag v-if="runtimeBindAddr" type="success">SSE: {{ runtimeBindAddr }}</el-tag>
          <el-tag v-else type="info">SSE 未启动</el-tag>
        </div>
      </template>

      <el-alert
        title="提示"
        type="info"
        :closable="false"
        show-icon
        description="sse服务相关在修改后会自动重启 SSE 服务以应用端口/公网/token。弹幕屏蔽则不会。"
      />

      <div class="section">
        <h3>SSE 服务</h3>
        <el-form :model="settings" label-width="140px">
          <el-form-item label="端口">
            <el-input-number v-model="settings.ssePort" :min="1" :max="65535" controls-position="right" />
          </el-form-item>
          <el-form-item label="开启公网 (0.0.0.0)">
            <el-switch v-model="settings.ssePublic" />
          </el-form-item>
          <el-form-item label="访问 Token (可选)">
            <el-input
              v-model="settings.sseToken"
              placeholder="为空则不鉴权；不为空则 /api/sse?token=... 才能连"
              clearable
            />
          </el-form-item>
        </el-form>
      </div>

      <div class="section">
        <h3>连接默认值</h3>
        <el-form :model="settings" label-width="140px">
          <el-form-item label="默认重连间隔(ms)">
            <el-input-number v-model="settings.defaultReconnectInterval" :min="1000" :max="60000" :step="1000" controls-position="right" />
          </el-form-item>
          <el-form-item label="默认最大重连次数">
            <el-input-number v-model="settings.defaultMaxReconnectAttempts" :min="0" :max="100" :step="1" controls-position="right" />
          </el-form-item>
        </el-form>
      </div>

      <div class="section">
        <h3>调试</h3>
        <el-form :model="settings" label-width="140px">
          <el-form-item label="开启 WS Debug">
            <el-switch v-model="settings.wsDebug" />
          </el-form-item>
        </el-form>
      </div>

      <div class="section">
        <h3>预览渲染</h3>
        <el-form :model="settings.renderSettings" label-width="140px">
          <el-form-item label="出队间隔(ms)">
            <el-input-number
              v-model="settings.renderSettings.minDispatchIntervalMs"
              :min="0"
              :max="5000"
              :step="10"
              controls-position="right"
              :disabled="settings.renderSettings.unlimitedDispatch"
            />
            <div class="hint">浏览器切回前台时用于平滑显示；越小越“追实时”</div>
          </el-form-item>

          <el-form-item label="无限制">
            <el-switch v-model="settings.renderSettings.unlimitedDispatch" />
            <div class="hint">开启后不做出队间隔限制（仍会让出事件循环）</div>
          </el-form-item>

          <el-form-item label="队列最大长度">
            <el-input-number
              v-model="settings.renderSettings.queueMaxLength"
              :min="0"
              :max="5000"
              :step="10"
              controls-position="right"
            />
            <div class="hint">超出则丢弃最旧（0 表示不限制）</div>
          </el-form-item>

          <el-form-item label="积压最大保留(ms)">
            <el-input-number
              v-model="settings.renderSettings.queueMaxAgeMs"
              :min="0"
              :max="600000"
              :step="1000"
              controls-position="right"
            />
            <div class="hint">超过则直接丢弃（0 表示不过期）</div>
          </el-form-item>

          <el-form-item label="切回前台丢弃过期">
            <el-switch v-model="settings.renderSettings.dropOnResume" />
          </el-form-item>
        </el-form>
      </div>

      <div class="section">
        <h3>弹幕过滤</h3>
        <el-form :model="settings.danmuFilter" label-width="140px">
          <el-form-item label="启用关键词过滤">
            <el-switch v-model="settings.danmuFilter.BlacklistEnabled" />
          </el-form-item>

          <el-form-item label="关键词黑名单">
            <el-input
              v-model="keywordText"
              type="textarea"
              :rows="4"
              placeholder="每行一个关键词；包含任意关键词则过滤"
            />
          </el-form-item>

          <el-form-item label="最短长度">
            <el-input-number v-model="settings.danmuFilter.minLen" :min="0" :max="200" controls-position="right" />
          </el-form-item>
          <el-form-item label="最长长度">
            <el-input-number v-model="settings.danmuFilter.maxLen" :min="0" :max="500" controls-position="right" />
          </el-form-item>

          <el-form-item label="仅粉丝牌弹幕">
            <el-switch v-model="settings.danmuFilter.onlyFansMedal" />
            <div class="hint">通过 media_ruid 与主播 uid 匹配判断</div>
          </el-form-item>

          <el-form-item label="仅主播弹幕">
            <el-switch v-model="settings.danmuFilter.onlyStreamer" />
            <div class="hint">免登录场景：通过 face_url 匹配主播头像 URL</div>
          </el-form-item>

          <el-form-item label="屏蔽主播弹幕">
            <el-switch v-model="settings.danmuFilter.hideStreamer" />
          </el-form-item>
        </el-form>
      </div>

      <div class="actions">
        <el-button type="primary" @click="apply" :disabled="!canUseTauri">应用</el-button>
        <el-button @click="reload" :disabled="!canUseTauri">撤销修改</el-button>
      </div>

      <el-alert
        v-if="!canUseTauri"
        title="当前不在 Tauri 环境"
        type="warning"
        :closable="false"
        show-icon
        description="通用设置需要在 Tauri 环境中调用后端命令。"
      />
    </el-card>
  </div>
</template>

<script setup>
import { reactive, ref, onMounted, watch } from 'vue'
import { ElMessage } from 'element-plus'

const canUseTauri = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__

const runtimeBindAddr = ref('')

const settings = reactive({
  ssePort: 8081,
  ssePublic: false,
  sseToken: '',
  wsDebug: false,
  defaultReconnectInterval: 3000,
  defaultMaxReconnectAttempts: 5,
  renderSettings: {
    minDispatchIntervalMs: 160,
    unlimitedDispatch: false,
    queueMaxLength: 200,
    queueMaxAgeMs: 15000,
    dropOnResume: true
  },
  danmuFilter: {
    BlacklistEnabled: false,
    keywordBlacklist: [],
    minLen: null,
    maxLen: null,
    onlyFansMedal: false,
    onlyStreamer: false,
    hideStreamer: false
  }
})

const keywordText = ref('')

watch(keywordText, () => {
  const lines = keywordText.value
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
  settings.danmuFilter.keywordBlacklist = lines
})

watch(
  () => settings.danmuFilter.keywordBlacklist,
  (list) => {
    keywordText.value = (list || []).join('\n')
  },
  { deep: true }
)

const reload = async () => {
  try {
    if (!canUseTauri) return
    const tauriAPI = window.__TAURI_INTERNALS__.invoke
    const resp = await tauriAPI('get_general_settings')
    runtimeBindAddr.value = resp.runtimeBindAddr || ''
    const s = resp.settings

    settings.ssePort = s.ssePort
    settings.ssePublic = s.ssePublic
    settings.sseToken = s.sseToken || ''
    settings.wsDebug = !!s.wsDebug
    settings.defaultReconnectInterval = s.defaultReconnectInterval
    settings.defaultMaxReconnectAttempts = s.defaultMaxReconnectAttempts
    settings.renderSettings = {
      ...settings.renderSettings,
      ...(s.renderSettings || {})
    }
    settings.danmuFilter = {
      ...settings.danmuFilter,
      ...(s.danmuFilter || {})
    }

    // 同步关键词文本
    keywordText.value = (settings.danmuFilter.keywordBlacklist || []).join('\n')
  } catch (e) {
    console.error(e)
    ElMessage.error(`加载设置失败: ${e}`)
  }
}

const apply = async () => {
  try {
    if (!canUseTauri) return

    const tauriAPI = window.__TAURI_INTERNALS__.invoke
    const payload = {
      ssePort: Number(settings.ssePort),
      ssePublic: !!settings.ssePublic,
      sseToken: settings.sseToken ? String(settings.sseToken) : null,
      wsDebug: !!settings.wsDebug,
      defaultReconnectInterval: Number(settings.defaultReconnectInterval),
      defaultMaxReconnectAttempts: Number(settings.defaultMaxReconnectAttempts),
      renderSettings: {
        minDispatchIntervalMs: Number(settings.renderSettings.minDispatchIntervalMs || 0),
        unlimitedDispatch: !!settings.renderSettings.unlimitedDispatch,
        queueMaxLength: Number(settings.renderSettings.queueMaxLength || 0),
        queueMaxAgeMs: Number(settings.renderSettings.queueMaxAgeMs || 0),
        dropOnResume: !!settings.renderSettings.dropOnResume
      },
      danmuFilter: {
        ...settings.danmuFilter,
        minLen: settings.danmuFilter.minLen === null ? null : Number(settings.danmuFilter.minLen),
        maxLen: settings.danmuFilter.maxLen === null ? null : Number(settings.danmuFilter.maxLen)
      }
    }

    const result = await tauriAPI('set_general_settings', { settings: payload })
    ElMessage.success(result)
    await reload()
  } catch (e) {
    console.error(e)
    ElMessage.error(`应用失败: ${e}`)
  }
}

onMounted(reload)
</script>

<style scoped>
.general-settings {
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
  gap: 12px;
}

.section {
  margin-top: 18px;
}

.section h3 {
  margin: 0 0 12px 0;
}

.hint {
  margin-left: 12px;
  color: #909399;
  font-size: 12px;
}

.actions {
  margin-top: 20px;
  display: flex;
  gap: 12px;
}
</style>
