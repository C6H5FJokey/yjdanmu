<template>
  <div class="remote-style-settings">
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <div class="header-title">
            <div class="title">弹幕样式规则</div>
            <div class="subtitle">作用范围：全局（所有直播间）</div>
          </div>
          <el-tooltip placement="top" content="普通弹幕是基准；礼物/醒目留言等都在基准上附加样式或覆盖；粉丝牌/舰队/房管只做视觉强调。">
            <el-tag type="info">说明</el-tag>
          </el-tooltip>
        </div>
      </template>
      
      <div class="info-section">
        <el-alert
          title="提示"
          type="info"
          description="先设置『普通弹幕』作为基准样式。然后在『附加样式』里选择礼物/醒目留言/粉丝牌/舰队/房管等目标，决定是否启用，并编辑它的附加样式。"
          :closable="false"
          show-icon
        />
      </div>

      <el-tabs v-model="activeTab" class="tabs">
        <el-tab-pane label="普通弹幕" name="base">
          <div class="tab-desc">基准样式（其它弹幕默认继承它）。</div>
          <div class="style-section">
            <h3>danmu 基准样式</h3>
            <el-form :model="editStyle" label-width="120px">
              <el-form-item label="字体大小">
                <el-input-number v-model="editStyle.fontSize" :min="10" :max="100" :step="2" controls-position="right" />
              </el-form-item>
              <el-form-item label="字体颜色">
                <div style="display:flex; align-items:center; gap:12px;">
                  <el-color-picker v-model="textColorPickerValue" :disabled="followTextColor" />
                  <el-switch v-model="followTextColor" active-text="不覆盖" inactive-text="自定义" />
                </div>
              </el-form-item>
              <el-form-item label="描边颜色">
                <div style="display:flex; align-items:center; gap:12px;">
                  <el-color-picker v-model="strokeColorPickerValue" :disabled="followStrokeColor" />
                  <el-switch v-model="followStrokeColor" active-text="不覆盖" inactive-text="自定义" />
                </div>
              </el-form-item>
              <el-form-item label="描边宽度">
                <el-input-number v-model="editStyle.strokeWidth" :min="0" :max="10" :step="1" controls-position="right" />
              </el-form-item>
              <el-form-item label="打字速度(ms)">
                <el-input-number v-model="editStyle.typingSpeed" :min="10" :max="500" :step="10" controls-position="right" />
              </el-form-item>
              <el-form-item label="显示时长(ms)">
                <el-input-number v-model="editStyle.displayDuration" :min="1000" :max="10000" :step="500" controls-position="right" />
              </el-form-item>
              <el-form-item label="消失时长(ms)">
                <el-input-number v-model="editStyle.fadeDuration" :min="100" :max="10000" :step="100" controls-position="right" />
              </el-form-item>
              <el-form-item label="抖动幅度">
                <el-input-number v-model="editStyle.shakeAmplitude" :min="0" :max="10" :step="0.5" controls-position="right" />
              </el-form-item>
              <el-form-item label="随机倾斜角度">
                <el-input-number v-model="editStyle.randomTilt" :min="0" :max="45" :step="1" controls-position="right" />
              </el-form-item>
            </el-form>
          </div>
        </el-tab-pane>

        <el-tab-pane label="附加样式" name="extra">
          <div class="tab-desc">在基准样式基础上，为某个目标启用附加样式（礼物/醒目留言/粉丝牌/舰队/房管等）。</div>

          <div class="extra-toolbar">
            <el-select v-model="extraKey" style="width: 320px" placeholder="选择附加样式目标">
              <el-option-group label="弹幕附加（视觉强调）">
                <el-option v-for="o in overlayOptions" :key="o.key" :label="o.label" :value="o.key" />
              </el-option-group>
              <el-option-group label="其它弹幕类型（可整体覆盖）">
                <el-option v-for="t in extraTypeOptions" :key="t" :label="t" :value="t" />
              </el-option-group>
            </el-select>

            <el-switch v-model="extraEnabled" :disabled="!extraKey" active-text="启用" inactive-text="禁用" />

            <el-input v-model="newTypeKey" style="width: 260px" placeholder="新增其它弹幕类型（例如 like/enter）" clearable />
            <el-button @click="addExtraType" :disabled="!newTypeKey.trim()">新增</el-button>
            <el-button type="danger" @click="deleteExtraType" :disabled="!canDeleteExtraType">删除此类型覆盖</el-button>
          </div>

          <div v-if="extraKey" class="style-section">
            <h3>附加样式：{{ currentExtraLabel }}</h3>
            <el-form :model="editStyle" label-width="120px">
              <el-form-item label="字体大小">
                <el-input-number v-model="editStyle.fontSize" :disabled="!extraEnabled" :min="10" :max="100" :step="2" controls-position="right" />
              </el-form-item>
              <el-form-item label="字体颜色">
                <div style="display:flex; align-items:center; gap:12px;">
                  <el-color-picker v-model="textColorPickerValue" :disabled="!extraEnabled || followTextColor" />
                  <el-switch v-model="followTextColor" :disabled="!extraEnabled" active-text="不覆盖" inactive-text="自定义" />
                </div>
              </el-form-item>
              <el-form-item label="描边颜色">
                <div style="display:flex; align-items:center; gap:12px;">
                  <el-color-picker v-model="strokeColorPickerValue" :disabled="!extraEnabled || followStrokeColor" />
                  <el-switch v-model="followStrokeColor" :disabled="!extraEnabled" active-text="不覆盖" inactive-text="自定义" />
                </div>
              </el-form-item>
              <el-form-item label="描边宽度">
                <el-input-number v-model="editStyle.strokeWidth" :disabled="!extraEnabled" :min="0" :max="10" :step="1" controls-position="right" />
              </el-form-item>

              <el-form-item label="打字速度(ms)">
                <el-input-number v-model="editStyle.typingSpeed" :disabled="!extraEnabled || isOverlayExtra" :min="10" :max="500" :step="10" controls-position="right" />
              </el-form-item>
              <el-form-item label="显示时长(ms)">
                <el-input-number v-model="editStyle.displayDuration" :disabled="!extraEnabled || isOverlayExtra" :min="1000" :max="10000" :step="500" controls-position="right" />
              </el-form-item>
              <el-form-item label="消失时长(ms)">
                <el-input-number v-model="editStyle.fadeDuration" :disabled="!extraEnabled || isOverlayExtra" :min="100" :max="10000" :step="100" controls-position="right" />
              </el-form-item>
              <el-form-item label="抖动幅度">
                <el-input-number v-model="editStyle.shakeAmplitude" :disabled="!extraEnabled || isOverlayExtra" :min="0" :max="10" :step="0.5" controls-position="right" />
              </el-form-item>
              <el-form-item label="随机倾斜角度">
                <el-input-number v-model="editStyle.randomTilt" :disabled="!extraEnabled || isOverlayExtra" :min="0" :max="45" :step="1" controls-position="right" />
              </el-form-item>
            </el-form>

            <div v-if="!extraEnabled" class="sub-note">未启用：当前将直接使用基准样式。</div>
            <div v-else-if="isOverlayExtra" class="sub-note">这是“视觉强调”类附加样式：只会应用字号/颜色/描边（其它参数不生效）。</div>
            <div v-else class="sub-note">这是“其它弹幕类型覆盖”：启用后该类型将不再继承基准样式。</div>
          </div>
          <div v-else class="empty">先选择一个附加样式目标。</div>
        </el-tab-pane>
      </el-tabs>
      
      <div class="actions">
        <el-button @click="save" type="primary" :disabled="!canUseTauri">保存当前页</el-button>
        <el-button @click="load" :disabled="!canUseTauri">撤销修改</el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { reactive, ref, onMounted, watch, computed } from 'vue'
import { ElMessage } from 'element-plus'

const canUseTauri = typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__
const tauriAPI = canUseTauri ? window.__TAURI_INTERNALS__.invoke : null

const activeTab = ref('base')

const builtinTypes = ['gift', 'superChat']
const extraKey = ref('')
const newTypeKey = ref('')

const profile = reactive({
  base: {
    fontSize: 32,
    color: '#ffffff',
    strokeColor: '#000000',
    strokeWidth: 2,
    typingSpeed: 100,
    displayDuration: 3000,
    fadeDuration: 1000,
    shakeAmplitude: 2,
    randomTilt: 10
  },
  byType: {},
  ownMedal: null,
  guardGovernor: null,
  guardAdmiral: null,
  guardCaptain: null,
  streamer: null,
  moderator: null
})

const editStyle = reactive({ ...profile.base })

const lastTextColor = ref('#ffffff')
const lastStrokeColor = ref('#000000')

watch(
  () => editStyle.color,
  (v) => {
    if (typeof v === 'string' && v) lastTextColor.value = v
  }
)

watch(
  () => editStyle.strokeColor,
  (v) => {
    if (typeof v === 'string' && v) lastStrokeColor.value = v
  }
)

const followTextColor = computed({
  get: () => editStyle.color === null || editStyle.color === undefined,
  set: (v) => {
    if (v) {
      editStyle.color = null
    } else {
      if (editStyle.color === null || editStyle.color === undefined) {
        editStyle.color = lastTextColor.value || '#ffffff'
      }
    }
  }
})

const followStrokeColor = computed({
  get: () => editStyle.strokeColor === null || editStyle.strokeColor === undefined,
  set: (v) => {
    if (v) {
      editStyle.strokeColor = null
    } else {
      if (editStyle.strokeColor === null || editStyle.strokeColor === undefined) {
        editStyle.strokeColor = lastStrokeColor.value || '#000000'
      }
    }
  }
})

const textColorPickerValue = computed({
  get: () => {
    if (editStyle.color === null || editStyle.color === undefined) return lastTextColor.value
    return editStyle.color
  },
  set: (v) => {
    editStyle.color = v
  }
})

const strokeColorPickerValue = computed({
  get: () => {
    if (editStyle.strokeColor === null || editStyle.strokeColor === undefined) return lastStrokeColor.value
    return editStyle.strokeColor
  },
  set: (v) => {
    editStyle.strokeColor = v
  }
})

const extraEnabled = ref(false)

const overlayOptions = [
  { key: 'ownMedal', label: '粉丝牌（本房间）' },
  { key: 'guardCaptain', label: '舰长' },
  { key: 'guardAdmiral', label: '提督' },
  { key: 'guardGovernor', label: '总督' },
  { key: 'streamer', label: '主播/本人' },
  { key: 'moderator', label: '房管' }
]

const extraTypeOptions = ref([])

const refreshExtraTypeOptions = () => {
  const keys = new Set([...(builtinTypes || []), ...Object.keys(profile.byType || {})])
  keys.delete('danmu')
  extraTypeOptions.value = Array.from(keys)
}

const isOverlayExtra = computed(() => {
  const key = extraKey.value
  return !!overlayOptions.find(o => o.key === key)
})

const currentExtraLabel = computed(() => {
  const key = extraKey.value
  const overlay = overlayOptions.find(o => o.key === key)
  if (overlay) return overlay.label
  return key || ''
})

const canDeleteExtraType = computed(() => {
  const key = extraKey.value
  if (!key) return false
  if (isOverlayExtra.value) return false
  // 仅允许删除非内建类型；gift/superChat 不允许删除入口
  if (builtinTypes.includes(key)) return false
  return !!profile.byType?.[key]
})

const currentEditorMode = () => {
  if (activeTab.value === 'base') return 'base'
  if (activeTab.value === 'extra') return 'extra'
  return 'base'
}

const getDanmuBase = () => {
  const override = profile.byType?.danmu
  return override || profile.base
}

const syncEditFromProfile = () => {
  const mode = currentEditorMode()
  if (mode === 'base') {
    Object.assign(editStyle, getDanmuBase())
    return
  }
  if (mode === 'extra') {
    if (!extraKey.value) return

    if (isOverlayExtra.value) {
      const cfg = profile[extraKey.value]
      extraEnabled.value = !!cfg
      Object.assign(editStyle, cfg || getDanmuBase())
      return
    }

    const override = profile.byType?.[extraKey.value]
    extraEnabled.value = !!override
    Object.assign(editStyle, override || getDanmuBase())
  }
}

watch([activeTab, extraKey], syncEditFromProfile)

const load = async () => {
  try {
    if (!tauriAPI) return

    const resp = await tauriAPI('get_style_profile')

    // resp.profile 的字段是 camelCase：base / byType
    profile.base = resp.profile.base
    profile.byType = resp.profile.byType || {}
    profile.ownMedal = resp.profile.ownMedal || null
    profile.guardGovernor = resp.profile.guardGovernor || null
    profile.guardAdmiral = resp.profile.guardAdmiral || null
    profile.guardCaptain = resp.profile.guardCaptain || null
    profile.streamer = resp.profile.streamer || null
    profile.moderator = resp.profile.moderator || null

    refreshExtraTypeOptions()

    if (!extraKey.value) {
      // 默认选择 gift（用户最常配），否则选第一个视觉强调项
      extraKey.value = extraTypeOptions.value.includes('gift') ? 'gift' : overlayOptions[0].key
    }

    syncEditFromProfile()
  } catch (e) {
    console.error(e)
    ElMessage.error(`加载样式配置失败: ${e}`)
  }
}

const save = async () => {
  try {
    if (!tauriAPI) return

    const nextProfile = {
      base: { ...profile.base },
      byType: { ...(profile.byType || {}) },
      ownMedal: profile.ownMedal ? { ...profile.ownMedal } : null,
      guardGovernor: profile.guardGovernor ? { ...profile.guardGovernor } : null,
      guardAdmiral: profile.guardAdmiral ? { ...profile.guardAdmiral } : null,
      guardCaptain: profile.guardCaptain ? { ...profile.guardCaptain } : null,
      streamer: profile.streamer ? { ...profile.streamer } : null,
      moderator: profile.moderator ? { ...profile.moderator } : null
    }

    if (activeTab.value === 'base') {
      // 普通弹幕基准写到 byType.danmu
      nextProfile.byType.danmu = { ...editStyle }
    } else if (activeTab.value === 'extra') {
      if (!extraKey.value) {
        ElMessage.warning('请选择附加样式目标')
        return
      }

      if (isOverlayExtra.value) {
        // 视觉强调类附加：仅保存视觉字段
        const danmuBase = getDanmuBase()
        const visualOnly = {
          fontSize: editStyle.fontSize,
          color: editStyle.color,
          strokeColor: editStyle.strokeColor,
          strokeWidth: editStyle.strokeWidth,
          typingSpeed: danmuBase.typingSpeed,
          displayDuration: danmuBase.displayDuration,
          fadeDuration: danmuBase.fadeDuration,
          shakeAmplitude: danmuBase.shakeAmplitude,
          randomTilt: danmuBase.randomTilt
        }
        nextProfile[extraKey.value] = extraEnabled.value ? visualOnly : null
      } else {
        // 其它弹幕类型：启用时保存独立覆盖；禁用时删除覆盖，回到继承基准
        if (extraEnabled.value) {
          nextProfile.byType[extraKey.value] = { ...editStyle }
        } else {
          if (nextProfile.byType && nextProfile.byType[extraKey.value]) {
            delete nextProfile.byType[extraKey.value]
          }
        }
      }
    }

    const result = await tauriAPI('set_style_profile', { profile: nextProfile })
    ElMessage.success(result)

    // 重新加载，保证本地状态一致
    await load()
  } catch (e) {
    console.error(e)
    ElMessage.error(`保存失败: ${e}`)
  }
}

const addExtraType = () => {
  const key = newTypeKey.value.trim()
  if (!key) return
  if (!profile.byType) profile.byType = {}
  if (!profile.byType[key]) {
    profile.byType[key] = { ...getDanmuBase() }
  }
  extraKey.value = key
  extraEnabled.value = true
  newTypeKey.value = ''
  refreshExtraTypeOptions()
  syncEditFromProfile()
}

const deleteExtraType = () => {
  if (!canDeleteExtraType.value) return
  const key = extraKey.value
  if (profile.byType && profile.byType[key]) {
    delete profile.byType[key]
  }
  extraEnabled.value = false
  refreshExtraTypeOptions()
  syncEditFromProfile()
}

onMounted(load)
</script>

<style scoped>
.remote-style-settings {
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

.header-title {
  display: flex;
  flex-direction: column;
}

.title {
  font-weight: 600;
}

.subtitle {
  font-size: 12px;
  color: #666;
}

.info-section {
  margin-bottom: 20px;
}

.style-section {
  margin-bottom: 30px;
}

.style-section h3 {
  margin-top: 0;
  margin-bottom: 15px;
}

.actions {
  margin-top: 12px;
  display: flex;
  gap: 12px;
}

.tabs {
  margin-top: 8px;
}

.tab-desc {
  margin: 8px 0 12px;
  color: #666;
  font-size: 12px;
}

.extra-toolbar {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-bottom: 12px;
  flex-wrap: wrap;
}

.empty {
  color: #999;
  padding: 12px 0;
}

.sub-note {
  margin-top: 8px;
  color: #999;
  font-size: 12px;
}
</style>