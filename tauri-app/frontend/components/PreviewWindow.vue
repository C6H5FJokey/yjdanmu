<template>
  <div class="preview-window">
    <div class="preview-header">
      <h3>预览窗口</h3>
      <div class="controls">
        <el-button type="primary" @click="openExternalPreview" size="small">
          在新窗口打开预览
        </el-button>
        <el-button @click="refreshPreview" size="small" icon="Refresh">
          刷新
        </el-button>
      </div>
    </div>
    <div class="preview-content">
      <iframe 
        v-if="showPreview" 
        :src="previewUrl" 
        width="100%" 
        height="100%" 
        frameborder="0"
        ref="previewFrame"
      ></iframe>
      <div v-else class="no-preview">
        <p>预览窗口已分离</p>
        <el-button @click="showEmbeddedPreview" type="primary">重新嵌入预览</el-button>
      </div>
    </div>
  </div>
</template>

<script>
import { ref, onMounted, onUnmounted } from 'vue';
import { ElButton } from 'element-plus';

export default {
  name: 'PreviewWindow',
  components: {
    ElButton
  },
  setup() {
    const showPreview = ref(true);
    const previewFrame = ref(null);
    const externalWindow = ref(null);
    const isExternal = ref(false);
    
    const previewUrl = ref('http://localhost:8081/preview.html'); // 使用本地的预览页面
    
    // 打开外部预览窗口
    const openExternalPreview = () => {
      if (externalWindow.value && !externalWindow.value.closed) {
        externalWindow.value.focus();
        return;
      }
      
      // 尝试通过Tauri打开新窗口
      try {
        // 这里我们先尝试用普通方式打开新窗口
        externalWindow.value = window.open(
          previewUrl.value, 
          'danmu-preview', 
          'width=1200,height=800,resizable=yes,scrollbars=yes'
        );
        
        if (externalWindow.value) {
          isExternal.value = true;
          showPreview.value = false;
          
          // 监听窗口关闭事件
          const checkClosed = setInterval(() => {
            if (externalWindow.value.closed) {
              clearInterval(checkClosed);
              isExternal.value = false;
              externalWindow.value = null;
            }
          }, 1000);
        }
      } catch (error) {
        console.error('打开外部预览窗口失败:', error);
        // 如果无法打开新窗口，则使用嵌入式预览
        showPreview.value = true;
      }
    };
    
    // 刷新预览
    const refreshPreview = async () => {
      if (previewFrame.value) {
        // 先将iframe的src设为空，触发页面卸载事件
        previewFrame.value.src = 'about:blank';
        
        // 等待一段时间确保旧连接已断开
        await new Promise(resolve => setTimeout(resolve, 100));
        
        // 重新加载预览页面
        previewFrame.value.src = previewUrl.value;
      }
    };
    
    // 重新显示嵌入式预览
    const showEmbeddedPreview = () => {
      showPreview.value = true;
      isExternal.value = false;
    };
    
    // 监听来自预览窗口的消息
    const handleMessage = (event) => {
      // 处理来自预览窗口的消息
      if (event.origin === window.location.origin || event.origin === 'http://localhost:8081') {
        // 可以根据需要处理特定消息
        console.log('收到预览窗口消息:', event.data);
      }
    };
    
    onMounted(() => {
      window.addEventListener('message', handleMessage);
    });
    
    onUnmounted(() => {
      window.removeEventListener('message', handleMessage);
      if (externalWindow.value && !externalWindow.value.closed) {
        externalWindow.value.close();
      }
    });
    
    return {
      showPreview,
      previewFrame,
      previewUrl,
      openExternalPreview,
      refreshPreview,
      showEmbeddedPreview
    };
  }
};
</script>

<style scoped>
.preview-window {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: #f5f5f5;
}

.preview-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 16px;
  background: #ffffff;
  border-bottom: 1px solid #e4e7ed;
  box-shadow: 0 1px 2px rgba(0,0,0,0.05);
}

.preview-header h3 {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: #303133;
}

.controls {
  display: flex;
  gap: 8px;
}

.preview-content {
  flex: 1;
  min-height: 0;
  position: relative;
}

.preview-content iframe {
  width: 100%;
  height: 100%;
  border: none;
}

.no-preview {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  height: 100%;
  padding: 20px;
  text-align: center;
  color: #909399;
}
</style>