# -*- coding: utf-8 -*-
import asyncio
import logging
import os
import sys

# 让仓库内的 blivedm 可直接 import（包根目录在 ./blivedm）
REPO_ROOT = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(REPO_ROOT, 'blivedm'))

import blivedm  # noqa: E402
import blivedm.models.web as web_models  # noqa: E402


class MyHandler(blivedm.BaseHandler):
    def _on_heartbeat(self, client: blivedm.BLiveClient, message: web_models.HeartbeatMessage):
        print(f'[{client.room_id}] heartbeat popularity={message.popularity}')

    def _on_danmaku(self, client: blivedm.BLiveClient, message: web_models.DanmakuMessage):
        print(f'[{client.room_id}] {message.uname}: {message.msg}')


async def main():
    if len(sys.argv) < 2:
        print('Usage: python test_blivedm_debug_room.py <ROOM_ID>')
        sys.exit(2)

    room_id = int(sys.argv[1])

    # 让 BLIVEDM_DEBUG 的 warning 输出可见
    logging.basicConfig(level=logging.WARNING)

    client = blivedm.BLiveClient(room_id)
    client.set_handler(MyHandler())

    client.start()
    try:
        await asyncio.sleep(20)
        client.stop()
        await client.join()
    finally:
        await client.stop_and_close()


if __name__ == '__main__':
    # 推荐：在命令行设置 BLIVEDM_DEBUG=1
    asyncio.run(main())
