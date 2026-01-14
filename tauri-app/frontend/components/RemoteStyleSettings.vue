<template>
  <div class="remote-style-settings">
    <el-card class="settings-card">
      <template #header>
        <div class="card-header">
          <span>远程样式设置</span>
        </div>
      </template>
      
      <div class="info-section">
        <el-alert
          title="提示"
          type="info"
          description="此页面用于配置远程弹幕的样式设置。具体功能待实现。"
          :closable="false"
          show-icon
        />
      </div>
      
      <!-- 远程样式设置区域 -->
      <div class="style-section">
        <h3>远程样式配置</h3>
        <el-form :model="remoteStyleSettings" label-width="120px">
          <el-form-item label="字体大小">
            <el-input-number 
              v-model="remoteStyleSettings.fontSize" 
              :min="10" 
              :max="100" 
              :step="2"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="字体颜色">
            <el-color-picker v-model="remoteStyleSettings.color" />
          </el-form-item>
          
          <el-form-item label="描边颜色">
            <el-color-picker v-model="remoteStyleSettings.strokeColor" />
          </el-form-item>
          
          <el-form-item label="描边宽度">
            <el-input-number 
              v-model="remoteStyleSettings.strokeWidth" 
              :min="0" 
              :max="10" 
              :step="1"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="打字速度(ms)">
            <el-input-number 
              v-model="remoteStyleSettings.typingSpeed" 
              :min="10" 
              :max="500" 
              :step="10"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="显示时长(ms)">
            <el-input-number 
              v-model="remoteStyleSettings.displayDuration" 
              :min="1000" 
              :max="10000" 
              :step="500"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="消失时长(ms)">
            <el-input-number 
              v-model="remoteStyleSettings.fadeDuration" 
              :min="100" 
              :max="10000" 
              :step="100"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="抖动幅度">
            <el-input-number 
              v-model="remoteStyleSettings.shakeAmplitude" 
              :min="0" 
              :max="10" 
              :step="0.5"
              controls-position="right"
            />
          </el-form-item>
          
          <el-form-item label="随机倾斜角度">
            <el-input-number 
              v-model="remoteStyleSettings.randomTilt" 
              :min="0" 
              :max="45" 
              :step="1"
              controls-position="right"
            />
          </el-form-item>
        </el-form>
        
        <el-button @click="applyRemoteStyleSettings" type="primary">应用远程样式设置</el-button>
      </div>
    </el-card>
  </div>
</template>

<script setup>
import { reactive } from 'vue'
import { ElMessage } from 'element-plus'

const remoteStyleSettings = reactive({
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

const applyRemoteStyleSettings = async () => {
  try {
    ElMessage.info('远程样式设置功能待实现')
  } catch (error) {
    console.error('应用远程样式设置失败:', error)
    ElMessage.error('应用远程样式设置失败')
  }
}
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
</style>