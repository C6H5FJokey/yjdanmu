from asyncio import sleep, run
import time
import requests

def send_danmu(message):
    data = {
  "type": "danmu",
  "text": message,
  "color": "#000000",
  "size": 32,
  "strokeColor": "#ffffff",
  "strokeWidth": 2,
  "typingSpeed": 100,
  "displayDuration": 2000,
  "fadeDuration": 1000,
  "shakeAmplitude": 2,
  "randomTilt": 10,
  "time": time.time()
}
    print(data)
    r = requests.post("http://localhost:8180/api/send-danmu", json=data)
    return r.status_code, r.text
    

danmu = [
    "痛苦啊，你就是我的唯一。",
    "除了你，我皆无欲求。",
    "痛苦啊，你忠实地陪伴着我，直至现在也没有一丝改变。",
    "当我的灵魂徘徊于深渊之底时。",
    "唯有你相伴在我的身旁，守护着我。",
    "我又怎能埋怨你呢。",
    "痛苦啊，你绝不会从我的身旁遁走。",
    "我终于能够表达对你的尊敬。",
    "现在也认识到了你的存在。",
    "而你只是存在于世，就已那么美丽。",
    "痛苦啊，你就像那从未离开我那贫苦的心之火炉旁的人一样。",
    "比我那身为至爱的恋人还要多情。",
    "我知道在我迈向死亡的那一天。",
    "你会进到我的内心深处。",
    "与我并排躺下。"
]

async def main():
    for message in danmu:
        status_code, response_text = send_danmu(message)
        print(f"Status Code: {status_code}, Response: {response_text}")
        await sleep(len(message) * 0.1 + 3)  

run(main())

# send_danmu("不过，我的惯性告诉我，我总得写点什么。")