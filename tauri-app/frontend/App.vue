<template>
  <div id="app">
    <el-container class="layout-container">
      <el-header class="header">
        <h1>弹幕控制系统</h1>
      </el-header>
      <el-main class="main-content">
        <ControlPanel />
      </el-main>
    </el-container>
  </div>
</template>

<script setup>
import ControlPanel from './components/ControlPanel.vue'
</script>

<style>
body {
  margin: 0;
  padding: 0;
  font-family: Arial, sans-serif;
}

.app-container {
  height: 100vh;
  width: 100vw;
  overflow: hidden;
}

.main-tabs {
  height: 100%;
}

.main-tabs .el-tab-pane {
  height: calc(100% - 40px);
  overflow: auto;
}
</style>