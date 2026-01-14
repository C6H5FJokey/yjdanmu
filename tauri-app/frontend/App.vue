<template>
  <div id="app">
    <el-container class="layout-container">
      <el-main class="main-content">
        <el-tabs v-model="activeTab" class="main-tabs">
          <el-tab-pane label="控制面板" name="control">
            <ControlPanel />
          </el-tab-pane>
          <el-tab-pane label="WebSocket设置" name="websocket">
            <WebSocketSettings />
          </el-tab-pane>
          <el-tab-pane label="远程样式设置" name="remote-style">
            <RemoteStyleSettings />
          </el-tab-pane>
        </el-tabs>
      </el-main>
    </el-container>
  </div>
</template>

<script setup>
import { ref } from 'vue'
import ControlPanel from './components/ControlPanel.vue'
import WebSocketSettings from './components/WebSocketSettings.vue'
import RemoteStyleSettings from './components/RemoteStyleSettings.vue'

const activeTab = ref('control')
</script>

<style>
body {
  margin: 0;
  padding: 0;
  font-family: Arial, sans-serif;
  background-color: #00000000;
}

.app-container {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}

.main-tabs {
  height: 100%;
}
</style>